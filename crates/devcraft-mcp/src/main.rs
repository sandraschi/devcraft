//! DevCraft FastMCP Server & Universal CLI.
//!
//! Follows the MCP 2024-11-05 standard: stdout is 100% JSON-RPC, diagnostics go to stderr.

use clap::{Parser, Subcommand};
use devcraft_core::cockpit::{tool_specs, Cockpit};
use devcraft_core::{
    backup::BackupEngine, fleet::FleetScanner, meta::MetaMcpDepot, ports::PortSentinel,
    resolve_workspace_root, sandbox::ContainerSandbox, services::ServiceSentinel, DevCraftStatus,
    APP_VERSION,
};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "devcraft-mcp")]
#[command(about = "DevCraft: Sovereign Developer Cockpit & Compound MCP Server", version = APP_VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Run as MCP 2024-11-05 stdio server
    #[arg(long)]
    mcp: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Check DevCraft status and active subsystems
    Status,
    /// Audit fleet workspace repos and git health
    Audit {
        #[arg(long, default_value = ".")]
        root: String,
    },
    /// Snapshot files to atomic backup ledger
    Snapshot {
        #[arg(short, long)]
        tag: String,
        #[arg(short, long)]
        desc: String,
        files: Vec<String>,
    },
    /// Inspect socket port reservations and active listeners
    Ports,
    /// Discover MCP servers in workspace
    Discover { query: Option<String> },
    /// Run a cockpit tool (devcraft_status, devcraft_refresh, devcraft_board, ...)
    Tool {
        name: String,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        holder: Option<String>,
        #[arg(long)]
        card: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        ide: Option<String>,
        #[arg(long)]
        path: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    if cli.mcp {
        run_mcp_server();
        return;
    }

    match cli.command {
        Some(Commands::Status) => {
            let status = DevCraftStatus::default();
            println!("{}", serde_json::to_string_pretty(&status).unwrap_or_default());
        }
        Some(Commands::Audit { root }) => {
            let target_root = if root == "." {
                resolve_workspace_root()
            } else {
                PathBuf::from(root)
            };
            let report = FleetScanner::scan_workspace(&target_root);
            println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
        }
        Some(Commands::Snapshot { tag, desc, files }) => {
            let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            let ws = resolve_workspace_root();
            match BackupEngine::create_snapshot(&ws, &paths, &tag, &desc) {
                Ok(rec) => println!("{}", serde_json::to_string_pretty(&rec).unwrap_or_default()),
                Err(e) => eprintln!("Snapshot failed: {}", e),
            }
        }
        Some(Commands::Ports) => {
            let ws = resolve_workspace_root();
            let rep = PortSentinel::audit_ports(&ws);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default());
        }
        Some(Commands::Discover { query }) => {
            let ws = resolve_workspace_root();
            let servers = MetaMcpDepot::discover_servers(&ws, query.as_deref());
            println!("{}", serde_json::to_string_pretty(&servers).unwrap_or_default());
        }
        Some(Commands::Tool {
            name,
            repo,
            holder,
            card,
            to,
            ide,
            path,
        }) => match run_cockpit(
            &name,
            &cockpit_args(repo, holder, card, to, ide, path),
        ) {
            Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
            Err(e) => eprintln!("{e}"),
        },
        None => {
            eprintln!("DevCraft MCP CLI v{}. Use --mcp for stdio server or --help.", APP_VERSION);
        }
    }
}

