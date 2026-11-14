//! Live fleet board. Columns are computed from the working tree.
//! A drag is accepted or refused here, then the cockpit applies it.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const INBOX: &str = "inbox";
pub const READY: &str = "ready";
pub const ACTIVE: &str = "active";
pub const REVIEW: &str = "review";
pub const BLOCKED: &str = "blocked";
pub const DONE: &str = "done";
pub const QUIET: &str = "quiet";

pub const COLUMNS: [&str; 7] = [INBOX, READY, ACTIVE, REVIEW, BLOCKED, DONE, QUIET];

const DAY: u64 = 24;
const WEEK: u64 = 24 * 7;
const QUIET_AFTER: u64 = 24 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Facts {
    pub id: String,
    pub name: String,
    pub path: String,
    pub has_git: bool,
    pub branch: String,
    pub dirty: usize,
    pub untracked: usize,
    pub ahead: u32,
    pub unmerged: bool,
    pub index_lock: bool,
    pub has_agents: bool,
    pub has_claude: bool,
    pub handover_next: String,
    pub handover_age_hours: Option<u64>,
    pub last_commit_age_hours: Option<u64>,
    /// Commits per day for the last 14 days. Index 0 is the oldest day.
    pub commits_14: [u16; 14],
    /// Linked worktrees other than this checkout.
    pub worktrees: Vec<Worktree>,
    /// A TaskCraft issue key, such as BC-12. Empty when the card has none.
    pub task_key: Option<String>,
    pub claimed_by: Option<String>,
    pub pinned: bool,
    pub craft: bool,
}

/// One extra checkout created with `git worktree`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Worktree {
    pub name: String,
    pub branch: String,
}

impl Facts {
    pub fn changes(&self) -> usize {
        self.dirty + self.untracked
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Snooze,
    Pin,
    Claim,
    Release,
}

pub fn column(f: &Facts) -> &'static str {
    if block_reason(f).is_some() {
        return BLOCKED;
    }
    if f.claimed_by.is_some() {
        return ACTIVE;
    }
    if f.changes() > 0 || f.ahead > 0 {
        return REVIEW;
    }
    if f.last_commit_age_hours.is_some_and(|h| h < DAY) {
        return DONE;
    }
    if f.pinned || f.handover_age_hours.is_some_and(|h| h <= WEEK) {
        return READY;
    }
    if f.last_commit_age_hours.is_none() || f.last_commit_age_hours.is_some_and(|h| h > QUIET_AFTER)
    {
        return QUIET;
    }
    READY
}

pub fn block_reason(f: &Facts) -> Option<String> {
    if f.index_lock {
        return Some("git index.lock is still here".into());
    }
    if f.unmerged {
        return Some("the tree has unmerged paths".into());
    }
    if f.handover_age_hours.is_some_and(|h| h > WEEK) && f.changes() > 0 {
        return Some("handover is older than 7 days and the tree is dirty".into());
    }
    None
}

/// Cards worth a column. Quiet repos stay a count on Today.
/// Hours fall into under a day, a week, 30 days, 60 days, or older.
pub fn age_bands(repos: &[Facts]) -> [usize; 5] {
    let mut bands = [0usize; 5];
    for repo in repos {
        let band = match repo.last_commit_age_hours {
            Some(hours) if hours < DAY => 0,
            Some(hours) if hours < WEEK => 1,
            Some(hours) if hours < 24 * 30 => 2,
            Some(hours) if hours < QUIET_AFTER => 3,
            _ => 4,
        };
        bands[band] += 1;
    }
    bands
}

/// Claims held by cursor, opencode, agy, and claude, in that order.
pub fn holder_counts(repos: &[Facts]) -> [usize; 4] {
    let mut counts = [0usize; 4];
    for repo in repos {
        let Some(holder) = repo.claimed_by.as_deref() else {
            continue;
        };
        let index = match holder {
            "cursor" => 0,
            "opencode" => 1,
            "agy" => 2,
            "claude" => 3,
            _ => continue,
        };
        counts[index] += 1;
    }
    counts
}

/// Sum the per-repo 14-day series. Index 0 is the oldest day.
pub fn activity(repos: &[Facts]) -> [u32; 14] {
    let mut total = [0u32; 14];
    for repo in repos {
        for (index, count) in repo.commits_14.iter().enumerate() {
            total[index] += u32::from(*count);
        }
    }
    total
}

