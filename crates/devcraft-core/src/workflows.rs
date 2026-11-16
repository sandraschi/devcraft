//! Queue an SOP request. An agent runs the checklist. This file does not.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

struct Sop {
    kind: &'static str,
    title: &'static str,
    path: &'static str,
}

const SOPS: &[Sop] = &[
    Sop {
        kind: "craft-assess",
        title: "Craft app assess and fix",
        path: "mcp-central-docs/patterns/CRAFT_APP_ASSESS_AND_FIX_SOP.md",
    },
    Sop {
        kind: "assfix",
        title: "Repo assess and fix",
        path: "mcp-central-docs/patterns/repo-assess-and-fix.md",
    },
    Sop {
        kind: "gapfind",
        title: "Gapfind",
        path: "mcp-central-docs/patterns/GAPFIND_AUDIT_SOP.md",
    },
    Sop {
        kind: "drift-scan",
        title: "Drift scan",
        path: "mcp-central-docs/patterns/DRIFT_SCAN_SOP.md",
    },
    Sop {
        kind: "quality-check",
        title: "Quality check",
        path: "mcp-central-docs/patterns/QUALITY_CHECK_SOP.md",
    },
    Sop {
        kind: "doc-sync",
        title: "Doc sync",
        path: "mcp-central-docs/patterns/DOC_SYNC_SOP.md",
    },
];

pub fn kinds() -> Vec<&'static str> {
    SOPS.iter().map(|sop| sop.kind).collect()
}

pub fn queue(root: &Path, kind: &str, repo: &str, confirm: bool) -> Result<Value, String> {
    if !confirm {
        return Err("confirm the workflow request".into());
    }
    if !valid_repo(repo) || !root.join(repo).is_dir() {
        return Err("queue a workflow for a checkout that exists".into());
    }
    let Some(sop) = SOPS.iter().find(|sop| sop.kind == kind) else {
        return Err(format!("unknown workflow. known: {}", kinds().join(", ")));
    };
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = root.join(".devcraft").join("workflows");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let file = dir.join(format!("{stamp}-{kind}-{repo}.md"));
    let body = format!(
        "# {title}\n\nRepo: {repo}\nSOP: {path}\n\nThis file is a request. Run the SOP in an agent. DevCraft does not edit the checkout from this button.\n",
        title = sop.title,
        path = sop.path,
    );
    fs::write(&file, body).map_err(|err| err.to_string())?;
    Ok(json!({
        "kind": kind,
        "repo": repo,
        "sop": sop.path,
        "request": file.display().to_string(),
    }))
}

fn valid_repo(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', ':', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn queue_writes_a_request_and_refuses_without_confirm() {
        let root = std::env::temp_dir().join(format!(
            "devcraft-wf-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("celcraft")).unwrap();
        let err = queue(&root, "assfix", "celcraft", false).unwrap_err();
        assert!(err.contains("confirm"));
        let value = queue(&root, "assfix", "celcraft", true).unwrap();
        let path = value["request"].as_str().unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(text.contains("repo-assess-and-fix.md"));
        assert!(text.contains("celcraft"));
        let _ = fs::remove_dir_all(&root);
    }
}
