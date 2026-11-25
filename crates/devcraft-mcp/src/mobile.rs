//! Server-rendered mobile pages for the Tailnet companion. No JavaScript:
//! every interaction is a link or a form POST, so it works in any iPad browser.
//!
//! Theming follows the DJCraft touch-remote scheme: every color keys off a
//! `:root` variable block. The block is picked from the desktop `skin.json`
//! (same file the egui SkinState writes) with a `?skin=` override.

use devcraft_core::board;
use devcraft_core::cockpit::Cockpit;
use serde_json::Value;
use std::path::PathBuf;

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Desktop skin name from `%LOCALAPPDATA%/devcraft/skin.json`, "" when unreadable.
fn desktop_skin() -> String {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    let text = std::fs::read_to_string(PathBuf::from(base).join("devcraft").join("skin.json"))
        .unwrap_or_default();
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| v.get("skin").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn theme_block(
    bg0: &str,
    bg1: &str,
    text: &str,
    muted: &str,
    accent: &str,
    on_accent: &str,
    input: &str,
    card: &str,
    border: &str,
) -> String {
    format!(
        ":root{{--bg0:{bg0};--bg1:{bg1};--text:{text};--muted:{muted};\
         --a:{accent};--on-a:{on_accent};--input:{input};--card:{card};--border:{border};\
         --ok:#34d399;--warn:#fbbf8;--danger:#f87171;--info:#38bdf8;--violet:#a78bfa;}}"
    )
}

/// `:root` variable block for a skin query (`?skin=`), the desktop skin, or the default.
pub fn vars_for(query_skin: &str) -> String {
    let q = query_skin.trim().to_lowercase();
    let key = if q.is_empty() {
        desktop_skin().to_lowercase()
    } else {
        q
    };
    let (bg0, bg1, text, muted, accent, on_accent, input, card, border) = match key.as_str() {
        "cockpitdark" | "cockpit dark" | "cockpit" => (
            "#0b1017",
            "#12243c",
            "#f3f6fa",
            "#bac6d6",
            "#60a5fa",
            "#082f49",
            "#1c2940",
            "rgba(255,255,255,.05)",
            "rgba(255,255,255,.08)",
        ),
        "clubmidnight" | "club midnight" | "midnight" => (
            "#080c14",
            "#0f1e38",
            "#f0f6ff",
            "#aabcd8",
            "#38bdf8",
            "#082f49",
            "#14233c",
            "rgba(255,255,255,.05)",
            "rgba(255,255,255,.08)",
        ),
        "carbondeck" | "carbon deck" | "carbon" => (
            "#0e1014",
            "#1c222a",
            "#f5f7fa",
            "#b0b8c4",
            "#14b8a6",
            "#082f2b",
            "#232b36",
            "rgba(255,255,255,.05)",
            "rgba(255,255,255,.10)",
        ),
        "shockingpink" | "shocking pink" | "shocking pink club" | "pink" => (
            "#140412",
            "#2c0a26",
            "#fff5fc",
            "#ffb6e0",
            "#ff1493",
            "#2b0218",
            "#380e30",
            "rgba(255,255,255,.06)",
            "rgba(255,20,147,.35)",
        ),
        "synthwaveoutrun" | "synthwave outrun" | "synthwave" => (
            "#10081e",
            "#221244",
            "#fcf6ff",
            "#d4bceb",
            "#22d3ee",
            "#083344",
            "#2a1550",
            "rgba(255,255,255,.05)",
            "rgba(34,211,238,.30)",
        ),
        "lcarsstarfleet" | "star trek lcars" | "lcars" => (
            "#06060a",
            "#141432",
            "#fff4e0",
            "#d6c2aa",
            "#ff9933",
            "#231303",
            "#161624",
            "rgba(255,255,255,.05)",
            "rgba(255,153,51,.35)",
        ),
        "baroquegold" | "baroque velvet & gold" | "baroque velvet" | "baroque" => (
            "#12090e",
            "#2c1620",
            "#fcf6e8",
            "#dccaaa",
            "#d4af37",
            "#231a03",
            "#2a1420",
            "rgba(255,255,255,.05)",
            "rgba(212,175,55,.35)",
        ),
        "daylightstudio" | "daylight studio (light)" | "daylight studio" | "daylight" => (
            "#eef2f7",
            "#dfe6ef",
            "#0b1220",
            "#334155",
            "#1d4ed8",
            "#ffffff",
            "#ffffff",
            "rgba(255,255,255,.75)",
            "rgba(15,23,42,.14)",
        ),
        _ => (
            "#090a0e",
            "#111c33",
            "#f4f6fa",
            "#a8b0c0",
            "#38bdf8",
            "#082f49",
            "#1e293b",
            "rgba(255,255,255,.05)",
            "rgba(255,255,255,.08)",
        ),
    };
    theme_block(
        bg0, bg1, text, muted, accent, on_accent, input, card, border,
    )
}

/// Skin picker entries: query value + display label.
pub const SKINS: &[(&str, &str)] = &[
    ("Battle", "Battle Console"),
    ("Cockpit", "Cockpit Dark"),
    ("Midnight", "Club Midnight"),
    ("Carbon", "Carbon Deck"),
    ("Pink", "Shocking Pink Club"),
    ("Synthwave", "Synthwave Outrun"),
    ("LCARS", "Star Trek LCARS"),
    ("Baroque", "Baroque Velvet & Gold"),
    ("Daylight", "Daylight Studio"),
];

fn css() -> &'static str {
    "body{font-family:system-ui,-apple-system,sans-serif;background:linear-gradient(180deg,var(--bg0),var(--bg1));\
     color:var(--text);margin:0;padding:16px 16px 96px;min-height:100vh;-webkit-user-select:none;user-select:none}\
     h1{font-size:24px;margin:4px 0 12px}h2{font-size:15px;text-transform:uppercase;letter-spacing:.08em;\
     color:var(--muted);margin:20px 0 8px}\
     .kpis{display:grid;grid-template-columns:1fr 1fr;gap:10px}\
     .kpi{background:var(--card);border:1px solid var(--border);border-radius:14px;padding:12px}\
     .kpi b{font-size:26px;display:block}.kpi span{font-size:12px;color:var(--muted)}\
     .card{background:var(--card);border:1px solid var(--border);border-radius:14px;padding:12px;margin:8px 0}\
     .row{display:flex;align-items:center;gap:10px}\
     .grow{flex:1;min-width:0}.muted{color:var(--muted);font-size:13px}.small{font-size:13px}\
     .pill{display:inline-block;font-size:12px;font-weight:700;padding:3px 10px;border-radius:99px;margin:2px 4px 2px 0}\
     .ok{background:rgba(52,211,153,.15);color:var(--ok)}.warn{background:rgba(251,191,36,.15);color:var(--warn)}\
     .danger{background:rgba(248,113,113,.15);color:var(--danger)}.info{background:rgba(56,189,248,.15);color:var(--info)}\
     .neutral{background:rgba(148,163,184,.15);color:var(--muted)}.accent{background:rgba(167,139,250,.18);color:var(--violet)}\
     button,input,select{font-size:17px;padding:12px 14px;border-radius:12px;border:0}\
     button{background:var(--a);color:var(--on-a);font-weight:700;touch-action:manipulation}\
     button.ghost{background:var(--card);color:var(--text);border:1px solid var(--border)}\
     input[type=text],input[name=title]{background:var(--input);color:var(--text);width:100%;box-sizing:border-box}\
     form.inline{display:flex;gap:8px;margin:8px 0}form.inline input{flex:1}\
     .banner{border-radius:14px;padding:12px;margin:8px 0;font-weight:600}\
     .banner.warn{background:rgba(251,191,36,.12);border:1px solid rgba(251,191,36,.4)}\
     .banner.ok{background:rgba(52,211,153,.10);border:1px solid rgba(52,211,153,.35)}\
     nav{position:fixed;left:0;right:0;bottom:0;display:flex;background:var(--bg0);\
     border-top:1px solid var(--border);padding:8px 4px calc(8px + env(safe-area-inset-bottom))}\
     nav a{flex:1;text-align:center;color:var(--muted);text-decoration:none;font-size:12px;font-weight:600;padding:8px 0}\
     nav a.on{color:var(--a)}a{color:var(--a)}\
     .skins{display:flex;flex-wrap:wrap;gap:8px}\
     .skins a{border:1px solid var(--border);border-radius:99px;padding:8px 14px;text-decoration:none;font-size:14px;font-weight:600}\
     .col{border-left:4px solid var(--muted);padding-left:10px;margin:14px 0}\
     .col-inbox{border-color:var(--danger)}.col-ready{border-color:var(--info)}.col-active{border-color:var(--violet)}\
     .col-review{border-color:var(--warn)}.col-blocked{border-color:var(--danger)}.col-done{border-color:var(--ok)}.col-quiet{border-color:var(--muted)}\
     .top{display:flex;align-items:center;gap:8px}.top h1{flex:1}"
}