/// Count `YYYY-MM-DD` lines from `git log --pretty=%cs` into the 14 days ending on `today`.
pub fn tally_iso_days(log: &str, today: &str) -> [u16; 14] {
    let mut counts = [0u16; 14];
    let Some(today_ord) = iso_ord(today) else {
        return counts;
    };
    for line in log.lines() {
        let Some(ord) = iso_ord(line.trim()) else {
            continue;
        };
        let age = today_ord.saturating_sub(ord);
        if age < 14 {
            let index = 13 - age as usize;
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
}

fn iso_ord(text: &str) -> Option<i32> {
    if text.len() < 10 {
        return None;
    }
    let year: i32 = text.get(0..4)?.parse().ok()?;
    let month: u32 = text.get(5..7)?.parse().ok()?;
    let day: u32 = text.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(civil_days(year, month, day))
}

fn civil_days(year: i32, month: u32, day: u32) -> i32 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = (year - era * 400) as u32;
    let month_shift = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shift + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era as i32 - 719468
}

pub fn shown(f: &Facts) -> bool {
    match column(f) {
        QUIET => false,
        READY => f.pinned || f.handover_age_hours.is_some(),
        _ => true,
    }
}

pub fn allow(to: &str, github: bool) -> Result<Move, String> {
    match to {
        BLOCKED | QUIET => Err("Blocked and Quiet are filled from the working tree.".into()),
        REVIEW => Err("Review is filled from a dirty tree or commits ahead of origin.".into()),
        INBOX => Ok(Move::Snooze),
        READY if github => Err("Pin a repository. This card is a digest line.".into()),
        READY => Ok(Move::Pin),
        ACTIVE if github => {
            Err("This is a GitHub item. Claim the checkout, not the digest line.".into())
        }
        ACTIVE => Ok(Move::Claim),
        DONE if github => Ok(Move::Snooze),
        DONE => Ok(Move::Release),
        other => Err(format!("Unknown column {other}")),
    }
}

/// `BC-12`. Uppercase project, a dash, then a number with no leading zero.
pub fn task_key_ok(key: &str) -> bool {
    let Some((proj, num)) = key.split_once('-') else {
        return false;
    };
    let mut chars = proj.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() || proj.len() > 8 {
        return false;
    }
    if !chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return false;
    }
    if num.is_empty() || num.len() > 6 || num.starts_with('0') {
        return false;
    }
    num.chars().all(|c| c.is_ascii_digit())
}

/// Extra worktrees from `git worktree list --porcelain`. The checkout itself is left out.
pub fn child_worktrees(porcelain: &str, repo: &Path) -> Vec<Worktree> {
    let main = repo.to_string_lossy();
    let mut out = Vec::new();
    let mut path = String::new();
    let mut branch = String::new();
    let flush = |path: &mut String, branch: &mut String, out: &mut Vec<Worktree>| {
        if path.is_empty() {
            return;
        }
        if !same_path(path, &main) {
            let name = worktree_name(path);
            if !name.is_empty() && !out.iter().any(|row| row.name == name) {
                out.push(Worktree {
                    name,
                    branch: branch_label(branch),
                });
            }
        }
        path.clear();
        branch.clear();
    };
    for line in porcelain.lines() {
        if line.is_empty() {
            flush(&mut path, &mut branch, &mut out);
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            if !path.is_empty() {
                flush(&mut path, &mut branch, &mut out);
            }
            path = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("branch ") {
            branch = rest.trim().to_string();
        }
    }
    flush(&mut path, &mut branch, &mut out);
    out
}

fn same_path(left: &str, right: &str) -> bool {
    let fold = |text: &str| {
        text.replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    };
    fold(left) == fold(right)
}

fn worktree_name(path: &str) -> String {
    path.replace('\\', "/")
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .replace('#', "-")
        .to_string()
}

fn branch_label(raw: &str) -> String {
    raw.trim()
        .strip_prefix("refs/heads/")
        .unwrap_or(raw.trim())
        .to_string()
}

pub fn holder_ok(holder: &str) -> bool {
    matches!(holder, "cursor" | "opencode" | "agy" | "claude")
}

pub fn holder_mark(holder: &str) -> &'static str {
    match holder {
        "cursor" => "Cu",
        "opencode" => "Oc",
        "agy" => "Ag",
        "claude" => "Cl",
        _ => "",
    }
}

/// `git status -sb` first line plus porcelain body.
pub fn parse_status_sb(text: &str) -> (String, u32, usize, usize, bool) {
    let mut lines = text.lines();
    let head = lines.next().unwrap_or("");
    let branch = head
        .trim_start_matches("##")
        .split("...")
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let ahead = ahead_count(head);
    let mut dirty = 0;
    let mut untracked = 0;
    let mut unmerged = false;
    for line in lines {
        let t = line.trim_end();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("??") {
            untracked += 1;
        } else {
            dirty += 1;
            let code: String = t.chars().take(2).collect();
            if code.contains('U') || code == "AA" || code == "DD" {
                unmerged = true;
            }
        }
    }
    let branch = if branch.is_empty() {
        "unknown".into()
    } else {
        branch
    };
    (branch, ahead, dirty, untracked, unmerged)
}

