//! Read-only craft-app freshness: binaries, help docs, and menu-bar strings.

use serde::Serialize;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct FreshRow {
    pub name: String,
    pub release_exe: bool,
    pub debug_exe: bool,
    pub changelog: bool,
    pub help: bool,
    pub menu_bar: bool,
}

pub fn inspect(path: &Path, name: &str) -> FreshRow {
    FreshRow {
        name: name.to_string(),
        release_exe: has_exe(&path.join("target").join("release")),
        debug_exe: has_exe(&path.join("target").join("debug")),
        changelog: path.join("CHANGELOG.md").is_file(),
        help: path.join("README.md").is_file()
            && (path.join("docs").join("PRD.md").is_file()
                || path.join("docs").join("README.md").is_file()),
        menu_bar: menu_strings(path),
    }
}

fn has_exe(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    })
}

fn menu_strings(root: &Path) -> bool {
    let mut saw_file = false;
    let mut saw_help = false;
    let mut reads = 0;
    for entry in WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
    {
        if reads >= 40 || (saw_file && saw_help) {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if !name.ends_with(".rs") {
            continue;
        }
        let Ok(text) = fs::read_to_string(entry.path()) else {
            continue;
        };
        reads += 1;
        let sample: String = text.chars().take(20_000).collect();
        if sample.contains("File") {
            saw_file = true;
        }
        if sample.contains("Help") {
            saw_help = true;
        }
    }
    saw_file && saw_help
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_checkout_with_help_and_menu_strings_counts() {
        let dir = std::env::temp_dir().join(format!(
            "devcraft-fresh-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::create_dir_all(dir.join("target").join("release")).unwrap();
        fs::write(dir.join("README.md"), "hello\n").unwrap();
        fs::write(dir.join("docs").join("PRD.md"), "prd\n").unwrap();
        fs::write(dir.join("CHANGELOG.md"), "log\n").unwrap();
        fs::write(dir.join("target").join("release").join("app.exe"), "x").unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src").join("main.rs"), "File Help\n").unwrap();
        let row = inspect(&dir, "samplecraft");
        assert!(row.release_exe && row.changelog && row.help && row.menu_bar);
        assert!(!row.debug_exe);
        let _ = fs::remove_dir_all(&dir);
    }
}
