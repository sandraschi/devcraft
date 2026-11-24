//! Personal todos: quick capture outside the fleet TaskCraft tracker.
//!
//! Stored as `{workspace}/.devcraft/todos.json`. The same file backs the
//! desktop Today strip, the MCP tools, and the Tailnet mobile companion,
//! so all three stay in sync through `reload_light`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// One personal todo. `due` is `today`, `tomorrow`, `YYYY-MM-DD`, or empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub due: String,
    #[serde(default)]
    pub done: bool,
    pub created: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub repo: String,
}

/// Read `todos.json` from the state dir. Missing or broken files read as empty.
pub fn load(state_dir: &Path) -> Vec<Todo> {
    let path = state_dir.join("todos.json");
    let text = fs::read_to_string(&path).unwrap_or_default();
    if text.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&text).unwrap_or_default()
}

/// Write `todos.json` to the state dir, creating it when needed.
pub fn save(state_dir: &Path, todos: &[Todo]) -> Result<(), String> {
    fs::create_dir_all(state_dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(todos).map_err(|e| e.to_string())?;
    fs::write(state_dir.join("todos.json"), text).map_err(|e| e.to_string())
}

/// Today's date in the local timezone as `YYYY-MM-DD`.
pub fn today_local() -> String {
    chrono::DateTime::<chrono::Local>::from(SystemTime::now())
        .format("%Y-%m-%d")
        .to_string()
}

/// Normalize a due string. Accepts `today`, `tomorrow`, `YYYY-MM-DD`, or empty.
pub fn normalize_due(raw: &str) -> Result<String, String> {
    let due = raw.trim().to_lowercase();
    if due.is_empty() || due == "today" || due == "tomorrow" {
        return Ok(due);
    }
    if chrono::NaiveDate::parse_from_str(&due, "%Y-%m-%d").is_ok() {
        return Ok(due);
    }
    Err("due is today, tomorrow, YYYY-MM-DD, or empty".into())
}

/// Add a todo. Titles are capped at 280 chars; empty titles are refused.
pub fn add(
    todos: &mut Vec<Todo>,
    title: &str,
    due: &str,
    detail: &str,
    source: &str,
    repo: &str,
) -> Result<Todo, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("title is required".into());
    }
    if title.chars().count() > 280 {
        return Err("title is capped at 280 chars".into());
    }
    let due = normalize_due(due)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let todo = Todo {
        id: format!("t{nanos:x}"),
        title: title.to_string(),
        detail: detail.trim().to_string(),
        due,
        done: false,
        created: chrono::Utc::now().to_rfc3339(),
        source: source.trim().to_string(),
        repo: repo.trim().to_string(),
    };
    todos.push(todo.clone());
    Ok(todo)
}

/// Mark a todo done. Matches a full id or a unique id prefix.
pub fn complete(todos: &mut [Todo], id: &str) -> Result<Todo, String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("id is required".into());
    }
    let mut hits = todos.iter_mut().filter(|t| t.id.starts_with(id));
    let Some(todo) = hits.next() else {
        return Err(format!("no todo matches {id}"));
    };
    if hits.next().is_some() {
        return Err(format!("{id} matches several todos; use the full id"));
    }
    todo.done = true;
    Ok(todo.clone())
}

/// Drop finished todos. Returns how many were removed.
pub fn clear_done(todos: &mut Vec<Todo>) -> usize {
    let before = todos.len();
    todos.retain(|t| !t.done);
    before - todos.len()
}

/// Open todos first, then done, each newest last.
pub fn sorted(todos: &[Todo]) -> Vec<Todo> {
    let mut open: Vec<Todo> = todos.iter().filter(|t| !t.done).cloned().collect();
    let mut done: Vec<Todo> = todos.iter().filter(|t| t.done).cloned().collect();
    open.append(&mut done);
    open
}

/// Human due label for the Today strip and the mobile page.
pub fn due_label(todo: &Todo, today: &str) -> String {
    match todo.due.as_str() {
        "" => String::new(),
        "today" => "today".into(),
        "tomorrow" => "tomorrow".into(),
        day if day == today => "today".into(),
        day => day.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn add_complete_clear_roundtrip() {
        let mut todos = Vec::new();
        let first = add(&mut todos, "Buy milk", "today", "", "desktop", "").unwrap();
        add(&mut todos, "Read PRD", "", "", "desktop", "").unwrap();
        assert_eq!(todos.len(), 2);
        assert_eq!(first.due, "today");
        let done = complete(&mut todos, &first.id).unwrap();
        assert!(done.done);
        assert_eq!(todos.iter().filter(|t| !t.done).count(), 1);
        assert_eq!(clear_done(&mut todos), 1);
        assert_eq!(todos.len(), 1);
    }

    #[test]
    fn rejects_empty_title_and_bad_due() {
        let mut todos = Vec::new();
        assert!(add(&mut todos, "   ", "", "", "desktop", "").is_err());
        assert!(add(&mut todos, "x", "someday", "", "desktop", "").is_err());
        assert!(add(&mut todos, "x", "2026-13-40", "", "desktop", "").is_err());
        assert!(normalize_due("2026-11-24").is_ok());
        assert!(normalize_due("tomorrow").is_ok());
    }

    #[test]
    fn unique_prefix_completes() {
        let mut todos = Vec::new();
        let first = add(&mut todos, "one", "", "", "desktop", "").unwrap();
        let prefix = first.id[..5].to_string();
        assert!(complete(&mut todos, &prefix).is_ok());
        assert!(complete(&mut todos, "t000").is_err());
    }
}