fn ahead_count(head: &str) -> u32 {
    let Some(start) = head.find("[ahead ") else {
        return 0;
    };
    let rest = &head[start + "[ahead ".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            id: "celcraft".into(),
            name: "celcraft".into(),
            path: "celcraft".into(),
            has_git: true,
            branch: "master".into(),
            dirty: 0,
            untracked: 0,
            ahead: 0,
            unmerged: false,
            index_lock: false,
            has_agents: true,
            has_claude: true,
            handover_next: String::new(),
            handover_age_hours: None,
            last_commit_age_hours: Some(3),
            commits_14: [0; 14],
            worktrees: Vec::new(),
            task_key: None,
            claimed_by: None,
            pinned: false,
            craft: true,
        }
    }

    #[test]
    fn age_and_holders_and_fourteen_days() {
        let mut young = facts();
        let mut held = facts();
        held.id = "devcraft".into();
        held.claimed_by = Some("agy".into());
        held.last_commit_age_hours = Some(24 * 10);
        let repos = [young.clone(), held];
        assert_eq!(age_bands(&repos), [1, 0, 1, 0, 0]);
        assert_eq!(holder_counts(&repos), [0, 0, 1, 0]);
        young.commits_14[13] = 2;
        assert_eq!(activity(&[young])[13], 2);
        let days = tally_iso_days("2026-11-14\n2026-11-14\n2026-11-01\nnope\n", "2026-11-14");
        assert_eq!(days[13], 2);
        assert_eq!(days[0], 1);
        assert_eq!(tally_iso_days("2026-10-01\n", "2026-11-14")[0], 0);
        assert!(task_key_ok("BC-12"));
        assert!(!task_key_ok("bc-12"));
        assert!(!task_key_ok("BC-01"));
        let porcelain = "\
worktree D:/Dev/repos/celcraft
HEAD abc
branch refs/heads/main

worktree D:/Dev/repos/celcraft-fix
HEAD def
branch refs/heads/fix
";
        let children = child_worktrees(porcelain, Path::new("D:/Dev/repos/celcraft"));
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "celcraft-fix");
        assert_eq!(children[0].branch, "fix");
    }

    #[test]
    fn recent_clean_commit_is_done() {
        assert_eq!(column(&facts()), DONE);
    }

    #[test]
    fn a_claim_is_active_even_when_the_commit_is_new() {
        let mut f = facts();
        f.claimed_by = Some("claude".into());
        assert_eq!(column(&f), ACTIVE);
    }

    #[test]
    fn dirty_tree_is_review() {
        let mut f = facts();
        f.dirty = 2;
        f.last_commit_age_hours = Some(100);
        assert_eq!(column(&f), REVIEW);
    }

    #[test]
    fn index_lock_is_blocked() {
        let mut f = facts();
        f.index_lock = true;
        assert_eq!(column(&f), BLOCKED);
        assert!(block_reason(&f).unwrap().contains("index.lock"));
    }

    #[test]
    fn stale_handover_on_a_dirty_tree_is_blocked() {
        let mut f = facts();
        f.dirty = 1;
        f.handover_age_hours = Some(24 * 8);
        assert_eq!(column(&f), BLOCKED);
    }

    #[test]
    fn pinned_clean_repo_is_ready_and_shown() {
        let mut f = facts();
        f.last_commit_age_hours = Some(100);
        f.pinned = true;
        assert_eq!(column(&f), READY);
        assert!(shown(&f));
    }

    #[test]
    fn old_clean_repo_is_quiet_and_hidden() {
        let mut f = facts();
        f.last_commit_age_hours = Some(24 * 90);
        assert_eq!(column(&f), QUIET);
        assert!(!shown(&f));
    }

    #[test]
    fn moves() {
        assert_eq!(allow(ACTIVE, false).unwrap(), Move::Claim);
        assert_eq!(allow(DONE, false).unwrap(), Move::Release);
        assert_eq!(allow(INBOX, false).unwrap(), Move::Snooze);
        assert!(allow(BLOCKED, false).is_err());
        assert!(allow(REVIEW, false).is_err());
        assert!(allow(ACTIVE, true).is_err());
        assert_eq!(allow(DONE, true).unwrap(), Move::Snooze);
    }

    #[test]
    fn status_sb_parses_ahead_and_conflicts() {
        let text = "## master...origin/master [ahead 2]\n M a.rs\nUU b.rs\n?? c.rs\n";
        let (branch, ahead, dirty, untracked, unmerged) = parse_status_sb(text);
        assert_eq!(branch, "master");
        assert_eq!(ahead, 2);
        assert_eq!(dirty, 2);
        assert_eq!(untracked, 1);
        assert!(unmerged);
    }
}
