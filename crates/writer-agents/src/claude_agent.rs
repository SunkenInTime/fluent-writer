//! Runs the bundled Node bridge around the Claude Agent SDK with every tool denied, plus the
//! factory that picks an agent and the fixture agent for offline UI testing
//! (Swift `ClaudeAgent.swift`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::agent_models::{AgentEvent, AgentProvider, AgentSession, EventHandler, ProcessSink};
use crate::codex_agent::{CodexAgent, last_line};
use crate::json_value::JsonValue;
use crate::line_process::{ExecutableLocator, LineProcess, ProcessEvent};

/// Overrides the bridge script: a path to `bridge.mjs`.
pub const BRIDGE_ENV: &str = "FLUENT_WRITER_BRIDGE";
/// Overrides the Node.js executable.
pub const NODE_ENV: &str = "FLUENT_WRITER_NODE";
/// Tells the bridge which Claude Code binary to use.
pub const CLAUDE_PATH_ENV: &str = "FLUENT_WRITER_CLAUDE_PATH";
/// Replays this file's text instead of calling a provider.
pub const FIXTURE_ENV: &str = "FLUENT_WRITER_AGENT_FIXTURE";

const BRIDGE_RELATIVE: &str = "bridge/claude/bridge.mjs";

pub struct ClaudeAgent {
    node: Option<PathBuf>,
    bridge_script: Option<PathBuf>,
    process: Option<LineProcess>,
    on_event: Option<EventHandler>,
    text: String,
    finished: bool,
    scratch: Option<PathBuf>,
}

impl Default for ClaudeAgent {
    /// Finds `node` and the bridge with [`ClaudeAgent::locate_node`] and [`ClaudeAgent::locate_bridge`].
    fn default() -> Self {
        ClaudeAgent::new(Self::locate_node(), Self::locate_bridge())
    }
}

impl ClaudeAgent {
    pub fn new(node: Option<PathBuf>, bridge_script: Option<PathBuf>) -> Self {
        ClaudeAgent {
            node,
            bridge_script,
            process: None,
            on_event: None,
            text: String::new(),
            finished: false,
            scratch: None,
        }
    }

    /// `$FLUENT_WRITER_NODE` if it names a file, then `node` next to this executable (for installs
    /// that bundle Node), then the Swift lookup ([`ExecutableLocator::find`]).
    pub fn locate_node() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os(NODE_ENV).map(PathBuf::from)
            && p.is_file()
        {
            return Some(p);
        }
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf));
        if let Some(dir) = exe_dir {
            let bundled = dir.join(if cfg!(windows) { "node.exe" } else { "node" });
            if bundled.is_file() {
                return Some(bundled);
            }
        }
        ExecutableLocator::find("node", &[])
    }

    /// The first `bridge.mjs` that exists among:
    /// 1. `$FLUENT_WRITER_BRIDGE`;
    /// 2. macOS: `<App>.app/Contents/Resources/bridge/claude/bridge.mjs` (Swift's bundle resources);
    /// 3. `<executable dir>/bridge/claude/bridge.mjs` (Windows and Linux installs; also where Swift's
    ///    `Bundle.main.resourceURL` pointed for an unbundled binary);
    /// 4. Linux: `<executable dir>/../share/fluent-writer/bridge/claude/bridge.mjs`;
    /// 5. the repository copy this crate was built from (Swift used `#filePath` the same way).
    pub fn locate_bridge() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = std::env::var_os(BRIDGE_ENV) {
            candidates.push(PathBuf::from(p));
        }
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
        {
            candidates.extend(Self::bridge_candidates(&dir));
        }
        candidates.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(BRIDGE_RELATIVE),
        );
        candidates.into_iter().find(|p| p.is_file())
    }

    /// Where an install keeps the bridge, relative to the directory holding the executable.
    pub fn bridge_candidates(exe_dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if cfg!(target_os = "macos") {
            out.push(exe_dir.join("../Resources").join(BRIDGE_RELATIVE));
        }
        out.push(exe_dir.join(BRIDGE_RELATIVE));
        if cfg!(target_os = "linux") {
            out.push(exe_dir.join("../share/fluent-writer").join(BRIDGE_RELATIVE));
        }
        out
    }

    /// Handles one line from the bridge.
    pub fn receive(&mut self, line: &str) {
        if self.finished {
            return;
        }
        let Some(msg) = JsonValue::parse(line) else {
            return;
        };
        match msg.get("type").and_then(JsonValue::string_value) {
            Some("delta") => {
                if let Some(t) = msg.get("text").and_then(JsonValue::string_value) {
                    self.text.push_str(t);
                    let text = self.text.clone();
                    if let Some(h) = self.on_event.as_mut() {
                        h(AgentEvent::Progress(text));
                    }
                }
            }
            Some("done") => {
                let text = msg
                    .get("text")
                    .and_then(JsonValue::string_value)
                    .map_or_else(|| self.text.clone(), String::from);
                self.finish(AgentEvent::Completed(text));
            }
            Some("error") => {
                let message = msg
                    .get("message")
                    .and_then(JsonValue::string_value)
                    .unwrap_or("Claude reported an error.");
                self.finish(AgentEvent::Failed(message.to_string()));
            }
            _ => {}
        }
    }

    fn finish(&mut self, event: AgentEvent) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(mut h) = self.on_event.take() {
            h(event);
        }
        self.tear_down();
    }

    fn tear_down(&mut self) {
        if let Some(p) = self.process.take() {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(500));
                p.terminate();
            });
        }
        if let Some(scratch) = self.scratch.take() {
            let _ = std::fs::remove_dir_all(scratch);
        }
    }
}

