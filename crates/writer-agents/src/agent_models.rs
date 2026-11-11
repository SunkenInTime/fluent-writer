//! Requests, events, the prompt the agents get and the parser for their replies
//! (Swift `AgentModels.swift`).

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, LazyLock};

use regex::Regex;
use serde::{Deserialize, Serialize};
use writer_core::{EditPosition, ProposedEdit};

use crate::line_process::ProcessEvent;
use crate::swift_text::{prefix, trim, trim_whitespace};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentProvider {
    #[default]
    Codex,
    Claude,
}

impl AgentProvider {
    pub const ALL: [AgentProvider; 2] = [AgentProvider::Codex, AgentProvider::Claude];

    /// `codex` or `claude`, as stored under `assist.provider`.
    pub fn raw_value(self) -> &'static str {
        match self {
            AgentProvider::Codex => "codex",
            AgentProvider::Claude => "claude",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            AgentProvider::Codex => "Codex",
            AgentProvider::Claude => "Claude",
        }
    }
}

impl FromStr for AgentProvider {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        Self::ALL.into_iter().find(|p| p.raw_value() == s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentAction {
    Critique,
    Clarify,
    Shorten,
    StrongerOpening,
    Custom,
    ResolveComments,
}

impl AgentAction {
    pub const ALL: [AgentAction; 6] = [
        AgentAction::Critique,
        AgentAction::Clarify,
        AgentAction::Shorten,
        AgentAction::StrongerOpening,
        AgentAction::Custom,
        AgentAction::ResolveComments,
    ];

    /// Actions offered as one-click buttons.
    pub const QUICK_ACTIONS: [AgentAction; 4] = [
        AgentAction::Critique,
        AgentAction::Clarify,
        AgentAction::Shorten,
        AgentAction::StrongerOpening,
    ];

    /// The Swift raw value, as stored in the Assist transcript's `action`.
    pub fn raw_value(self) -> &'static str {
        match self {
            AgentAction::Critique => "critique",
            AgentAction::Clarify => "clarify",
            AgentAction::Shorten => "shorten",
            AgentAction::StrongerOpening => "strongerOpening",
            AgentAction::Custom => "custom",
            AgentAction::ResolveComments => "resolveComments",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AgentAction::Critique => "Critique",
            AgentAction::Clarify => "Clarify",
            AgentAction::Shorten => "Shorten",
            AgentAction::StrongerOpening => "Stronger opening",
            AgentAction::Custom => "Ask",
            AgentAction::ResolveComments => "Resolve comments",
        }
    }

    /// Whether this action may change the text.
    pub fn proposes_edit(self) -> bool {
        self != AgentAction::Critique
    }

    fn task(self) -> &'static str {
        match self {
            AgentAction::Critique => {
                "Critique the text. Point out what works, what is unclear, and what to cut. Do not rewrite it."
            }
            AgentAction::Clarify => {
                "Edit the text so every sentence is clear and direct, keeping the author's voice, meaning and length."
            }
            AgentAction::Shorten => {
                "Shorten the text by roughly a third without losing its meaning or voice."
            }
            AgentAction::StrongerOpening => {
                "Rewrite only the opening so it pulls the reader in. Leave the rest unchanged."
            }
            AgentAction::Custom => {
                "Follow the author's instruction. It may ask you to change any part of the draft, add new text anywhere (including at the end), or just answer a question."
            }
            AgentAction::ResolveComments => {
                "The author left comments on the draft. Address each one: change the text it points at when the comment asks for a change, or answer it when it asks a question."
            }
        }
    }
}

impl FromStr for AgentAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        Self::ALL.into_iter().find(|a| a.raw_value() == s).ok_or(())
    }
}

/// A comment as the agent sees it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptComment {
    pub id: String,
    pub quote: String,
    pub body: String,
    /// Earlier replies, each already formatted (the Swift UI sent `"<author>: <text>"`).
    pub replies: Vec<String>,
}

impl PromptComment {
    pub fn new(id: impl Into<String>, quote: impl Into<String>, body: impl Into<String>) -> Self {
        PromptComment {
            id: id.into(),
            quote: quote.into(),
            body: body.into(),
            replies: Vec::new(),
        }
    }
}

/// One earlier instruction and the reply to it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoryEntry {
    pub instruction: String,
    pub feedback: String,
}

