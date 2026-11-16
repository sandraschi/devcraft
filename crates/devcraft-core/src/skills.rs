//! Skills compiled into the binary. The chat preprompt and MCP read this list.

use serde_json::{json, Value};

pub struct SkillSrc {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

pub static SKILLS: &[SkillSrc] = &[
    SkillSrc {
        name: "devcraft-workflow",
        description: "Use when operating DevCraft pages, the fleet board, ports, tasks, logs, or telemetry.",
        body: include_str!("../../../skills/devcraft-workflow/SKILL.md"),
    },
    SkillSrc {
        name: "devcraft-sops",
        description: "Use when checking craft-app freshness or queueing assfix, gapfind, drift-scan, quality-check, or doc-sync.",
        body: include_str!("../../../skills/devcraft-sops/SKILL.md"),
    },
];

pub fn skills() -> &'static [SkillSrc] {
    SKILLS
}

pub fn skill(name: &str) -> Option<&'static SkillSrc> {
    SKILLS.iter().find(|skill| skill.name == name)
}

pub fn skill_by_uri(uri: &str) -> Result<&'static SkillSrc, String> {
    let rest = uri
        .strip_prefix("skill://")
        .ok_or_else(|| format!("unknown resource: {uri}"))?;
    let name = rest
        .strip_suffix("/SKILL.md")
        .ok_or_else(|| format!("unknown resource: {uri}"))?;
    skill(name).ok_or_else(|| format!("unknown resource: {uri}"))
}

pub fn resources_list() -> Value {
    json!({
        "resources": SKILLS.iter().map(|skill| json!({
            "uri": format!("skill://{}/SKILL.md", skill.name),
            "name": skill.name,
            "description": skill.description,
            "mimeType": "text/markdown",
        })).collect::<Vec<_>>()
    })
}

pub fn resource_read(uri: &str) -> Result<Value, (i32, String)> {
    match skill_by_uri(uri) {
        Ok(skill) => Ok(json!({
            "contents": [{
                "uri": format!("skill://{}/SKILL.md", skill.name),
                "mimeType": "text/markdown",
                "text": skill.body,
            }]
        })),
        Err(message) => Err((-32002, message)),
    }
}

#[derive(Clone, Debug)]
pub struct PromptPart {
    pub name: String,
    pub text: String,
    pub truncated: bool,
}

pub fn assemble_preprompt(
    version: &str,
    today: &str,
    tools: &[(String, String)],
    status: &str,
    selection: &str,
    user_extra: &str,
) -> (String, Vec<PromptPart>) {
    let header = format!(
        "DevCraft {version}. Date {today}.\n\
Answer from the context and tool results.\n\
Say when you do not know.\n\
Never invent tool names, issue keys, or numbers.\n\
Quote the tool result you relied on.\n\
Changing data requires calling a tool, which the user will confirm."
    );
    let skills_text = SKILLS
        .iter()
        .map(|skill| skill.body)
        .collect::<Vec<_>>()
        .join("\n\n");
    let catalogue = tools
        .iter()
        .map(|(name, desc)| format!("- {name}: {desc}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut parts = vec![
        PromptPart {
            name: "header".into(),
            text: header,
            truncated: false,
        },
        cap("skills", skills_text, 40_000),
        cap("tools", catalogue, 4_000),
        cap("status", status.to_string(), 4_000),
        cap("selection", selection.to_string(), 4_000),
        cap("user", user_extra.to_string(), 2_000),
    ];
    let mut used = parts[0].text.len();
    let budget = 48_000;
    for part in parts.iter_mut().skip(1) {
        if used + part.text.len() > budget {
            let room = budget.saturating_sub(used);
            if part.text.len() > room {
                part.text.truncate(room);
                part.text.push_str("\n[truncated]");
                part.truncated = true;
            }
        }
        used += part.text.len();
    }
    let text = parts
        .iter()
        .map(|part| format!("## {}\n{}", part.name, part.text))
        .collect::<Vec<_>>()
        .join("\n\n");
    (text, parts)
}

fn cap(name: &str, mut text: String, limit: usize) -> PromptPart {
    let truncated = text.len() > limit;
    if truncated {
        text.truncate(limit);
        text.push_str("\n[truncated]");
    }
    PromptPart {
        name: name.into(),
        text,
        truncated,
    }
}

pub fn tool_mentions(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in body.split('`') {
        if let Some(name) = chunk.strip_prefix("devcraft_") {
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                out.push(format!("devcraft_{name}"));
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cockpit::tool_specs;

    #[test]
    fn every_skill_folder_is_bundled() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills");
        let mut folders = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            if entry.path().join("SKILL.md").is_file() {
                folders.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        folders.sort();
        let mut bundled: Vec<&str> = SKILLS.iter().map(|skill| skill.name).collect();
        bundled.sort();
        assert_eq!(folders, bundled);
    }

    #[test]
    fn skills_parse_and_name_real_tools() {
        let names: Vec<String> = tool_specs()
            .iter()
            .filter_map(|tool| {
                tool.get("name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .collect();
        let mut total = 0;
        for skill in SKILLS {
            assert!(skill.body.starts_with("---\n"));
            assert!(skill.body.contains(&format!("name: {}", skill.name)));
            assert!(skill.description.chars().count() <= 200);
            assert!(skill.body.len() <= 12_000);
            assert!(!skill.body.contains('\r'));
            assert!(!skill.body.contains('\u{2014}'));
            total += skill.body.len();
            for mention in tool_mentions(skill.body) {
                assert!(names.iter().any(|name| name == &mention), "{mention}");
            }
        }
        assert!(total <= 40_000);
        let err = resource_read("skill://missing/SKILL.md").unwrap_err();
        assert_eq!(err.0, -32002);
        assert!(resource_read("skill://devcraft-workflow/SKILL.md").is_ok());
    }
}