impl AgentSession for ClaudeAgent {
    fn start(&mut self, prompt: &str, on_event: EventHandler, sink: ProcessSink) {
        self.on_event = Some(on_event);
        let Some(node) = self.node.clone() else {
            self.finish(AgentEvent::Failed(
                "Claude needs Node.js. Install it from nodejs.org or with `brew install node`."
                    .into(),
            ));
            return;
        };
        let Some(bridge) = self.bridge_script.clone() else {
            self.finish(AgentEvent::Failed(
                "The Claude bridge is missing from this build.".into(),
            ));
            return;
        };
        let dir = ExecutableLocator::scratch_directory();
        self.scratch = Some(dir.clone());
        let mut dirs = vec![node.parent().map(PathBuf::from).unwrap_or_default()];
        let claude = ExecutableLocator::find("claude", &[]);
        if let Some(c) = &claude {
            dirs.push(c.parent().map(PathBuf::from).unwrap_or_default());
        }
        let mut env = ExecutableLocator::environment(&dirs);
        if let Some(c) = claude.filter(|c| usable_by_sdk(c)) {
            env.insert(CLAUDE_PATH_ENV.into(), c.into_os_string());
        }
        let p = LineProcess::new(&node, [bridge.as_os_str()], Some(dir.clone()), Some(env));
        if let Err(e) = p.run(sink) {
            self.finish(AgentEvent::Failed(format!("Couldn't start Claude: {e}")));
            return;
        }
        let req: JsonValue = json!({
            "type": "request",
            "prompt": prompt,
            "cwd": dir.to_string_lossy(),
        })
        .into();
        p.send(&req.serialized());
        self.process = Some(p);
    }

    fn handle(&mut self, event: ProcessEvent) {
        match event {
            ProcessEvent::Line(line) => self.receive(&line),
            ProcessEvent::Exit { status, stderr } => {
                if self.finished {
                    return;
                }
                let detail = last_line(&stderr).unwrap_or_else(|| format!("exit {status}"));
                self.finish(AgentEvent::Failed(format!(
                    "Claude stopped unexpectedly: {detail}"
                )));
            }
        }
    }

    fn cancel(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.on_event = None;
        if let Some(p) = &self.process {
            p.send(&JsonValue::object([("type", "cancel".into())]).serialized());
        }
        self.tear_down();
    }

    fn is_finished(&self) -> bool {
        self.finished
    }
}

/// The SDK spawns `pathToClaudeCodeExecutable` directly, which Node can't do for a Windows
/// `.cmd`/`.bat` npm shim; the bridge then falls back to the SDK's bundled Claude Code.
fn usable_by_sdk(path: &Path) -> bool {
    if !cfg!(windows) {
        return true;
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    ext != "cmd" && ext != "bat"
}

/// Picks the agent for a provider.
pub struct AgentFactory;

impl AgentFactory {
    /// A [`FixtureAgent`] when `FLUENT_WRITER_AGENT_FIXTURE` names a readable UTF-8 file,
    /// otherwise the provider's agent.
    pub fn make(provider: AgentProvider) -> Box<dyn AgentSession> {
        if let Some(path) = std::env::var_os(FIXTURE_ENV)
            && let Ok(reply) = std::fs::read_to_string(path)
        {
            return Box::new(FixtureAgent::new(reply));
        }
        match provider {
            AgentProvider::Codex => Box::new(CodexAgent::default()),
            AgentProvider::Claude => Box::new(ClaudeAgent::default()),
        }
    }
}

/// Replays a canned reply word by word, 40 ms per word; used for offline UI testing.
pub struct FixtureAgent {
    reply: String,
    cancelled: Arc<AtomicBool>,
}

impl FixtureAgent {
    pub fn new(reply: impl Into<String>) -> Self {
        FixtureAgent {
            reply: reply.into(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AgentSession for FixtureAgent {
    fn start(&mut self, _prompt: &str, mut on_event: EventHandler, _sink: ProcessSink) {
        let chunks: Vec<String> = self.reply.split(' ').map(String::from).collect();
        let cancelled = self.cancelled.clone();
        let begin = Instant::now();
        thread::spawn(move || {
            let mut text = String::new();
            let last = chunks.len() - 1;
            for (i, chunk) in chunks.into_iter().enumerate() {
                let due = begin + Duration::from_millis(40 * (i as u64 + 1));
                thread::sleep(due.saturating_duration_since(Instant::now()));
                if cancelled.load(Ordering::SeqCst) {
                    return;
                }
                if i > 0 {
                    text.push(' ');
                }
                text.push_str(&chunk);
                if i == last {
                    cancelled.store(true, Ordering::SeqCst);
                    on_event(AgentEvent::Completed(text));
                    return;
                }
                on_event(AgentEvent::Progress(text.clone()));
            }
        });
    }

    fn handle(&mut self, _event: ProcessEvent) {}

    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    fn is_finished(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}