fn nav(token: &str, active: &str) -> String {
    let tabs = [
        ("today", "Today", "/"),
        ("board", "Board", "/board"),
        ("todos", "Todos", "/todos"),
        ("git", "Git", "/git"),
        ("more", "More", "/more"),
    ];
    let mut out = String::from("<nav>");
    for (id, label, path) in tabs {
        let on = if id == active { " class=\"on\"" } else { "" };
        out.push_str(&format!("<a{on} href=\"{path}?token={token}\">{label}</a>"));
    }
    out.push_str("</nav>");
    out
}

fn shell(token: &str, active: &str, title: &str, body: &str, vars: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1,viewport-fit=cover\">\
         <meta name=\"apple-mobile-web-app-capable\" content=\"yes\">\
         <meta name=\"apple-mobile-web-app-status-bar-style\" content=\"black-translucent\">\
         <meta name=\"apple-mobile-web-app-title\" content=\"DevCraft\">\
         <link rel=\"manifest\" href=\"/manifest.json\">\
         <title>DevCraft {title}</title><style>{vars}{}</style></head>\
         <body>{body}{}</body></html>",
        css(),
        nav(token, active),
    )
}

fn num(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_else(|| "n/a".into())
}

fn col_class(column: &str) -> &'static str {
    match column {
        "inbox" => "col-inbox",
        "ready" => "col-ready",
        "active" => "col-active",
        "review" => "col-review",
        "blocked" => "col-blocked",
        "done" => "col-done",
        _ => "col-quiet",
    }
}

