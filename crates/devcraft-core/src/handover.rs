//! The next step from a HANDOVER.md, and how old the file is.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn next_step(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim().trim_start_matches('#').trim();
        let head = t.to_ascii_lowercase();
        if head.starts_with("next") {
            for follow in lines.iter().skip(i + 1) {
                let step = follow.trim().trim_start_matches(['-', '*', ' ']);
                if !step.is_empty() && !step.starts_with('#') {
                    return step.to_string();
                }
            }
        }
    }
    String::new()
}

pub fn age_hours(path: &Path, now: SystemTime) -> Option<u64> {
    let modified = path.metadata().ok()?.modified().ok()?;
    let now_s = now.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let then_s = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(now_s.saturating_sub(then_s) / 3600)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_line_under_the_next_heading() {
        let text = "# Handover\n\n## What was done\n- scanned\n\n## Next step\n- claim the repo\n";
        assert_eq!(next_step(text), "claim the repo");
    }

    #[test]
    fn empty_when_there_is_no_next_heading() {
        assert_eq!(next_step("# Notes\n\n- nothing"), "");
    }
}
