//! Fleet scanner and git governance module.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub path: PathBuf,
    pub has_git: bool,
    pub has_agents_md: bool,
    pub has_claude_md: bool,
    pub branch: String,
    pub dirty_count: usize,
    pub untracked_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetAuditReport {
    pub total_scanned: usize,
    pub git_repos: usize,
    pub dirty_repos: usize,
    pub missing_agents_md: usize,
    pub repos: Vec<RepoInfo>,
}

pub struct FleetScanner;

impl FleetScanner {
    /// Scan a root directory for repos and assess their git & documentation health.
    pub fn scan_workspace(root: &Path) -> FleetAuditReport {
        let mut repos = Vec::new();
        let entries = match std::fs::read_dir(root) {
            Ok(rd) => rd,
            Err(_) => {
                return FleetAuditReport {
                    total_scanned: 0,
                    git_repos: 0,
                    dirty_repos: 0,
                    missing_agents_md: 0,
                    repos,
                };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = match path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };

            // Ignore hidden directories like .gemini or .git at root
            if name.starts_with('.') && name != ".claude" {
                continue;
            }

            let has_git = path.join(".git").exists();
            let has_agents_md = path.join("AGENTS.md").exists();
            let has_claude_md = path.join("CLAUDE.md").exists();

            let mut branch = String::from("unknown");
            let mut dirty_count = 0;
            let mut untracked_count = 0;

            if has_git {
                // Check branch
                if let Ok(output) = Command::new("git")
                    .args(["-C", &path.to_string_lossy(), "rev-parse", "--abbrev-ref", "HEAD"])
                    .output()
                {
                    if output.status.success() {
                        branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    }
                }

                // Check git status porcelain
                if let Ok(output) = Command::new("git")
                    .args(["-C", &path.to_string_lossy(), "status", "--porcelain"])
                    .output()
                {
                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        for line in stdout.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("??") {
                                untracked_count += 1;
                            } else if !trimmed.is_empty() {
                                dirty_count += 1;
                            }
                        }
                    }
                }
            }

            repos.push(RepoInfo {
                name,
                path,
                has_git,
                has_agents_md,
                has_claude_md,
                branch,
                dirty_count,
                untracked_count,
            });
        }

        repos.sort_by_key(|a| a.name.to_lowercase());

        let git_repos = repos.iter().filter(|r| r.has_git).count();
        let dirty_repos = repos.iter().filter(|r| r.dirty_count > 0 || r.untracked_count > 0).count();
        let missing_agents_md = repos.iter().filter(|r| r.has_git && !r.has_agents_md).count();

        FleetAuditReport {
            total_scanned: repos.len(),
            git_repos,
            dirty_repos,
            missing_agents_md,
            repos,
        }
    }
}
