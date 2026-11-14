//! The morning digest already written to `operations/fleet-status.md`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GhLine {
    pub repo: String,
    pub kind: String,
    pub number: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirtyLine {
    pub repo: String,
    pub files: u32,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriftLine {
    pub repo: String,
    pub ahead: u32,
    pub behind: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Digest {
    pub present: bool,
    pub generated: String,
    pub repos_scanned: Option<u32>,
    pub open_prs: Option<u32>,
    pub open_issues: Option<u32>,
    pub stale_pr_count: Option<u32>,
    pub stale_issue_count: Option<u32>,
    pub discussion_count: Option<u32>,
    pub unanswered_count: Option<u32>,
    pub dirty_worktrees: Option<u32>,
    pub sync_drift: Option<u32>,
    pub needs_reply: Vec<GhLine>,
    pub stale_prs: Vec<GhLine>,
    pub stale_issues: Vec<GhLine>,
    pub unanswered: Vec<GhLine>,
    pub dirty: Vec<DirtyLine>,
    pub drift: Vec<DriftLine>,
    pub errors: Vec<String>,
    pub dirty_more: u32,
    pub note: String,
}

impl Digest {
    pub fn missing(note: impl Into<String>) -> Digest {
        Digest {
            present: false,
            generated: String::new(),
            repos_scanned: None,
            open_prs: None,
            open_issues: None,
            stale_pr_count: None,
            stale_issue_count: None,
            discussion_count: None,
            unanswered_count: None,
            dirty_worktrees: None,
            sync_drift: None,
            needs_reply: Vec::new(),
            stale_prs: Vec::new(),
            stale_issues: Vec::new(),
            unanswered: Vec::new(),
            dirty: Vec::new(),
            drift: Vec::new(),
            errors: Vec::new(),
            dirty_more: 0,
            note: note.into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Reply,
    StalePr,
    StaleIssue,
    Qa,
    Dirty,
    Drift,
    Errors,
}

pub fn parse(text: &str) -> Digest {
    let mut digest = Digest {
        present: true,
        ..Digest::missing("")
    };
    digest.present = true;
    digest.note.clear();
    let mut section = Section::None;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("Generated:") {
            digest.generated = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = t.strip_prefix("Repos scanned:") {
            digest.repos_scanned = rest.trim().parse().ok();
            continue;
        }
        if let Some(heading) = t.strip_prefix("## ") {
            section = section_of(heading);
            continue;
        }
        if t.starts_with("- Open PRs:") {
            digest.open_prs = bold_number(t);
        } else if t.starts_with("- Open issues:") {
            digest.open_issues = bold_number(t);
        } else if t.starts_with("- Stale PRs") {
            digest.stale_pr_count = bold_number(t);
        } else if t.starts_with("- Stale issues:") {
            digest.stale_issue_count = bold_number(t);
        } else if t.starts_with("- Open discussions:") {
            digest.discussion_count = bold_number(t);
        } else if t.starts_with("- Unanswered") {
            digest.unanswered_count = bold_number(t);
        } else if t.starts_with("- Dirty worktrees") {
            digest.dirty_worktrees = bold_number(t);
        } else if t.starts_with("- Sync drift") {
            digest.sync_drift = bold_number(t);
        } else if let Some(body) = t.strip_prefix("- ") {
            match section {
                Section::Reply => digest.needs_reply.push(parse_gh(body)),
                Section::StalePr => digest.stale_prs.push(parse_gh(body)),
                Section::StaleIssue => digest.stale_issues.push(parse_gh(body)),
                Section::Qa => digest.unanswered.push(parse_gh(body)),
                Section::Dirty => {
                    if let Some(row) = parse_dirty(body) {
                        digest.dirty.push(row);
                    } else if let Some(n) = more_count(body) {
                        digest.dirty_more = n;
                    }
                }
                Section::Drift => {
                    if let Some(row) = parse_drift(body) {
                        digest.drift.push(row);
                    }
                }
                Section::Errors => digest.errors.push(body.to_string()),
                Section::None => {}
            }
        } else if section == Section::Errors {
            digest.errors.push(t.to_string());
        }
    }
    if digest.generated.is_empty() && digest.open_prs.is_none() {
        digest.note = "the file is not a morning digest".into();
        digest.present = false;
    }
    digest
}

fn section_of(heading: &str) -> Section {
    let h = heading.to_ascii_lowercase();
    if h.contains("needs first reply") {
        Section::Reply
    } else if h.contains("stale pr") {
        Section::StalePr
    } else if h.contains("stale issue") {
        Section::StaleIssue
    } else if h.contains("unanswered") {
        Section::Qa
    } else if h.contains("uncommitted") || h.contains("hygiene") {
        Section::Dirty
    } else if h.contains("sync drift") {
        Section::Drift
    } else if h.contains("scan error") {
        Section::Errors
    } else {
        Section::None
    }
}

fn bold_split(body: &str) -> (String, &str) {
    let Some(inner) = body.strip_prefix("**") else {
        return (String::new(), body);
    };
    let Some(end) = inner.find("**") else {
        return (String::new(), body);
    };
    (inner[..end].to_string(), inner[end + 2..].trim())
}

pub fn parse_gh(body: &str) -> GhLine {
    let (repo, rest) = bold_split(body);
    let mut kind = String::new();
    let mut number = String::new();
    let mut title = rest.to_string();
    if let Some(hash) = rest.find('#') {
        kind = rest[..hash]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        let after = &rest[hash + 1..];
        number = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        title = after[number.len()..]
            .trim()
            .trim_start_matches('-')
            .trim()
            .to_string();
    }
    let mut url = String::new();
    if let Some(idx) = title.rfind("http://").or_else(|| title.rfind("https://")) {
        url = title[idx..].trim().to_string();
        title = title[..idx].trim().to_string();
    }
    GhLine {
        repo,
        kind,
        number,
        title,
        url,
    }
}

fn parse_dirty(body: &str) -> Option<DirtyLine> {
    let (repo, rest) = bold_split(body);
    if repo.is_empty() {
        return None;
    }
    let files = rest
        .split_whitespace()
        .find_map(|w| w.parse::<u32>().ok())
        .unwrap_or(0);
    let detail = rest
        .split_once('(')
        .and_then(|(_, tail)| tail.split(')').next())
        .unwrap_or("")
        .trim()
        .to_string();
    Some(DirtyLine { repo, files, detail })
}

fn parse_drift(body: &str) -> Option<DriftLine> {
    let (repo, rest) = bold_split(body);
    if repo.is_empty() {
        return None;
    }
    Some(DriftLine {
        repo,
        ahead: word_after(rest, "ahead").unwrap_or(0),
        behind: word_after(rest, "behind").unwrap_or(0),
    })
}

fn word_after(text: &str, label: &str) -> Option<u32> {
    let idx = text.find(label)? + label.len();
    let rest = text[idx..].trim();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn more_count(body: &str) -> Option<u32> {
    let rest = body.trim_start_matches('.').trim();
    let mut words = rest.split_whitespace();
    if words.next() != Some("and") {
        return None;
    }
    words.next()?.parse().ok()
}

fn bold_number(line: &str) -> Option<u32> {
    let start = line.find("**")? + 2;
    let rest = line.get(start..)?;
    let end = rest.find("**")?;
    rest[..end].trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_totals_and_the_reply_section() {
        let text = "\
Generated: 2026-11-14T05:08:49+00:00
Repos scanned: 234

## Totals
- Open PRs: **44**
- Open issues: **169**
- Stale PRs (>=7d): **3**
- Stale issues: **18**
- Unanswered Q&A: **2**
- Dirty worktrees (uncommitted/untracked): **55**
- Sync drift (ahead/behind origin): **10**

## Needs first reply (rude-if-ignored)
- **sandraschi/arr-mcp** issue #9 - example title https://github.com/sandraschi/arr-mcp/issues/9
- **sandraschi/dreame-mcp** pr #4 - example pr https://github.com/sandraschi/dreame-mcp/pull/4

## Stale PRs
- **sandraschi/inkscape-mcp** #7 - quiet https://github.com/sandraschi/inkscape-mcp/pull/7

## Local workspace hygiene (uncommitted work)
- **democraft-mcp** - 2 dirty file(s) ( M src/a.py)
- ... and 15 more dirty repos

## Sync drift (ahead/behind origin)
- **docs-mcp** - ahead 0, behind 5

## Scan errors
- sandraschi/health-mcp: issues disabled
";
        let d = parse(text);
        assert!(d.present);
        assert_eq!(d.repos_scanned, Some(234));
        assert_eq!(d.open_prs, Some(44));
        assert_eq!(d.open_issues, Some(169));
        assert_eq!(d.stale_pr_count, Some(3));
        assert_eq!(d.dirty_worktrees, Some(55));
        assert_eq!(d.sync_drift, Some(10));
        assert_eq!(d.needs_reply.len(), 2);
        assert_eq!(d.needs_reply[0].repo, "sandraschi/arr-mcp");
        assert_eq!(d.needs_reply[0].number, "9");
        assert!(d.needs_reply[0].url.contains("issues/9"));
        assert_eq!(d.stale_prs.len(), 1);
        assert_eq!(d.dirty.len(), 1);
        assert_eq!(d.dirty[0].files, 2);
        assert_eq!(d.dirty_more, 15);
        assert_eq!(d.drift[0].behind, 5);
        assert_eq!(d.errors.len(), 1);
    }
}
