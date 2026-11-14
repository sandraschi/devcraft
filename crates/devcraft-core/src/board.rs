//! Live fleet board. Columns are computed from the working tree.
//! A drag is accepted or refused here, then the cockpit applies it.

use serde::{Deserialize, Serialize};

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
    pub claimed_by: Option<String>,
    pub pinned: bool,
    pub craft: bool,
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
pub fn shown(f: &Facts) -> bool {
    match column(f) {
        QUIET => false,
        READY => f.pinned || f.handover_age_hours.is_some(),
        _ => true,
    }
}

pub fn allow(to: &str, github: bool) -> Result<Move, String> {
    match to {
        BLOCKED | QUIET => {
            Err("Blocked and Quiet are filled from the working tree.".into())
        }
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
            claimed_by: None,
            pinned: false,
            craft: true,
        }
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