fn pill_class(column: &str) -> &'static str {
    match column {
        "inbox" | "blocked" => "danger",
        "ready" => "info",
        "active" => "accent",
        "review" => "warn",
        "done" => "ok",
        _ => "neutral",
    }
}

fn scan_line(pit: &Cockpit) -> String {
    if pit.scanned() {
        format!(
            "<p class=\"muted\">Live scan: {} repos. Digest-driven pages work before and after the scan.</p>",
            pit.repos().len()
        )
    } else {
        "<p class=\"muted\">Live git scan still running in the background; digest data below is already complete.</p>"
            .to_string()
    }
}

/// Today: digest KPIs, reply/dirty/drift, todos preview.
pub fn today(pit: &Cockpit, token: &str, open_todos: usize, vars: &str) -> String {
    let d = pit.digest();
    let stale = d.age_hours.is_some_and(|h| h > 26);
    let mut body = format!(
        "<div class=\"top\"><h1>Today</h1>\
         <form method=\"post\" action=\"/api/refresh?token={token}\"><button class=\"ghost\" type=\"submit\">Refresh</button></form></div>"
    );
    if !d.present {
        body.push_str(&format!(
            "<div class=\"banner warn\">No morning digest: {}</div>",
            escape(&d.note)
        ));
    } else if stale {
        body.push_str(&format!(
            "<div class=\"banner warn\">Digest is {}h old (stale after 26h). Generated {}.</div>",
            d.age_hours.unwrap_or(0),
            escape(&d.generated)
        ));
    } else {
        body.push_str(&format!(
            "<div class=\"banner ok\">Digest fresh. Generated {}.</div>",
            escape(&d.generated)
        ));
    }
    body.push_str(&format!(
        "<div class=\"kpis\">\
         <div class=\"kpi\"><b>{}</b><span>Open PRs</span></div>\
         <div class=\"kpi\"><b>{}</b><span>Open issues</span></div>\
         <div class=\"kpi\"><b>{}</b><span>Dirty trees</span></div>\
         <div class=\"kpi\"><b>{}</b><span>Sync drift</span></div>\
         <div class=\"kpi\"><b>{}</b><span>Need a reply</span></div>\
         <div class=\"kpi\"><b>{}</b><span>Stale PRs</span></div>\
         <div class=\"kpi\"><b>{open_todos}</b><span>Open todos</span></div>\
         <div class=\"kpi\"><b>{}</b><span>On the board</span></div>\
         </div>",
        num(d.open_prs),
        num(d.open_issues),
        num(d.dirty_worktrees),
        num(d.sync_drift),
        d.needs_reply.len(),
        num(d.stale_pr_count),
        pit.cards().len(),
    ));
    if !d.needs_reply.is_empty() {
        body.push_str("<h2>Needs a first reply</h2>");
        for line in d.needs_reply.iter().take(8) {
            body.push_str(&format!(
                "<div class=\"card\"><div class=\"small\"><b>{}</b></div>\
                 <div class=\"muted\">{} {} #{}</div></div>",
                escape(&line.title),
                escape(&line.repo),
                escape(&line.kind),
                escape(&line.number),
            ));
        }
    }
    if !d.dirty.is_empty() {
        body.push_str(&format!("<h2>Dirty trees ({})</h2>", d.dirty.len()));
        for line in d.dirty.iter().take(10) {
            body.push_str(&format!(
                "<div class=\"card\"><div class=\"row\"><div class=\"grow\"><b>{}</b>\
                 <div class=\"muted small\">{}</div></div>\
                 <span class=\"pill warn\">{} files</span></div></div>",
                escape(&line.repo),
                escape(&line.detail),
                line.files,
            ));
        }
    }
    if !d.drift.is_empty() {
        body.push_str(&format!("<h2>Sync drift ({})</h2>", d.drift.len()));
        for line in d.drift.iter().take(10) {
            body.push_str(&format!(
                "<div class=\"card small\">{}: ahead {} / behind {}</div>",
                escape(&line.repo),
                line.ahead,
                line.behind,
            ));
        }
    }
    body.push_str(&scan_line(pit));
    body.push_str(&format!(
        "<p><a href=\"/todos?token={token}\">Open todos</a> · \
         <a href=\"/board?token={token}\">Board</a> · \
         <a href=\"/github?token={token}\">GitHub detail</a></p>"
    ));
    shell(token, "today", "Today", &body, vars)
}

