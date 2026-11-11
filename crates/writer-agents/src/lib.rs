//! Fluent Writer's writing agents, ported from the Swift `WriterAgents` module.
//!
//! Assist sends the draft to Codex (`codex app-server` over JSON-RPC) or Claude (the Node bridge in
//! `bridge/claude` around the Claude Agent SDK). Both run in an empty scratch folder with read-only
//! sandboxes and every tool request declined.
//!
//! The UI builds an [`AgentRequest`] and calls [`AgentHandle::start_request`]. The request runs on
//! a worker thread; events arrive over a channel, and the wake callback runs after each one so the
//! UI thread can call [`AgentHandle::try_recv`]. [`AgentPrompt::parse`] turns the raw text of a
//! [`AgentEvent::Progress`] or [`AgentEvent::Completed`] into feedback, edits and comment replies.
//! No async runtime is involved.

pub mod agent_handle;
pub mod agent_models;
pub mod claude_agent;
pub mod codex_agent;
pub mod json_value;
pub mod line_process;

mod swift_text;

pub use agent_handle::AgentHandle;
pub use agent_models::{
    AgentAction, AgentEvent, AgentPrompt, AgentProvider, AgentReply, AgentRequest, AgentSession,
    EventHandler, HistoryEntry, ProcessSink, PromptComment,
};
pub use claude_agent::{AgentFactory, ClaudeAgent, FixtureAgent};
pub use codex_agent::{CodexAgent, Phase, SendOverride};
pub use json_value::JsonValue;
pub use line_process::{Environment, ExecutableLocator, LineProcess, ProcessEvent};
