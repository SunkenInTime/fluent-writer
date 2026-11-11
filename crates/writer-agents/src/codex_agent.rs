//! Talks to `codex app-server` over JSON-RPC: one ephemeral read-only thread, one turn
//! (Swift `CodexAgent.swift`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde_json::json;

use crate::agent_models::{AgentEvent, AgentSession, EventHandler, ProcessSink};
use crate::json_value::JsonValue;
use crate::line_process::{ExecutableLocator, LineProcess, ProcessEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Initializing,
    StartingThread,
    StartingTurn,
    Streaming,
    Finished,
}

/// Receives each line the agent would write to the server.
pub type SendOverride = Box<dyn FnMut(&str) + Send>;

pub struct CodexAgent {
    executable: Option<PathBuf>,
    process: Option<LineProcess>,
    on_event: Option<EventHandler>,
    next_id: i64,
    pending: HashMap<i64, String>,
    prompt: String,
    thread_id: Option<String>,
    turn_id: Option<String>,
    text: String,
    final_text: Option<String>,
    scratch: Option<PathBuf>,
    phase: Phase,
    /// Lines written to the server; exposed for tests.
    pub transcript: Vec<String>,
    /// Overrides the process transport in tests.
    pub send_override: Option<SendOverride>,
}

impl Default for CodexAgent {
    /// Finds `codex` the way a login shell would.
    fn default() -> Self {
        CodexAgent::new(ExecutableLocator::find("codex", &[]))
    }
}

impl CodexAgent {
    pub fn new(executable: Option<PathBuf>) -> Self {
        CodexAgent {
            executable,
            process: None,
            on_event: None,
            next_id: 1,
            pending: HashMap::new(),
            prompt: String::new(),
            thread_id: None,
            turn_id: None,
            text: String::new(),
            final_text: None,
            scratch: None,
            phase: Phase::Idle,
            transcript: Vec::new(),
            send_override: None,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    // MARK: Protocol

    fn request(&mut self, method: &str, params: serde_json::Value) {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, method.to_string());
        self.write(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).into());
    }

    fn notify(&mut self, method: &str) {
        self.write(json!({"jsonrpc": "2.0", "method": method}).into());
    }

    fn respond(&mut self, id: JsonValue, result: serde_json::Value) {
        self.write(JsonValue::object([
            ("jsonrpc", "2.0".into()),
            ("id", id),
            ("result", result.into()),
        ]));
    }

    fn respond_error(&mut self, id: JsonValue, message: String) {
        self.write(JsonValue::object([
            ("jsonrpc", "2.0".into()),
            ("id", id),
            ("error", json!({"code": -32601, "message": message}).into()),
        ]));
    }

    fn write(&mut self, v: JsonValue) {
        let line = v.serialized();
        self.transcript.push(line.clone());
        if let Some(send) = self.send_override.as_mut() {
            send(&line);
        } else if let Some(p) = &self.process {
            p.send(&line);
        }
    }

    /// Handles one line from the server.
    pub fn receive(&mut self, line: &str) {
        if self.phase == Phase::Finished {
            return;
        }
        let Some(msg) = JsonValue::parse(line) else {
            return;
        };
        let method = msg.get("method").and_then(JsonValue::string_value);
        if let (Some(id), Some(method)) = (msg.get("id"), method) {
            self.handle_server_request(id.clone(), method);
        } else if let Some(id) = msg.get("id").and_then(JsonValue::number_value) {
            self.handle_response(id as i64, &msg);
        } else if let Some(method) = method {
            let params = msg.get("params").cloned().unwrap_or_default();
            self.handle_notification(method, &params);
        }
    }

    fn handle_response(&mut self, id: i64, msg: &JsonValue) {
        let Some(method) = self.pending.remove(&id) else {
            return;
        };
        if let Some(err) = msg.get("error") {
            let message = err
                .get("message")
                .and_then(JsonValue::string_value)
                .unwrap_or("unknown error");
            if method == "turn/interrupt" {
                return;
            }
            self.finish(AgentEvent::Failed(Self::friendly(message)));
            return;
        }
        let result = msg.get("result").cloned().unwrap_or_default();
        match method.as_str() {
            "initialize" => {
                self.notify("initialized");
                self.phase = Phase::StartingThread;
                let mut params = json!({
                    "approvalPolicy": "never",
                    "sandbox": "read-only",
                    "ephemeral": true,
                });
                if let Some(scratch) = &self.scratch {
                    params["cwd"] = scratch.to_string_lossy().into_owned().into();
                }
                self.request("thread/start", params);
            }
            "thread/start" => {
                let Some(id) = result
                    .at(&["thread", "id"])
                    .and_then(JsonValue::string_value)
                    .map(String::from)
                else {
                    self.finish(AgentEvent::Failed(
                        "Codex didn't start a conversation.".into(),
                    ));
                    return;
                };
                self.thread_id = Some(id.clone());
                self.phase = Phase::StartingTurn;
                let prompt = self.prompt.clone();
                self.request(
                    "turn/start",
                    json!({
                        "threadId": id,
                        "input": [{"type": "text", "text": prompt}],
                        "approvalPolicy": "never",
                        "sandboxPolicy": {"type": "readOnly"},
                    }),
                );
            }
            "turn/start" => {
                self.turn_id = result
                    .at(&["turn", "id"])
                    .and_then(JsonValue::string_value)
                    .map(String::from);
                self.phase = Phase::Streaming;
            }
            _ => {}
        }
    }