/// Board: columns with cards.
pub fn board(pit: &Cockpit, token: &str, vars: &str) -> String {
    let mut body = format!(
        "<div class=\"top\"><h1>Board</h1>\
         <form method=\"post\" action=\"/api/refresh?token={token}\"><button class=\"ghost\" type=\"submit\">Refresh</button></form></div>"
    );
    let cards = pit.cards();
    if cards.is_empty() {
        body.push_str("<div class=\"card\">No cards yet. The git scan is still running, or everything is quiet.</div>");
    }
    for column in ["inbox", "ready", "active", "review", "blocked", "done"] {
        let in_col: Vec<_> = cards.iter().filter(|c| c.column == column).collect();
        if in_col.is_empty() {
            continue;
        }
        body.push_str(&format!(
            "<div class=\"col {}\"><h2>{} ({})</h2>",
            col_class(column),
            escape(column),
            in_col.len()
        ));
        for card in in_col.iter().take(30) {
            let mut pills = format!(
                "<span class=\"pill {}\">{}</span>",
                pill_class(&card.column),
                escape(&card.column)
            );
            if !card.key.is_empty() {
                pills.push_str(&format!(
                    "<span class=\"pill neutral\">{}</span>",
                    escape(&card.key)
                ));
            }
            if let Some(holder) = &card.holder {
                pills.push_str(&format!(
                    "<span class=\"pill accent\">{}</span>",
                    escape(holder)
                ));
            }
            if let Some(reason) = &card.blocked {
                pills.push_str(&format!(
                    "<div class=\"muted small\">{}</div>",
                    escape(reason)
                ));
            }
            body.push_str(&format!(
                "<div class=\"card\"><b>{}</b><div class=\"muted small\">{}</div><div>{pills}</div></div>",
                escape(&card.title),
                escape(&card.detail),
            ));
        }
        if in_col.len() > 30 {
            body.push_str(&format!(
                "<p class=\"muted\">+ {} more in {}</p>",
                in_col.len() - 30,
                escape(column)
            ));
        }
        body.push_str("</div>");
    }
    body.push_str(&scan_line(pit));
    shell(token, "board", "Board", &body, vars)
}

