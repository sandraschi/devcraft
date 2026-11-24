//! Server-rendered mobile pages for the Tailnet companion. No JavaScript:
//! every interaction is a link or a form POST, so it works in any iPad browser.

use devcraft_core::board;
use devcraft_core::cockpit::Cockpit;
use serde_json::Value;

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn css() -> &'static str {
    "body{font-family:system-ui,-apple-system,sans-serif;background:linear-gradient(180deg,#0b1220,#111c33);\
     color:#e2e8f0;margin:0;padding:16px 16px 96px;min-height:100vh}\
     h1{font-size:24px;margin:4px 0 12px}h2{font-size:15px;text-transform:uppercase;letter-spacing:.08em;\
     color:#94a3b8;margin:20px 0 8px}\
     .kpis{display:grid;grid-template-columns:1fr 1fr;gap:10px}\
     .kpi{background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.08);border-radius:14px;padding:12px}\
     .kpi b{font-size:26px;display:block}.kpi span{font-size:12px;color:#94a3b8}\
     .card{background:rgba(255,255,255,.05);border:1px solid rgba(255,255,255,.08);border-radius:14px;padding:12px;margin:8px 0}\
     .row{display:flex;align-items:center;gap:10px}\
     .grow{flex:1;min-width:0}.muted{color:#94a3b8;font-size:13px}.small{font-size:13px}\
     .pill{display:inline-block;font-size:12px;font-weight:700;padding:3px 10px;border-radius:99px;margin:2px 4px 2px 0}\
     .ok{background:rgba(52,211,153,.15);color:#34d399}.warn{background:rgba(251,191,36,.15);color:#fbbf24}\
     .danger{background:rgba(248,113,113,.15);color:#f87171}.info{background:rgba(56,189,248,.15);color:#38bdf8}\
     .neutral{background:rgba(148,163,184,.15);color:#94a3b8}.accent{background:rgba(167,139,250,.18);color:#a78bfa}\
     button,input,select{font-size:17px;padding:12px 14px;border-radius:12px;border:0}\
     button{background:#38bdf8;color:#082f49;font-weight:700}\
     button.ghost{background:rgba(255,255,255,.08);color:#e2e8f0}\
     input[type=text],input[name=title]{background:#1e293b;color:#e2e8f0;width:100%;box-sizing:border-box}\
     form.inline{display:flex;gap:8px;margin:8px 0}form.inline input{flex:1}\
     .banner{border-radius:14px;padding:12px;margin:8px 0;font-weight:600}\
     .banner.warn{background:rgba(251,191,36,.12);border:1px solid rgba(251,191,36,.4)}\
     .banner.ok{background:rgba(52,211,153,.10);border:1px solid rgba(52,211,153,.35)}\
     nav{position:fixed;left:0;right:0;bottom:0;display:flex;background:rgba(11,18,32,.96);\
     border-top:1px solid rgba(255,255,255,.1);padding:8px 4px calc(8px + env(safe-area-inset-bottom))}\
     nav a{flex:1;text-align:center;color:#94a3b8;text-decoration:none;font-size:12px;font-weight:600;padding:8px 0}\
     nav a.on{color:#38bdf8}a{color:#38bdf8}\
     .col{border-left:4px solid #94a3b8;padding-left:10px;margin:14px 0}\
     .col-inbox{border-color:#f87171}.col-ready{border-color:#38bdf8}.col-active{border-color:#a78bfa}\
     .col-review{border-color:#fbbf24}.col-blocked{border-color:#f87171}.col-done{border-color:#34d399}.col-quiet{border-color:#64748b}\
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

fn shell(token: &str, active: &str, title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta name=\"viewport\" content=\"width=device-width,initial-scale=1,viewport-fit=cover\">\
         <title>DevCraft {title}</title><style>{}</style></head>\
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
pub fn today(pit: &Cockpit, token: &str, open_todos: usize) -> String {
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
    shell(token, "today", "Today", &body)
}

/// Board: columns with cards.
pub fn board(pit: &Cockpit, token: &str) -> String {
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
    shell(token, "board", "Board", &body)
}

/// Todos: full list with add/done/clear.
pub fn todos(pit: &Cockpit, token: &str) -> String {
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
    shell(token, "todos", "Todos", &body)
}

/// Git: live checkouts.
pub fn git(pit: &Cockpit, token: &str) -> String {
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
    shell(token, "git", "Git", &body)
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
pub fn github(pit: &Cockpit, token: &str) -> String {
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
    shell(token, "more", "GitHub", &body)
}

/// Crafts: app checkouts with binary/handover state.
pub fn crafts(pit: &Cockpit, token: &str) -> String {
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
    shell(token, "more", "Crafts", &body)
}

/// Tasks: scheduled fleet jobs.
pub fn tasks(token: &str, value: &Value) -> String {
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
    shell(token, "more", "Tasks", &body)
}

/// Reports: fleet reports with freshness.
pub fn reports(pit: &Cockpit, token: &str) -> String {
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
    shell(token, "more", "Reports", &body)
}

/// More: index of secondary pages + scan controls.
pub fn more(pit: &Cockpit, token: &str) -> String {
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
    body.push_str(&format!(
        "<form method=\"post\" action=\"/api/refresh?token={token}\">\
         <button type=\"submit\">Rescan fleet now</button></form>\
         <p><a href=\"/api/board?token={token}\">Board JSON</a> · \
         <a href=\"/api/digest?token={token}\">Digest JSON</a> · \
         <a href=\"/api/todos?token={token}\">Todos JSON</a></p>"
    ));
    shell(token, "more", "More", &body)
}
