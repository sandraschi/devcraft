//! Fleet pictures: commit days, line counts, languages, and tags.
//! Read-only. Hidden folders and names that start with `_` stay out.

use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;

const WEEKS: i64 = 26;
const TIDE_WEEKS: usize = 12;
const CELLS: usize = (WEEKS as usize) * 7;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DayCell {
    pub day: String,
    pub commits: u32,
    pub future: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WeekFlow {
    pub label: String,
    pub commits: u32,
    pub added: u64,
    pub deleted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LangShare {
    pub name: String,
    pub lines: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoSize {
    pub name: String,
    pub lines: u64,
    pub lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseMark {
    pub repo: String,
    pub day: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Star {
    pub name: String,
    pub commits: u32,
    pub lines: u64,
    pub lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorShare {
    pub name: String,
    pub commits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphSnap {
    pub generated: String,
    pub repos: u32,
    pub commits: u64,
    pub lines: u64,
    pub added: u64,
    pub deleted: u64,
    pub days: Vec<DayCell>,
    pub weeks: Vec<WeekFlow>,
    pub langs: Vec<LangShare>,
    pub largest: Vec<RepoSize>,
    pub releases: Vec<ReleaseMark>,
    /// Checkouts with at least one commit in the window. The night sky.
    #[serde(default)]
    pub stars: Vec<Star>,
    /// People named on those commits. At most eight.
    #[serde(default)]
    pub authors: Vec<AuthorShare>,
    /// Monday through Sunday.
    #[serde(default)]
    pub weekdays: [u32; 7],
    /// Checkouts with a commit in the last seven days.
    #[serde(default)]
    pub hot: u32,
    /// Checkouts with no commit in the window.
    #[serde(default)]
    pub quiet: u32,
}

#[derive(Debug, Default)]
struct RepoPiece {
    name: String,
    commits: BTreeMap<String, u32>,
    added: BTreeMap<String, u64>,
    deleted: BTreeMap<String, u64>,
    lines: BTreeMap<String, u64>,
    tags: Vec<(String, String)>,
    authors: BTreeMap<String, u32>,
}

pub fn cache_path(root: &Path) -> PathBuf {
    root.join(".devcraft").join("graphs.json")
}

pub fn load_cache(root: &Path) -> Option<GraphSnap> {
    let text = fs::read_to_string(cache_path(root)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_cache(root: &Path, snap: &GraphSnap) {
    let path = cache_path(root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(snap) {
        let _ = fs::write(path, text);
    }
}

pub fn measure(root: &Path) -> GraphSnap {
    let jobs = list_git_repos(root);
    let pieces = Mutex::new(Vec::new());
    let next = Mutex::new(0usize);
    let workers = 8.min(jobs.len());
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(index) = next_index(&next, jobs.len()) {
                    let (name, path) = &jobs[index];
                    let piece = read_repo(name, path);
                    lock(&pieces).push(piece);
                }
            });
        }
    });
    let pieces = lock(&pieces);
    let mut snap = assemble(Utc::now().date_naive(), &pieces);
    snap.generated = Utc::now().to_rfc3339();
    snap
}

fn assemble(today: NaiveDate, pieces: &[RepoPiece]) -> GraphSnap {
    let start = grid_start(today);
    let mut days = Vec::with_capacity(CELLS);
    let mut commits = 0u64;
    let mut weekdays = [0u32; 7];
    let mut cursor = start;
    for _ in 0..CELLS {
        let key = cursor.format("%Y-%m-%d").to_string();
        let mut count = 0u32;
        for piece in pieces {
            count = count.saturating_add(piece.commits.get(&key).copied().unwrap_or(0));
        }
        commits = commits.saturating_add(u64::from(count));
        let slot = cursor.weekday().num_days_from_monday() as usize;
        if slot < 7 {
            weekdays[slot] = weekdays[slot].saturating_add(count);
        }
        days.push(DayCell {
            day: key,
            commits: count,
            future: cursor > today,
        });
        cursor += Duration::days(1);
    }

    let tide_from = CELLS.saturating_sub(TIDE_WEEKS * 7);
    let mut weeks = Vec::with_capacity(TIDE_WEEKS);
    let mut added = 0u64;
    let mut deleted = 0u64;
    for week in 0..TIDE_WEEKS {
        let offset = tide_from + week * 7;
        let monday = start + Duration::days(offset as i64);
        let mut week_commits = 0u32;
        let mut week_added = 0u64;
        let mut week_deleted = 0u64;
        for cell in days.iter().skip(offset).take(7) {
            week_commits = week_commits.saturating_add(cell.commits);
            for piece in pieces {
                week_added =
                    week_added.saturating_add(piece.added.get(&cell.day).copied().unwrap_or(0));
                week_deleted =
                    week_deleted.saturating_add(piece.deleted.get(&cell.day).copied().unwrap_or(0));
            }
        }
        added = added.saturating_add(week_added);
        deleted = deleted.saturating_add(week_deleted);
        weeks.push(WeekFlow {
            label: monday.format("%m-%d").to_string(),
            commits: week_commits,
            added: week_added,
            deleted: week_deleted,
        });
    }

    let hot_from = (today - Duration::days(6)).format("%Y-%m-%d").to_string();
    let mut lang_totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut author_totals: BTreeMap<String, u32> = BTreeMap::new();
    let mut sizes: Vec<RepoSize> = Vec::new();
    let mut stars = Vec::new();
    let mut lines = 0u64;
    let mut hot = 0u32;
    let mut quiet = 0u32;
    for piece in pieces {
        let mut total = 0u64;
        let mut best_lang = String::new();
        let mut best = 0u64;
        for (lang, count) in &piece.lines {
            total = total.saturating_add(*count);
            lines = lines.saturating_add(*count);
            *lang_totals.entry(lang.clone()).or_default() += *count;
            if *count > best {
                best = *count;
                best_lang = lang.clone();
            }
        }
        if total > 0 {
            sizes.push(RepoSize {
                name: piece.name.clone(),
                lines: total,
                lang: best_lang.clone(),
            });
        }
        let mut commit_n = 0u32;
        let mut recent = false;
        for (day, count) in &piece.commits {
            commit_n = commit_n.saturating_add(*count);
            if day.as_str() >= hot_from.as_str() && *count > 0 {
                recent = true;
            }
        }
        if commit_n == 0 {
            quiet = quiet.saturating_add(1);
        } else {
            hot = hot.saturating_add(u32::from(recent));
            stars.push(Star {
                name: piece.name.clone(),
                commits: commit_n,
                lines: total,
                lang: best_lang,
            });
        }
        for (name, count) in &piece.authors {
            author_totals
                .entry(name.clone())
                .and_modify(|total| *total = total.saturating_add(*count))
                .or_insert(*count);
        }
    }
    sizes.sort_by(|a, b| b.lines.cmp(&a.lines).then(a.name.cmp(&b.name)));
    sizes.truncate(12);
    stars.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.name.cmp(&b.name)));
    stars.truncate(72);
    let mut authors: Vec<AuthorShare> = author_totals
        .into_iter()
        .map(|(name, commits)| AuthorShare { name, commits })
        .collect();
    authors.sort_by(|a, b| b.commits.cmp(&a.commits).then(a.name.cmp(&b.name)));
    authors.truncate(8);

    let mut langs: Vec<LangShare> = lang_totals
        .into_iter()
        .map(|(name, lines)| LangShare { name, lines })
        .collect();
    langs.sort_by(|a, b| b.lines.cmp(&a.lines).then(a.name.cmp(&b.name)));
    if langs.len() > 8 {
        let rest = langs[8..].iter().map(|lang| lang.lines).sum();
        langs.truncate(8);
        if rest > 0 {
            langs.push(LangShare {
                name: "Other".into(),
                lines: rest,
            });
        }
    }

    let mut releases = Vec::new();
    for piece in pieces {
        for (day, name) in &piece.tags {
            releases.push(ReleaseMark {
                repo: piece.name.clone(),
                day: day.clone(),
                name: name.clone(),
            });
        }
    }
    releases.sort_by(|a, b| b.day.cmp(&a.day).then(a.repo.cmp(&b.repo)));
    releases.truncate(40);

    GraphSnap {
        generated: String::new(),
        repos: pieces.len() as u32,
        commits,
        lines,
        added,
        deleted,
        days,
        weeks,
        langs,
        largest: sizes,
        releases,
        stars,
        authors,
        weekdays,
        hot,
        quiet,
    }
}

fn grid_start(today: NaiveDate) -> NaiveDate {
    let monday_offset = i64::from(today.weekday().num_days_from_monday());
    let this_monday = today - Duration::days(monday_offset);
    this_monday - Duration::days((WEEKS - 1) * 7)
}

fn list_git_repos(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut repos = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }
        if crate::mirrored_upstream(root, name) {
            continue;
        }
        if path.join(".git").exists() {
            repos.push((name.to_string(), path));
        }
    }
    repos.sort_by(|a, b| a.0.cmp(&b.0));
    repos
}

fn read_repo(name: &str, path: &Path) -> RepoPiece {
    let mut piece = RepoPiece {
        name: name.to_string(),
        ..RepoPiece::default()
    };
    parse_log(
        &git(
            path,
            &[
                "log",
                "--since=182.days",
                "--pretty=format:%as%x09%an",
                "--numstat",
            ],
        ),
        &mut piece,
    );
    parse_grep(&git(path, &["grep", "-I", "-c", "-e", "^"]), &mut piece);
    parse_tags(
        &git(
            path,
            &[
                "for-each-ref",
                "--sort=-creatordate",
                "--count=8",
                "--format=%(creatordate:short)%09%(refname:short)",
                "refs/tags",
            ],
        ),
        &mut piece,
    );
    piece
}

fn git(path: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks")
        .arg("-C")
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(out) = cmd.output() else {
        return String::new();
    };
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn parse_log(text: &str, piece: &mut RepoPiece) {
    let mut day = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if let Some((stamp, author)) = line.split_once('\t') {
            if is_day(stamp) {
                day = stamp.to_string();
                *piece.commits.entry(day.clone()).or_default() += 1;
                let author = author.trim();
                if !author.is_empty() {
                    *piece.authors.entry(author.to_string()).or_default() += 1;
                }
                continue;
            }
        }
        if is_day(line) {
            day = line.to_string();
            *piece.commits.entry(day.clone()).or_default() += 1;
            continue;
        }
        if day.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(added) = parts.next() else {
            continue;
        };
        let Some(deleted) = parts.next() else {
            continue;
        };
        if added == "-" || deleted == "-" {
            continue;
        }
        let Ok(added) = added.parse::<u64>() else {
            continue;
        };
        let Ok(deleted) = deleted.parse::<u64>() else {
            continue;
        };
        *piece.added.entry(day.clone()).or_default() += added;
        *piece.deleted.entry(day.clone()).or_default() += deleted;
    }
}

fn parse_grep(text: &str, piece: &mut RepoPiece) {
    for raw in text.lines() {
        let line = raw.trim();
        let Some((path, count)) = line.rsplit_once(':') else {
            continue;
        };
        let Ok(count) = count.trim().parse::<u64>() else {
            continue;
        };
        if count == 0 || skipped_path(path) {
            continue;
        }
        let Some(lang) = language(path) else {
            continue;
        };
        *piece.lines.entry(lang.to_string()).or_default() += count;
    }
}

fn parse_tags(text: &str, piece: &mut RepoPiece) {
    for raw in text.lines() {
        let line = raw.trim();
        let Some((day, name)) = line.split_once('\t') else {
            continue;
        };
        if !is_day(day) || name.is_empty() {
            continue;
        }
        let name = name.strip_prefix("refs/tags/").unwrap_or(name);
        piece.tags.push((day.to_string(), name.to_string()));
    }
}

fn is_day(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn skipped_path(path: &str) -> bool {
    path.split(['/', '\\']).any(|seg| {
        matches!(
            seg,
            "target"
                | "node_modules"
                | "dist"
                | "vendor"
                | ".git"
                | "venv"
                | ".venv"
                | "__pycache__"
        )
    })
}

fn language(path: &str) -> Option<&'static str> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if name.eq_ignore_ascii_case("cargo.lock") || name.eq_ignore_ascii_case("package-lock.json") {
        return None;
    }
    let ext = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    Some(match ext.to_ascii_lowercase().as_str() {
        "rs" => "Rust",
        "py" => "Python",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" | "mjs" => "JavaScript",
        "ps1" => "PowerShell",
        "go" => "Go",
        "c" | "h" => "C",
        "cpp" | "cc" | "hpp" | "cxx" => "C++",
        "cs" => "C#",
        "css" | "scss" => "CSS",
        "html" | "htm" => "HTML",
        "sql" => "SQL",
        "java" => "Java",
        "rb" => "Ruby",
        "php" => "PHP",
        "lua" => "Lua",
        "swift" => "Swift",
        "kt" => "Kotlin",
        "vue" => "Vue",
        "svelte" => "Svelte",
        _ => return None,
    })
}