/// Todos: full list with add/done/clear.
pub fn todos(pit: &Cockpit, token: &str, vars: &str) -> String {
    let today = devcraft_core::todos::today_local();
    let mut items = String::new();
    let mut open = 0;
    for todo in devcraft_core::todos::sorted(pit.todos()) {
        if todo.done {
            continue;
        }
        open += 1;
        let due = devcraft_core::todos::due_label(&todo, &today);
        let due = if due.is_empty() {
            String::new()
        } else {
            format!(" <span class=\"muted\">· {}</span>", escape(&due))
        };
        items.push_str(&format!(
            "<div class=\"card\"><form method=\"post\" action=\"/api/todos/done?token={token}\">\
             <div class=\"row\"><button type=\"submit\">Done</button>\
             <input type=\"hidden\" name=\"id\" value=\"{}\">\
             <div class=\"grow\"><b>{}</b>{due}</div></div></form></div>",
            escape(&todo.id),
            escape(&todo.title),
        ));
    }
    if open == 0 {
        items.push_str("<div class=\"card\">Nothing open. Add the first one below.</div>");
    }
    let finished = pit.todos().iter().filter(|t| t.done).count();
    let mut body = format!("<h1>Todos ({open} open)</h1>{items}");
    body.push_str(&format!(
        "<h2>Add</h2><form class=\"inline\" method=\"post\" action=\"/api/todos?token={token}\">\
         <input type=\"text\" name=\"title\" placeholder=\"New todo\" autocomplete=\"off\">\
         <input type=\"hidden\" name=\"due\" value=\"today\">\
         <button type=\"submit\">Add</button></form>"
    ));
    if finished > 0 {
        body.push_str(&format!(
            "<form method=\"post\" action=\"/api/todos/clear?token={token}\">\
             <button class=\"ghost\" type=\"submit\">Clear {finished} finished</button></form>"
        ));
    }
    shell(token, "todos", "Todos", &body, vars)
}

/// Git: live checkouts.
pub fn git(pit: &Cockpit, token: &str, vars: &str) -> String {
    let mut body = format!(
        "<div class=\"top\"><h1>Git</h1>\
         <form method=\"post\" action=\"/api/refresh?token={token}\"><button class=\"ghost\" type=\"submit\">Refresh</button></form></div>"
    );
    let mut rows: Vec<_> = pit.repos().iter().filter(|r| r.has_git).collect();
    rows.sort_by_key(|r| r.name.to_lowercase());
    if rows.is_empty() {
        body.push_str(
            "<div class=\"card\">Scan still running. Rows appear as each checkout is read.</div>",
        );
    }
    for repo in rows.iter().take(120) {
        let column = board::column(repo);
        let state = if repo.dirty > 0 {
            format!("<span class=\"pill warn\">{} dirty</span>", repo.dirty)
        } else {
            String::new()
        };
        let ahead = if repo.ahead > 0 {
            format!("<span class=\"pill info\">+{} ahead</span>", repo.ahead)
        } else {
            String::new()
        };
        let holder = repo.claimed_by.as_ref().map_or(String::new(), |h| {
            format!("<span class=\"pill accent\">{}</span>", escape(h))
        });
        body.push_str(&format!(
            "<div class=\"card\"><div class=\"row\"><div class=\"grow\"><b>{}</b>\
             <div class=\"muted small\">{}</div></div>\
             <span class=\"pill {}\">{}</span></div>\
             <div>{state}{ahead}{holder}</div></div>",
            escape(&repo.name),
            escape(&repo.branch),
            pill_class(column),
            escape(column),
        ));
    }
    if rows.len() > 120 {
        body.push_str(&format!(
            "<p class=\"muted\">+ {} more</p>",
            rows.len() - 120
        ));
    }
    body.push_str(&scan_line(pit));
    shell(token, "git", "Git", &body, vars)
}