fn run_mcp_server() {
    eprintln!("DevCraft FastMCP Server starting (MCP 2024-11-05 protocol)...");
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": format!("Parse error: {}", e) }
                });
                let _ = writeln!(out, "{}", err);
                let _ = out.flush();
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let resp = match method {
            "initialize" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "serverInfo": {
                            "name": "devcraft-mcp",
                            "version": APP_VERSION
                        },
                        "capabilities": {
                            "tools": {}
                        }
                    }
                })
            }
            "notifications/initialized" => continue,
            "tools/list" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": listed_tools()
                    }
                })
            }
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(json!({}));
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                let result_value = match name {
                    "devcraft_audit_fleet" => {
                        let root_str = args.get("root").and_then(|r| r.as_str()).unwrap_or("..");
                        let rep = FleetScanner::scan_workspace(Path::new(root_str));
                        json!(rep)
                    }
                    "devcraft_checkpoint_worktree" => {
                        let tag = args.get("tag").and_then(|t| t.as_str()).unwrap_or("agent-checkpoint");
                        let desc = args.get("description").and_then(|d| d.as_str()).unwrap_or("");
                        let files: Vec<PathBuf> = args.get("files")
                            .and_then(|f| f.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(PathBuf::from).collect())
                            .unwrap_or_default();
                        match BackupEngine::create_snapshot(Path::new("."), &files, tag, desc) {
                            Ok(rec) => json!(rec),
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "devcraft_audit_ports" => {
                        let rep = PortSentinel::audit_ports(Path::new(".."));
                        json!(rep)
                    }
                    "devcraft_sandbox_run" => {
                        let repo_str = args.get("repo").and_then(|r| r.as_str()).unwrap_or(".");
                        let cmd = args.get("command").and_then(|c| c.as_str()).unwrap_or("cargo check");
                        let image = args.get("image").and_then(|i| i.as_str());
                        match ContainerSandbox::execute(Path::new(repo_str), image, cmd) {
                            Ok(res) => json!(res),
                            Err(e) => json!({ "error": e }),
                        }
                    }
                    "devcraft_restart_service" => {
                        let sname = args.get("service_name").and_then(|s| s.as_str()).unwrap_or("");
                        let port = args.get("expected_port").and_then(|p| p.as_u64()).map(|p| p as u16);
                        let res = ServiceSentinel::restart_service_safely(sname, port);
                        json!(res)
                    }
                    "devcraft_discover_tools" => {
                        let q = args.get("query").and_then(|s| s.as_str());
                        let servers = MetaMcpDepot::discover_servers(Path::new(".."), q);
                        json!(servers)
                    }
                    other => match run_cockpit(other, &args) {
                        Ok(v) => v,
                        Err(e) => json!({ "error": e }),
                    },
                };

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string_pretty(&result_value).unwrap_or_default()
                            }
                        ]
                    }
                })
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("Method not found: {}", method) }
                })
            }
        };

        let _ = writeln!(out, "{}", resp);
        let _ = out.flush();
    }
}

fn listed_tools() -> Vec<Value> {
    let mut tools = vec![
        json!({
            "name": "devcraft_audit_fleet",
            "description": "Audit workspace repositories, git status, and AGENTS.md.",
            "inputSchema": { "type": "object", "properties": { "root": { "type": "string" } } }
        }),
        json!({
            "name": "devcraft_checkpoint_worktree",
            "description": "Snapshot files into the time-machine ledger.",
            "inputSchema": { "type": "object", "properties": {
                "tag": { "type": "string" },
                "description": { "type": "string" },
                "files": { "type": "array", "items": { "type": "string" } }
            }, "required": ["tag", "description", "files"] }
        }),
        json!({
            "name": "devcraft_audit_ports",
            "description": "Audit reserved ports against listeners.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "devcraft_sandbox_run",
            "description": "Run a command in Docker or a local subprocess.",
            "inputSchema": { "type": "object", "properties": {
                "repo": { "type": "string" },
                "command": { "type": "string" },
                "image": { "type": "string" }
            }, "required": ["repo", "command"] }
        }),
        json!({
            "name": "devcraft_restart_service",
            "description": "Restart a Windows service with sc.exe.",
            "inputSchema": { "type": "object", "properties": {
                "service_name": { "type": "string" },
                "expected_port": { "type": "integer" }
            }, "required": ["service_name"] }
        }),
        json!({
            "name": "devcraft_discover_tools",
            "description": "Find MCP servers in the workspace by keyword.",
            "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } }
        }),
    ];
    tools.extend(tool_specs());
    tools
}

fn cockpit_args(
    repo: Option<String>,
    holder: Option<String>,
    card: Option<String>,
    to: Option<String>,
    ide: Option<String>,
    path: Option<String>,
) -> Value {
    let mut args = serde_json::Map::new();
    if let Some(v) = repo {
        args.insert("repo".into(), json!(v));
    }
    if let Some(v) = holder {
        args.insert("holder".into(), json!(v));
    }
    if let Some(v) = card {
        args.insert("card".into(), json!(v));
    }
    if let Some(v) = to {
        args.insert("to".into(), json!(v));
    }
    if let Some(v) = ide {
        args.insert("ide".into(), json!(v));
    }
    if let Some(v) = path {
        args.insert("path".into(), json!(v));
    }
    Value::Object(args)
}

fn run_cockpit(name: &str, args: &Value) -> Result<Value, String> {
    let mut pit = Cockpit::from_env()?;
    pit.call(name, args)
}