fn next_index(counter: &Mutex<usize>, len: usize) -> Option<usize> {
    let mut guard = lock(counter);
    if *guard >= len {
        None
    } else {
        let index = *guard;
        *guard += 1;
        Some(index)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn log_grep_and_tags_become_a_snap() {
        let mut piece = RepoPiece {
            name: "demo".into(),
            ..RepoPiece::default()
        };
        parse_log(
            "\
2026-11-16	Ada
12\t4\tsrc/main.rs
-\t-\tpic.bin

2026-11-10
3\t1\tapp.py
",
            &mut piece,
        );
        parse_grep(
            "\
src/main.rs:20
target/debug/main.rs:99
web/app.tsx:8
notes.md:4
",
            &mut piece,
        );
        parse_tags("2026-11-01\tv0.2.0\nnope\n", &mut piece);
        assert_eq!(piece.commits.get("2026-11-16"), Some(&1));
        assert_eq!(piece.added.get("2026-11-16"), Some(&12));
        assert_eq!(piece.deleted.get("2026-11-10"), Some(&1));
        assert_eq!(piece.lines.get("Rust"), Some(&20));
        assert_eq!(piece.lines.get("TypeScript"), Some(&8));
        assert!(!piece.lines.contains_key("target"));
        assert_eq!(piece.tags.len(), 1);

        let today = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
        let snap = assemble(today, &[piece]);
        let cell = snap
            .days
            .iter()
            .find(|day| day.day == "2026-11-16")
            .unwrap();
        assert_eq!(cell.commits, 1);
        assert!(!cell.future);
        assert_eq!(snap.days.len(), CELLS);
        assert_eq!(snap.weeks.len(), TIDE_WEEKS);
        assert_eq!(snap.lines, 28);
        assert_eq!(snap.langs[0].name, "Rust");
        assert_eq!(snap.largest[0].name, "demo");
        assert_eq!(snap.releases[0].name, "v0.2.0");
        assert!(snap.added >= 15);
        assert_eq!(snap.authors[0].name, "Ada");
        assert_eq!(snap.stars[0].name, "demo");
        assert_eq!(snap.weekdays[0], 1);
        assert_eq!(snap.hot, 1);
        assert_eq!(snap.quiet, 0);
    }

    #[test]
    fn a_temp_repo_is_measured() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("devcraft-graphs-{nanos}"));
        let repo = root.join("sample");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(repo.join("src")).unwrap();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "{args:?}");
        };
        git(&["init"]);
        git(&["config", "user.email", "devcraft@example.com"]);
        git(&["config", "user.name", "DevCraft"]);
        fs::write(repo.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        git(&["add", "src/main.rs"]);
        git(&["commit", "-m", "start"]);
        git(&["tag", "v0.1.0"]);
        let snap = measure(&root);
        assert_eq!(snap.repos, 1);
        assert!(snap.commits >= 1, "commits {}", snap.commits);
        assert!(snap.lines >= 1, "lines {}", snap.lines);
        assert!(snap.langs.iter().any(|lang| lang.name == "Rust"));
        assert!(snap.releases.iter().any(|mark| mark.name == "v0.1.0"));
        let _ = fs::remove_dir_all(&root);
    }
}
