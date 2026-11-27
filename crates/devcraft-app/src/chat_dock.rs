//! Right-hand chat dock. The preprompt is the bundled skills plus live status.
//! Ollama is the only provider this build calls. A missing model is an error.

use devcraft_core::cockpit::{tool_read_only, tool_specs};
use devcraft_core::skills::{self, PromptPart};
use egui::{Layout, RichText, ScrollArea, TextEdit, TextWrapMode, Ui};
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct PendingCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatPersonality {
    #[default]
    FleetArchitect,
    ConcisePeer,
    TokyoNekomimiGal,
    SecurityHawk,
}

impl ChatPersonality {
    pub const ALL: [Self; 4] = [
        Self::FleetArchitect,
        Self::ConcisePeer,
        Self::TokyoNekomimiGal,
        Self::SecurityHawk,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::FleetArchitect => "fleet_architect",
            Self::ConcisePeer => "concise_peer",
            Self::TokyoNekomimiGal => "tokyo_nekomimi",
            Self::SecurityHawk => "security_hawk",
        }
    }

    pub fn from_id(s: &str) -> Self {
        match s {
            "concise_peer" => Self::ConcisePeer,
            "tokyo_nekomimi" => Self::TokyoNekomimiGal,
            "security_hawk" => Self::SecurityHawk,
            _ => Self::FleetArchitect,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FleetArchitect => "Fleet Architect",
            Self::ConcisePeer => "Concise Peer",
            Self::TokyoNekomimiGal => "Tokyo Nekomimi Gal",
            Self::SecurityHawk => "Security Hawk",
        }
    }

    pub fn directive(self) -> &'static str {
        match self {
            Self::FleetArchitect => "Persona: Direct Viennese fleet architect. Clear technical reasoning.",
            Self::ConcisePeer => "Persona: Ultra-terse senior peer. Bullet points only, zero fluff.",
            Self::TokyoNekomimiGal => "Persona: Bright, technically sharp Tokyo CraftApp nekomimi engineer gal. Grounded in real data.",
            Self::SecurityHawk => "Persona: Strict zero-trust security auditor. Highlight risks and invariants first.",
        }
    }
}

pub struct ChatState {
    pub open: bool,
    pub maximized: bool,
    pub model: String,
    pub personality: ChatPersonality,
    pub export_note: Option<String>,
    pub tools_on: bool,
    pub save_transcripts: bool,
    pub user_extra: String,
    pub input: String,
    pub messages: Vec<ChatMessage>,
    pub pending: Option<PendingCall>,
    pub error: Option<String>,
    pub show_prompt: bool,
    pub preprompt: String,
    pub parts: Vec<PromptPart>,
    pub busy: bool,
    pub rounds: u8,
    pub rx: Option<Receiver<Result<Turn, String>>>,
    pub cancel: Arc<AtomicBool>,
    pub discovered: Vec<String>,
    pub discover_note: String,
    pub discover_rx: Option<Receiver<Result<Vec<String>, String>>>,
    pub want_settings: bool,
}

pub struct Turn {
    pub content: String,
    pub calls: Vec<PendingCall>,
}

impl Default for ChatState {
    fn default() -> Self {
        let mut state = Self {
            open: false,
            maximized: false,
            model: String::new(),
            personality: ChatPersonality::default(),
            export_note: None,
            tools_on: true,
            save_transcripts: true,
            user_extra: String::new(),
            input: String::new(),
            messages: vec![ChatMessage {
                role: "assistant".into(),
                content: "Ask about the fleet, a craft app, or an SOP. Skills are already in the prompt. A tool that changes data waits for Allow once.".into(),
            }],
            pending: None,
            error: None,
            show_prompt: false,
            preprompt: String::new(),
            parts: Vec::new(),
            busy: false,
            rounds: 0,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            discovered: Vec::new(),
            discover_note: String::new(),
            discover_rx: None,
            want_settings: false,
        };
        state.load();
        state
    }
}