    fn handle_notification(&mut self, method: &str, params: &JsonValue) {
        match method {
            "turn/started" => {
                if self.turn_id.is_none() {
                    self.turn_id = params
                        .at(&["turn", "id"])
                        .and_then(JsonValue::string_value)
                        .map(String::from);
                }
            }
            "item/agentMessage/delta" => {
                if let Some(d) = params.get("delta").and_then(JsonValue::string_value) {
                    self.text.push_str(d);
                    let text = self.text.clone();
                    if let Some(h) = self.on_event.as_mut() {
                        h(AgentEvent::Progress(text));
                    }
                }
            }
            "item/completed" => {
                if params
                    .at(&["item", "type"])
                    .and_then(JsonValue::string_value)
                    == Some("agentMessage")
                    && let Some(t) = params
                        .at(&["item", "text"])
                        .and_then(JsonValue::string_value)
                {
                    self.final_text = Some(t.to_string());
                }
            }
            "turn/completed" => {
                let status = params
                    .at(&["turn", "status"])
                    .and_then(JsonValue::string_value);
                if status == Some("failed") {
                    let message = params
                        .at(&["turn", "error", "message"])
                        .and_then(JsonValue::string_value)
                        .unwrap_or("The request failed.");
                    self.finish(AgentEvent::Failed(Self::friendly(message)));
                } else if status == Some("interrupted") {
                    self.finish(AgentEvent::Failed("Stopped.".into()));
                } else {
                    let text = self.final_text.clone().unwrap_or_else(|| self.text.clone());
                    self.finish(AgentEvent::Completed(text));
                }
            }
            "error" => {
                if params.get("willRetry") == Some(&JsonValue::Bool(true)) {
                    return;
                }
                let message = params
                    .at(&["error", "message"])
                    .and_then(JsonValue::string_value)
                    .unwrap_or("Codex reported an error.");
                self.finish(AgentEvent::Failed(Self::friendly(message)));
            }
            _ => {}
        }
    }

    /// Writing help never needs tools; every approval or input request is declined.
    fn handle_server_request(&mut self, id: JsonValue, method: &str) {
        match method {
            "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "execCommandApproval"
            | "applyPatchApproval" => self.respond(id, json!({"decision": "decline"})),
            "item/permissions/requestApproval" => self.respond(id, json!({"decision": "decline"})),
            "mcpServer/elicitation/request" => self.respond(id, json!({"action": "decline"})),
            _ => self.respond_error(id, format!("Fluent Writer doesn't support {method}.")),
        }
    }

    /// Sign-in failures get a message that says what to do.
    pub fn friendly(message: &str) -> String {
        let lower = message.to_lowercase();
        if lower.contains("login")
            || lower.contains("auth")
            || lower.contains("401")
            || lower.contains("api key")
        {
            return "Codex isn't signed in. Run `codex login` in Terminal, then try again.".into();
        }
        message.to_string()
    }

    fn finish(&mut self, event: AgentEvent) {
        if self.phase == Phase::Finished {
            return;
        }
        self.phase = Phase::Finished;
        if let Some(mut h) = self.on_event.take() {
            h(event);
        }
        self.tear_down();
    }

    fn tear_down(&mut self) {
        if let Some(p) = self.process.take() {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(300));
                p.terminate();
            });
        }
        if let Some(scratch) = self.scratch.take() {
            let _ = std::fs::remove_dir_all(scratch);
        }
    }
}

impl AgentSession for CodexAgent {
    fn start(&mut self, prompt: &str, on_event: EventHandler, sink: ProcessSink) {
        self.prompt = prompt.to_string();
        self.on_event = Some(on_event);
        if self.send_override.is_none() {
            let Some(executable) = self.executable.clone() else {
                self.finish(AgentEvent::Failed(
                    "Codex isn't installed. Install it with `npm install -g @openai/codex`, then sign in with `codex login`.".into(),
                ));
                return;
            };
            let dir = ExecutableLocator::scratch_directory();
            self.scratch = Some(dir.clone());
            let tool_dir = executable.parent().map(PathBuf::from).unwrap_or_default();
            let p = LineProcess::new(
                &executable,
                ["app-server"],
                Some(dir),
                Some(ExecutableLocator::environment(&[tool_dir])),
            );
            if let Err(e) = p.run(sink) {
                self.finish(AgentEvent::Failed(format!("Couldn't start Codex: {e}")));
                return;
            }
            self.process = Some(p);
        }
        self.phase = Phase::Initializing;
        self.request(
            "initialize",
            json!({
                "clientInfo": {"name": "fluent_writer", "title": "Fluent Writer", "version": "0.1.0"},
                "capabilities": {"experimentalApi": false},
            }),
        );
    }

    fn handle(&mut self, event: ProcessEvent) {
        match event {
            ProcessEvent::Line(line) => self.receive(&line),
            ProcessEvent::Exit { status, stderr } => {
                if self.phase == Phase::Finished {
                    return;
                }
                let detail = last_line(&stderr).unwrap_or_else(|| format!("exit {status}"));
                self.finish(AgentEvent::Failed(format!(
                    "Codex stopped unexpectedly: {detail}"
                )));
            }
        }
    }

    fn cancel(&mut self) {
        if self.phase == Phase::Finished {
            return;
        }
        if let (Some(thread_id), Some(turn_id)) = (self.thread_id.clone(), self.turn_id.clone()) {
            self.request(
                "turn/interrupt",
                json!({"threadId": thread_id, "turnId": turn_id}),
            );
        }
        self.phase = Phase::Finished;
        self.on_event = None;
        self.tear_down();
    }

    fn is_finished(&self) -> bool {
        self.phase == Phase::Finished
    }
}

/// The last non-empty line of a process's stderr, for "stopped unexpectedly" messages. A trailing
/// `\r` is dropped so Windows output reads the same.
pub(crate) fn last_line(stderr: &str) -> Option<String> {
    stderr
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .rfind(|l| !l.is_empty())
        .map(String::from)
}