impl HistoryEntry {
    pub fn new(instruction: impl Into<String>, feedback: impl Into<String>) -> Self {
        HistoryEntry {
            instruction: instruction.into(),
            feedback: feedback.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRequest {
    pub provider: AgentProvider,
    pub action: AgentAction,
    pub instruction: String,
    /// The whole draft. Edits may land anywhere in it.
    pub document: String,
    /// The passage the author selected, if any. It focuses the request; it doesn't limit it.
    pub selection: Option<String>,
    pub title: String,
    pub comments: Vec<PromptComment>,
    /// Earlier instructions and replies for this draft, oldest first.
    pub history: Vec<HistoryEntry>,
}

impl AgentRequest {
    /// A request with no selection, comments or history.
    pub fn new(
        provider: AgentProvider,
        action: AgentAction,
        instruction: impl Into<String>,
        document: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        AgentRequest {
            provider,
            action,
            instruction: instruction.into(),
            document: document.into(),
            selection: None,
            title: title.into(),
            comments: Vec::new(),
            history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// Accumulated raw model output so far.
    Progress(String),
    /// The whole raw reply. Terminal.
    Completed(String),
    /// A message for the author. Terminal.
    Failed(String),
}

impl AgentEvent {
    pub fn is_terminal(&self) -> bool {
        !matches!(self, AgentEvent::Progress(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentReply {
    pub feedback: String,
    pub edits: Vec<ProposedEdit>,
    /// Answers to comments, keyed by comment id.
    pub replies: HashMap<String, String>,
}

/// Builds the prompt and parses replies.
pub struct AgentPrompt;

static EDIT_BLOCK: LazyLock<Regex> = LazyLock::new(|| block_regex("edit"));
static REPLY_BLOCK: LazyLock<Regex> = LazyLock::new(|| block_regex("reply"));
static AT_ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| attribute_regex("at"));
static FOR_ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| attribute_regex("for"));

fn block_regex(tag: &str) -> Regex {
    Regex::new(&format!(r"<{tag}(\s[^>]*)?>([\s\S]*?)</{tag}>")).expect("valid block regex")
}

fn attribute_regex(name: &str) -> Regex {
    Regex::new(&format!(r#"{name}\s*=\s*["']([^"']*)["']"#)).expect("valid attribute regex")
}

impl AgentPrompt {
    pub const FEEDBACK_TAG: &'static str = "feedback";
    pub const EDIT_TAG: &'static str = "edit";

    pub fn build(request: &AgentRequest) -> String {
        let titled = if request.title.is_empty() {
            String::new()
        } else {
            format!(" titled \"{}\"", request.title)
        };
        let mut lines: Vec<String> = vec![
            format!("You are a careful writing editor working with an author on their Markdown draft{titled}."),
            "Work only from the text in this message. Do not run commands, read or write files, browse, or use any tools.".into(),
            String::new(),
            format!("Task: {}", request.action.task()),
        ];
        let instruction = trim(&request.instruction);
        if !instruction.is_empty() {
            lines.push(format!("Author's instruction: {instruction}"));
        }
        if let Some(selection) = request.selection.as_deref().filter(|s| !s.is_empty()) {
            lines.extend([
                String::new(),
                "The author selected this passage, so focus on it. You may still change other parts of the draft when the instruction calls for it.".into(),
                "<selection>".into(),
                selection.into(),
                "</selection>".into(),
            ]);
        }
        if !request.comments.is_empty() {
            lines.push(String::new());
            lines.push(if request.action == AgentAction::ResolveComments {
                "The author's comments. Each one is attached to the quoted text:".into()
            } else {
                "Comments the author left on the draft, for context. Act on them only if the instruction asks you to:".into()
            });
            for c in &request.comments {
                lines.push(format!("<comment id=\"{}\">", c.id));
                lines.extend([
                    "<on>".into(),
                    c.quote.clone(),
                    "</on>".into(),
                    "<note>".into(),
                    c.body.clone(),
                    "</note>".into(),
                ]);
                for r in &c.replies {
                    lines.extend([
                        "<earlier-reply>".into(),
                        r.clone(),
                        "</earlier-reply>".into(),
                    ]);
                }
                lines.push("</comment>".into());
            }
        }
        let history = &request.history[request.history.len().saturating_sub(4)..];
        if !history.is_empty() {
            lines.extend([
                String::new(),
                "Earlier in this session (for context; the draft below already reflects whatever the author kept):".into(),
            ]);
            for h in history {
                let ask = if h.instruction.is_empty() {
                    "(a quick action)"
                } else {
                    &h.instruction
                };
                lines.push(format!("- Author asked: {}", prefix(ask, 400)));
                if !h.feedback.is_empty() {
                    lines.push(format!("  You replied: {}", prefix(&h.feedback, 600)));
                }
            }
        }
        let feedback = Self::FEEDBACK_TAG;
        lines.extend([
            String::new(),
            "Reply in exactly this format:".into(),
            format!("<{feedback}>"),
            "Brief notes for the author in plain prose. Two to six short sentences or bullet points.".into(),
            format!("</{feedback}>"),
        ]);
        if request.action.proposes_edit() {
            lines.extend(
                [
                    "Then zero or more edits. Each edit changes one passage, anywhere in the draft:",
                    "<edit>",
                    "<old>exact text copied from the draft</old>",
                    "<new>the replacement</new>",
                    "</edit>",
                    "To add text at the end of the draft (or at the start), use:",
                    "<edit at=\"end\">",
                    "<new>the text to add</new>",
                    "</edit>",
                    "Rules for edits:",
                    "- Copy <old> character for character from the draft, and make it long enough to appear only once. Keep it to the sentences you change, not the whole draft.",
                    "- To insert text after a passage, put that passage in <old> and repeat it at the start of <new>.",
                    "- To delete text, leave <new> empty.",
                    "- Never repeat the draft or add a second copy of it. Edits replace text in place; the author previews the result as a separate version.",
                    "- Leave out edits entirely if no change is warranted.",
                ]
                .map(String::from),
            );
            if !request.comments.is_empty() {
                lines.extend(
                    [
                        "- When an edit addresses a comment, name it: <edit for=\"COMMENT_ID\">.",
                        "- To answer a comment without changing the text, write <reply for=\"COMMENT_ID\">your answer</reply>.",
                    ]
                    .map(String::from),
                );
            }
        } else {
            lines.push("Do not include any edits.".into());
            if !request.comments.is_empty() {
                lines.push(
                    "You may answer comments with <reply for=\"COMMENT_ID\">your answer</reply>."
                        .into(),
                );
            }
        }
        lines.extend([
            String::new(),
            "Draft:".into(),
            "<draft>".into(),
            request.document.clone(),
            "</draft>".into(),
        ]);
        lines.join("\n")
    }

    /// Splits model output into feedback, edits and comment replies. Tolerates partial streams:
    /// feedback shows as it arrives, edits and replies only once their closing tag is in.
    pub fn parse(output: &str) -> AgentReply {
        let feedback = partial_section(Self::FEEDBACK_TAG, output);
        let edits: Vec<ProposedEdit> = blocks(&EDIT_BLOCK, output)
            .into_iter()
            .filter_map(|(attrs, body)| edit_from(attrs, body))
            .collect();
        let mut replies = HashMap::new();
        for (attrs, body) in blocks(&REPLY_BLOCK, output) {
            let Some(id) = attribute(&FOR_ATTRIBUTE, attrs)
                .and_then(|a| a.split(' ').find(|p| !p.is_empty()).map(String::from))
            else {
                continue;
            };
            let text = trim(body);
            if !text.is_empty() {
                replies.insert(id, text.to_string());
            }
        }
        if feedback.is_none() && edits.is_empty() && replies.is_empty() && !output.contains('<') {
            return AgentReply {
                feedback: trim(output).to_string(),
                ..AgentReply::default()
            };
        }
        AgentReply {
            feedback: trim(feedback.unwrap_or("")).to_string(),
            edits,
            replies,
        }
    }

    pub fn is_drafting_edits(output: &str) -> bool {
        output.contains("<edit")
    }
}

fn edit_from(attrs: &str, body: &str) -> Option<ProposedEdit> {
    let at = attribute(&AT_ATTRIBUTE, attrs).and_then(|a| match a.to_lowercase().as_str() {
        "start" => Some(EditPosition::Start),
        "end" => Some(EditPosition::End),
        _ => None,
    });
    let ids: Vec<String> = attribute(&FOR_ATTRIBUTE, attrs)
        .map(|a| {
            a.split([' ', ','])
                .filter(|p| !p.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let old = inner_section("old", body).map(trim_newlines);
    let new = inner_section("new", body).map(trim_newlines)?;
    if old.is_none() && at.is_none() {
        return None;
    }
    let position = if old.is_none_or(str::is_empty) {
        Some(at.unwrap_or(EditPosition::End))
    } else {
        None
    };
    Some(ProposedEdit {
        old: old.map(String::from),
        new: new.to_string(),
        position,
        comment_ids: ids,
    })
}

/// Complete `<tag …>…</tag>` blocks, in order, as `(attributes, body)`.
fn blocks<'a>(re: &Regex, text: &'a str) -> Vec<(&'a str, &'a str)> {
    re.captures_iter(text)
        .map(|m| {
            (
                m.get(1).map_or("", |g| g.as_str()),
                m.get(2).map_or("", |g| g.as_str()),
            )
        })
        .collect()
}

fn attribute<'a>(re: &Regex, attrs: &'a str) -> Option<&'a str> {
    re.captures(attrs)
        .and_then(|m| m.get(1))
        .map(|g| trim_whitespace(g.as_str()))
}

fn inner_section<'a>(tag: &str, text: &'a str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&format!("</{tag}>"))?;
    Some(&text[start..start + end])
}

/// A section that may still be streaming: everything after the open tag up to its close, or up to
/// the next tag when the close hasn't arrived.
fn partial_section<'a>(tag: &str, text: &'a str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let rest = &text[text.find(&open)? + open.len()..];
    if let Some(close) = rest.find(&format!("</{tag}>")) {
        return Some(&rest[..close]);
    }
    if let Some(next) = rest.find('<') {
        return Some(&rest[..next]);
    }
    Some(rest)
}

/// Strips leading and trailing `\n` and `\r` characters. A CRLF pair is one Swift character that
/// is neither, so it stays, as it did in Swift.
fn trim_newlines(s: &str) -> &str {
    let mut t = s;
    while (t.starts_with('\n') || t.starts_with('\r')) && !t.starts_with("\r\n") {
        t = &t[1..];
    }
    while (t.ends_with('\n') || t.ends_with('\r')) && !t.ends_with("\r\n") {
        t = &t[..t.len() - 1];
    }
    t
}

/// Receives events for one request. Called on whichever thread drives the session.
pub type EventHandler = Box<dyn FnMut(AgentEvent) + Send>;

/// Where a session's child process delivers its output, so the thread driving the session can
/// feed it back through [`AgentSession::handle`].
pub type ProcessSink = Arc<dyn Fn(ProcessEvent) + Send + Sync>;

/// One in-flight request to a writing agent (Swift `AgentSession`). The methods are synchronous;
/// [`crate::AgentHandle`] runs a session on a worker thread.
pub trait AgentSession: Send {
    /// Starts the request. `on_event` is called until a terminal event or [`AgentSession::cancel`].
    /// Output from a child process arrives at `sink`, to be passed to [`AgentSession::handle`].
    fn start(&mut self, prompt: &str, on_event: EventHandler, sink: ProcessSink);
    /// Feeds a line or the exit of the session's child process.
    fn handle(&mut self, event: ProcessEvent);
    /// Stops the request. No events are delivered after this returns.
    fn cancel(&mut self);
    /// True after a terminal event or `cancel`.
    fn is_finished(&self) -> bool;
}

impl fmt::Debug for dyn AgentSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentSession")
            .field("finished", &self.is_finished())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_pair_survives_trim_newlines() {
        assert_eq!(trim_newlines("\n\nabc\r"), "abc");
        assert_eq!(trim_newlines("\r\nabc\r\n"), "\r\nabc\r\n");
    }

    #[test]
    fn history_keeps_last_four_and_truncates() {
        let mut req = AgentRequest::new(AgentProvider::Codex, AgentAction::Custom, "", "x", "");
        req.history = (0..6)
            .map(|i| HistoryEntry::new(format!("ask{i}"), ""))
            .collect();
        req.history[5].instruction = "y".repeat(500);
        let p = AgentPrompt::build(&req);
        assert!(!p.contains("ask1"));
        assert!(p.contains("ask2"));
        assert!(p.contains(&format!("- Author asked: {}\n", "y".repeat(400))));
    }

    #[test]
    fn reply_and_edit_attributes() {
        let r = AgentPrompt::parse(
            "<feedback>x</feedback><edit for='a, b' at=\"START\"><new>n</new></edit><reply for=\" c1 c2\">ok</reply>",
        );
        assert_eq!(
            r.edits,
            vec![
                ProposedEdit::insert("n", EditPosition::Start)
                    .with_comment_ids(vec!["a".into(), "b".into()])
            ]
        );
        assert_eq!(r.replies.get("c1").map(String::as_str), Some("ok"));
    }
}