impl ChatState {
    pub fn load(&mut self) {
        let Ok(text) = fs::read_to_string(config_path()) else {
            return;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        if let Some(model) = value.get("model").and_then(Value::as_str) {
            self.model = model.to_string();
        }
        if let Some(flag) = value.get("tools_on").and_then(Value::as_bool) {
            self.tools_on = flag;
        }
        if let Some(flag) = value.get("save_transcripts").and_then(Value::as_bool) {
            self.save_transcripts = flag;
        }
        if let Some(extra) = value.get("user_extra").and_then(Value::as_str) {
            self.user_extra = extra.to_string();
        }
        if let Some(p) = value.get("personality").and_then(Value::as_str) {
            self.personality = ChatPersonality::from_id(p);
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(
            path,
            serde_json::to_string_pretty(&json!({
                "model": self.model,
                "personality": self.personality.id(),
                "tools_on": self.tools_on,
                "save_transcripts": self.save_transcripts,
                "user_extra": self.user_extra,
            }))
            .unwrap_or_default(),
        );
    }

    pub fn export_markdown(&self) -> String {
        let mut out = format!(
            "# DevCraft AI Chat Transcript\n\n- **Personality:** {}\n- **Model:** {}\n- **Messages:** {}\n\n---\n\n",
            self.personality.label(),
            self.model,
            self.messages.len()
        );
        for m in &self.messages {
            out.push_str(&format!("### {}\n\n{}\n\n", m.role, m.content));
        }
        out
    }

    pub fn export_to_file(&self) -> Result<PathBuf, String> {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let dir = config_path()
            .parent()
            .map(|p| p.join("chats"))
            .unwrap_or_else(|| PathBuf::from("chats"));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("chat_{ts}.md"));
        fs::write(&path, self.export_markdown()).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn refresh_prompt(&mut self, version: &str, status: &str, selection: &str) {
        let tools = tool_specs()
            .iter()
            .filter_map(|tool| {
                let name = tool.get("name")?.as_str()?.to_string();
                let desc = tool.get("description")?.as_str()?.to_string();
                Some((name, desc))
            })
            .collect::<Vec<_>>();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let combined_extra = if self.user_extra.trim().is_empty() {
            self.personality.directive().to_string()
        } else {
            format!(
                "{}\n{}",
                self.personality.directive(),
                self.user_extra.trim()
            )
        };
        let (text, parts) =
            skills::assemble_preprompt(version, &today, &tools, status, selection, &combined_extra);
        self.preprompt = text;
        self.parts = parts;
    }

    pub fn poll(&mut self) -> Option<Result<Turn, String>> {
        if let Some(rx) = &self.discover_rx {
            if let Ok(result) = rx.try_recv() {
                self.discover_rx = None;
                match result {
                    Ok(models) => {
                        self.discovered = models;
                        if self.model.is_empty() {
                            if let Some(model) = self.discovered.first() {
                                self.model = model.clone();
                                self.save();
                            }
                        }
                        self.discover_note = format!("{} local models", self.discovered.len());
                    }
                    Err(err) => self.discover_note = err,
                }
            }
        }
        let Some(rx) = &self.rx else {
            return None;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.busy = false;
                self.rx = None;
                Some(result)
            }
            Err(_) => None,
        }
    }

    pub fn kick_discover(&mut self) {
        if self.discover_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(discover_models());
        });
        self.discover_rx = Some(rx);
    }

    pub fn kick_turn(&mut self) {
        if self.model.trim().is_empty() {
            self.error = Some(
                "No model selected. Open LLM & Settings and pick an installed Ollama model.".into(),
            );
            return;
        }
        self.error = None;
        self.cancel.store(false, Ordering::Relaxed);
        let model = self.model.clone();
        let mut messages = vec![json!({"role": "system", "content": self.preprompt})];
        for message in &self.messages {
            if message.role == "assistant" && message.content.starts_with("Ask about") {
                continue;
            }
            messages.push(json!({"role": message.role, "content": message.content}));
        }
        let tools = if self.tools_on {
            Some(ollama_tools())
        } else {
            None
        };
        let cancel = Arc::clone(&self.cancel);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(ollama_chat(&model, &messages, tools.as_ref(), &cancel));
        });
        self.busy = true;
        self.rx = Some(rx);
    }

    pub fn header(&mut self, ui: &mut Ui) -> HeaderAction {
        let mut action = HeaderAction::None;
        ui.horizontal(|ui| {
            let pill = if self.model.is_empty() {
                "no model".to_string()
            } else {
                format!("Ollama {}", self.model)
            };
            if ui.button(pill).clicked() {
                action = HeaderAction::Settings;
            }
            if ui.button("View prompt").clicked() {
                self.show_prompt = !self.show_prompt;
            }
            if ui
                .button(if self.tools_on {
                    "Tools on"
                } else {
                    "Tools off"
                })
                .clicked()
            {
                self.tools_on = !self.tools_on;
                self.save();
            }
            if ui.button("Clear").clicked() {
                action = HeaderAction::Clear;
            }
            if ui
                .button(if self.maximized { "Dock" } else { "Max" })
                .clicked()
            {
                self.maximized = !self.maximized;
            }
        });
        ui.horizontal_wrapped(|ui| {
            let prev = self.personality;
            egui::ComboBox::from_id_salt("devcraft_chat_personality")
                .selected_text(self.personality.label())
                .width(142.0)
                .show_ui(ui, |ui| {
                    for p in ChatPersonality::ALL {
                        ui.selectable_value(&mut self.personality, p, p.label());
                    }
                });
            if self.personality != prev {
                self.save();
            }
            if ui.small_button("Export .md").clicked() {
                self.export_note = match self.export_to_file() {
                    Ok(p) => Some(format!("Saved: {}", p.display())),
                    Err(e) => Some(format!("Export failed: {e}")),
                };
            }
            if ui.small_button("Copy MD").clicked() {
                ui.ctx().copy_text(self.export_markdown());
                self.export_note = Some("Copied Markdown transcript.".to_string());
            }
        });
        if let Some(ref note) = self.export_note {
            ui.small(note);
        }
        let tokens = self.preprompt.len() / 4;
        ui.label(format!(
            "context ~{tokens} tokens of the prompt, num_ctx 16384"
        ));
        if self.parts.iter().any(|part| part.truncated) {
            ui.label("The lowest prompt parts were truncated.");
        }
        action
    }

    pub fn transcript(&mut self, ui: &mut Ui) -> TranscriptAction {
        let mut action = TranscriptAction::None;
        ScrollArea::vertical()
            .max_height((ui.available_height() - 88.0).max(120.0))
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(TextWrapMode::Wrap);
                if self.show_prompt {
                    ui.label(RichText::new("Prompt").strong());
                    for part in &self.parts {
                        let mark = if part.truncated { " truncated" } else { "" };
                        ui.label(format!("{} {} chars{mark}", part.name, part.text.len()));
                    }
                    ui.label(&self.preprompt);
                }
                for message in &self.messages {
                    ui.label(RichText::new(&message.role).strong());
                    ui.label(&message.content);
                    ui.add_space(6.0);
                }
                if let Some(pending) = &self.pending {
                    ui.label(RichText::new(format!("Confirm {}", pending.name)).strong());
                    ui.label(pending.arguments.to_string());
                    ui.horizontal(|ui| {
                        if ui.button("Allow once").clicked() {
                            action = TranscriptAction::Allow;
                        }
                        if ui.button("Deny").clicked() {
                            action = TranscriptAction::Deny;
                        }
                    });
                }
                if let Some(err) = &self.error {
                    ui.label(RichText::new(err).color(egui::Color32::from_rgb(180, 40, 40)));
                    if ui.button("Settings").clicked() {
                        action = TranscriptAction::Settings;
                    }
                }
                if self.busy {
                    ui.label("Waiting for Ollama");
                    if ui.button("Cancel").clicked() {
                        action = TranscriptAction::Cancel;
                    }
                }
            });
        action
    }

    pub fn input_bar(&mut self, ui: &mut Ui) -> bool {
        let mut send = false;
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Send").clicked() {
                send = true;
            }
            let width = ui.available_width();
            ui.add(
                TextEdit::multiline(&mut self.input)
                    .desired_rows(2)
                    .desired_width(width)
                    .hint_text("Ask about this fleet"),
            );
        });
        send
    }
}