fn gh_list(title: &str, rows: &[devcraft_core::digest::GhLine], empty: &str) -> String {
    let mut out = format!("<h2>{title} ({})</h2>", rows.len());
    if rows.is_empty() {
        out.push_str(&format!("<div class=\"card muted\">{empty}</div>"));
    }
    for line in rows.iter().take(30) {
        let link = if line.url.is_empty() {
            String::new()
        } else {
            format!(" <a href=\"{}\">open</a>", escape(&line.url))
        };
        out.push_str(&format!(
            "<div class=\"card\"><b>{}</b>\
             <div class=\"muted small\">{} {} #{}{link}</div></div>",
            escape(&line.title),
            escape(&line.repo),
            escape(&line.kind),
            escape(&line.number),
        ));
    }
    out
}

/// GitHub: digest detail.
pub fn github(pit: &Cockpit, token: &str, vars: &str) -> String {
    let d = pit.digest();
    let mut body = String::from("<h1>GitHub</h1>");
    if !d.present {
        body.push_str(&format!(
            "<div class=\"banner warn\">No morning digest: {}</div>",
            escape(&d.note)
        ));
    } else {
        body.push_str(&format!(
            "<p class=\"muted\">Generated {}. Counts: PRs {}, issues {}, stale PRs {}, stale issues {}.</p>",
            escape(&d.generated),
            num(d.open_prs),
            num(d.open_issues),
            num(d.stale_pr_count),
            num(d.stale_issue_count),
        ));
    }
    body.push_str(&gh_list(
        "Needs a first reply",
        &d.needs_reply,
        "Inbox zero.",
    ));
    body.push_str(&gh_list("Stale PRs", &d.stale_prs, "None."));
    body.push_str(&gh_list("Stale issues", &d.stale_issues, "None."));
    body.push_str(&gh_list("Unanswered questions", &d.unanswered, "None."));
    shell(token, "more", "GitHub", &body, vars)
}

/// Crafts: app checkouts with binary/handover state.
pub fn crafts(pit: &Cockpit, token: &str, vars: &str) -> String {
    let mut body = String::from("<h1>Craft apps</h1>");
    if pit.crafts().is_empty() {
        body.push_str("<div class=\"card\">No craft checkouts found next to this repo.</div>");
    }
    for craft in pit.crafts() {
        let bin = if craft.has_binary {
            "<span class=\"pill ok\">binary</span>"
        } else {
            "<span class=\"pill neutral\">no binary</span>"
        };
        let ho: &str = if craft.has_handover {
            "<span class=\"pill info\">handover</span>"
        } else {
            ""
        };
        let version = if craft.version.is_empty() {
            String::new()
        } else {
            format!(" <span class=\"muted\">v{}</span>", escape(&craft.version))
        };
        body.push_str(&format!(
            "<div class=\"card\"><b>{}</b>{version}<div class=\"muted small\">{}</div><div>{bin}{ho}</div></div>",
            escape(&craft.name),
            escape(&craft.tagline),
        ));
    }
    shell(token, "more", "Crafts", &body, vars)
}