pub enum HeaderAction {
    None,
    Settings,
    Clear,
}

pub enum TranscriptAction {
    None,
    Allow,
    Deny,
    Settings,
    Cancel,
}

fn config_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("devcraft").join("llm.json")
}

fn ollama_tools() -> Value {
    json!(tool_specs()
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.get("name").and_then(Value::as_str).unwrap_or(""),
                    "description": tool.get("description").and_then(Value::as_str).unwrap_or(""),
                    "parameters": tool.get("inputSchema").cloned().unwrap_or_else(|| json!({"type":"object"})),
                }
            })
        })
        .collect::<Vec<_>>())
}

pub fn is_read_only(name: &str) -> bool {
    tool_read_only(name)
}

fn discover_models() -> Result<Vec<String>, String> {
    let text = curl(
        &json!({}),
        "http://127.0.0.1:11434/api/tags",
        2,
        &AtomicBool::new(false),
        true,
    )?;
    let value: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    let mut models = Vec::new();
    if let Some(rows) = value.get("models").and_then(Value::as_array) {
        for row in rows {
            let Some(name) = row.get("name").and_then(Value::as_str) else {
                continue;
            };
            if name.contains("embed") {
                continue;
            }
            models.push(name.to_string());
        }
    }
    Ok(models)
}

fn ollama_chat(
    model: &str,
    messages: &[Value],
    tools: Option<&Value>,
    cancel: &AtomicBool,
) -> Result<Turn, String> {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "think": false,
        "options": { "num_ctx": 16384 }
    });
    if let Some(tools) = tools {
        body["tools"] = tools.clone();
    }
    let text = curl(&body, "http://127.0.0.1:11434/api/chat", 120, cancel, false)?;
    let value: Value = serde_json::from_str(&text).map_err(|err| format!("Ollama reply: {err}"))?;
    let message = value.get("message").cloned().unwrap_or(Value::Null);
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut calls = Vec::new();
    if let Some(rows) = message.get("tool_calls").and_then(Value::as_array) {
        for row in rows {
            let function = row.get("function").cloned().unwrap_or(Value::Null);
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let arguments = function.get("arguments").cloned().unwrap_or(json!({}));
            calls.push(PendingCall { name, arguments });
        }
    }
    if content.is_empty() && calls.is_empty() {
        return Err("Ollama returned an empty reply".into());
    }
    Ok(Turn { content, calls })
}

fn curl(
    body: &Value,
    url: &str,
    max_time: u64,
    cancel: &AtomicBool,
    get: bool,
) -> Result<String, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    let path = std::env::temp_dir().join(format!(
        "devcraft-chat-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    {
        let mut file = fs::File::create(&path).map_err(|err| err.to_string())?;
        file.write_all(body.to_string().as_bytes())
            .map_err(|err| err.to_string())?;
    }
    let mut cmd = Command::new("curl.exe");
    cmd.arg("--silent")
        .arg("--show-error")
        .arg("--connect-timeout")
        .arg("5")
        .arg("--max-time")
        .arg(max_time.to_string());
    if get {
        cmd.arg(url);
    } else {
        cmd.arg("--data-binary")
            .arg(format!("@{}", path.display()))
            .arg("-H")
            .arg("Content-Type: application/json")
            .arg(url);
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("curl did not start: {err}"))?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&path);
            return Err("cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_string(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut stderr);
                }
                let _ = fs::remove_file(&path);
                if !status.success() {
                    return Err(format!("curl failed: {}", stderr.trim()));
                }
                if stdout.trim().is_empty() {
                    return Err("empty reply from Ollama".into());
                }
                return Ok(stdout);
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(40)),
            Err(err) => {
                let _ = fs::remove_file(&path);
                return Err(err.to_string());
            }
        }
    }
}