/// Tasks: scheduled fleet jobs.
pub fn tasks(token: &str, value: &Value, vars: &str) -> String {
    let empty = Vec::new();
    let rows = value.as_array().unwrap_or(&empty);
    let mut body = String::from("<h1>Fleet tasks</h1>");
    if rows.is_empty() {
        body.push_str("<div class=\"card\">No scheduled tasks reported.</div>");
    }
    for row in rows {
        let name = row.get("name").and_then(Value::as_str).unwrap_or("");
        let state = row.get("state").and_then(Value::as_str).unwrap_or("");
        let result = row.get("result").and_then(Value::as_str).unwrap_or("");
        let last = row.get("last_run").and_then(Value::as_str).unwrap_or("");
        let tone = match result {
            "ok" => "ok",
            "failed" => "danger",
            _ => "neutral",
        };
        body.push_str(&format!(
            "<div class=\"card\"><b>{}</b>\
             <div class=\"muted small\">{} · last {}</div>\
             <div><span class=\"pill {tone}\">{}</span></div></div>",
            escape(name),
            escape(state),
            escape(last),
            escape(result),
        ));
    }
    shell(token, "more", "Tasks", &body, vars)
}

/// Reports: fleet reports with freshness.
pub fn reports(pit: &Cockpit, token: &str, vars: &str) -> String {
    let mut body = String::from("<h1>Reports</h1>");
    for report in pit.reports() {
        let (pill, label) = if !report.present {
            ("danger", "missing".to_string())
        } else if report.stale {
            ("warn", "stale".to_string())
        } else {
            ("ok", "fresh".to_string())
        };
        let age = report.age_hours.map_or(String::new(), |h| {
            format!(" <span class=\"muted\">· {h}h old</span>")
        });
        body.push_str(&format!(
            "<div class=\"card\"><b>{}</b>{age}<div><span class=\"pill {pill}\">{label}</span></div></div>",
            escape(&report.title),
        ));
    }
    shell(token, "more", "Reports", &body, vars)
}

/// More: index of secondary pages + scan controls + skin picker.
pub fn more(pit: &Cockpit, token: &str, vars: &str) -> String {
    let mut body = format!(
        "<h1>More</h1><div class=\"card\">{}<br>\
         <span class=\"muted\">Digest: {}</span></div>",
        scan_line(pit),
        escape(&pit.digest().generated),
    );
    for (label, path, desc) in [
        ("GitHub detail", "/github", "replies, stale PRs and issues"),
        ("Craft apps", "/crafts", "binaries and handovers"),
        ("Fleet tasks", "/tasks", "scheduled jobs and results"),
        ("Reports", "/reports", "freshness of fleet files"),
    ] {
        body.push_str(&format!(
            "<div class=\"card\"><a href=\"{path}?token={token}\"><b>{label}</b></a>\
             <div class=\"muted small\">{desc}</div></div>"
        ));
    }
    body.push_str("<h2>Skin</h2><div class=\"card\"><div class=\"skins\">");
    for (query, label) in SKINS {
        body.push_str(&format!(
            "<a href=\"/?token={token}&skin={query}\">{label}</a>"
        ));
    }
    body.push_str("</div><p class=\"muted small\">Matches the desktop skin automatically; tap to override.</p></div>");
    body.push_str(&format!(
        "<form method=\"post\" action=\"/api/refresh?token={token}\">\
         <button type=\"submit\">Rescan fleet now</button></form>\
         <p><a href=\"/api/board?token={token}\">Board JSON</a> · \
         <a href=\"/api/digest?token={token}\">Digest JSON</a> · \
         <a href=\"/api/todos?token={token}\">Todos JSON</a></p>"
    ));
    shell(token, "more", "More", &body, vars)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn skins_resolve_and_emit_vars() {
        assert!(vars_for("LCARS").contains("--a:#ff9933"));
        assert!(vars_for("daylight").contains("--a:#1d4ed8"));
        assert!(vars_for("nope").contains("--a:#38bdf8"));
        assert!(vars_for("").contains(":root"));
    }
}
