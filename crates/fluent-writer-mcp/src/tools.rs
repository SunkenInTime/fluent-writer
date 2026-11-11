//! The MCP tools: their schemas and what each call does.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::json;
use writer_agents::JsonValue;
use writer_core::draft_store::FILE_EXTENSIONS;
use writer_core::text_stats::counts_for_markdown;
use writer_core::{
    AssistExchange, Comment, DraftLibrary, DraftVersion, EditPosition, InboxCommand,
    InboxCommandKind, InboxResult, NotesStore, ProposedEdit, VersionKind, utf16_to_byte,
};

pub struct Server {
    pub store: NotesStore,
    pub support_directory: PathBuf,
    pub agent_name: String,
}

/// A tool failure; the message goes back to the agent with `isError: true`.
type ToolResult = Result<String, String>;

// MARK: Helpers

/// 1-based line of a byte offset (the Swift counted `\n` code units before a UTF-16 offset).
fn line_number(offset: usize, text: &str) -> usize {
    let end = offset.min(text.len());
    1 + text.as_bytes()[..end]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

/// Whole-number argument, clamped, so NaN or huge values can't trap the server.
fn int_arg(v: Option<f64>, lo: i64, hi: i64) -> Option<i64> {
    let v = v.filter(|v| v.is_finite())?;
    Some(v.round().clamp(lo as f64, hi as f64) as i64)
}

fn iso(d: SystemTime) -> JsonValue {
    let dt: DateTime<Utc> = d.into();
    dt.to_rfc3339_opts(SecondsFormat::Secs, true).into()
}

fn num(n: usize) -> JsonValue {
    JsonValue::Number(n as f64)
}

/// `split(separator: "\n", omittingEmptySubsequences: false)` on a Swift string: CRLF is one
/// character there, so it doesn't split.
fn swift_lines(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..b.len() {
        if b[i] == b'\n' && (i == 0 || b[i - 1] != b'\r') {
            out.push(&s[start..i]);
            start = i + 1;
        }
    }
    out.push(&s[start..]);
    out
}

fn expand_tilde(p: &str) -> PathBuf {
    if p == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(p));
    }
    if let Some(rest) = p.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(p)
}

/// `URL(fileURLWithPath:).standardizedFileURL`: absolute, `.` and `..` removed, symlinks kept.
fn standardize(p: &Path) -> PathBuf {
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|d| d.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    };
    let mut out = PathBuf::new();
    for c in absolute.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if out.parent().is_some() {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn read_text(path: &str) -> ToolResult {
    let data = std::fs::read(path).map_err(|_| format!("Couldn't read {path}."))?;
    Ok(String::from_utf8_lossy(&data).into_owned())
}

fn comment_json(c: &Comment, text: &str) -> JsonValue {
    let mut anchored = c.clone();
    anchored.reanchor(text);
    let mut o = JsonValue::from(json!({
        "id": c.id,
        "quote": c.quote,
        "comment": c.body,
        "author": c.author,
        "status": if c.is_open() { "open" } else { "resolved" },
    }));
    let JsonValue::Object(map) = &mut o else {
        unreachable!()
    };
    map.insert("created".into(), iso(c.created));
    if anchored.detached {
        map.insert(
            "note".into(),
            "The quoted text is no longer in the draft.".into(),
        );
    } else {
        map.insert("line".into(), num(line_number(anchored.range.start, text)));
    }
    if !c.replies.is_empty() {
        let replies = c
            .replies
            .iter()
            .map(|r| {
                JsonValue::object([
                    ("author", r.author.clone().into()),
                    ("text", r.text.clone().into()),
                    ("date", iso(r.date)),
                ])
            })
            .collect();
        map.insert("replies".into(), JsonValue::Array(replies));
    }
    o
}

fn version_summary(v: &DraftVersion) -> JsonValue {
    let mut o = JsonValue::object([
        ("id", v.id.clone().into()),
        ("label", v.label.clone().into()),
        ("source", v.source.clone().into()),
        (
            "kind",
            if v.kind == VersionKind::Snapshot {
                "saved draft"
            } else {
                "proposal"
            }
            .into(),
        ),
        ("created", iso(v.created)),
        ("words", num(counts_for_markdown(&v.body).words)),
    ]);
    let JsonValue::Object(map) = &mut o else {
        unreachable!()
    };
    if let Some(u) = v.used_at {
        map.insert("used".into(), iso(u));
    }
    if let Some(f) = &v.feedback {
        map.insert("summary".into(), f.clone().into());
    }
    if !v.resolves_comment_ids.is_empty() {
        map.insert(
            "resolves_comment_ids".into(),
            JsonValue::Array(
                v.resolves_comment_ids
                    .iter()
                    .map(|s| s.clone().into())
                    .collect(),
            ),
        );
    }
    o
}

fn exchange_json(e: &AssistExchange) -> JsonValue {
    let mut o = JsonValue::object([
        ("date", iso(e.date)),
        ("provider", e.provider.clone().into()),
        ("action", e.action.clone().into()),
        ("instruction", e.instruction.clone().into()),
        ("reply", e.feedback.clone().into()),
    ]);
    let JsonValue::Object(map) = &mut o else {
        unreachable!()
    };
    if let Some(v) = &e.version_id {
        map.insert("version_id".into(), v.clone().into());
    }
    if !e.comment_ids.is_empty() {
        map.insert(
            "comment_ids".into(),
            JsonValue::Array(e.comment_ids.iter().map(|s| s.clone().into()).collect()),
        );
    }
    o
}

fn strings(v: Option<&JsonValue>) -> Option<Vec<String>> {
    match v {
        Some(JsonValue::Array(a)) => Some(
            a.iter()
                .filter_map(|s| s.string_value().map(String::from))
                .collect(),
        ),
        _ => None,
    }
}

fn arg_str(args: &JsonValue, key: &str) -> Option<String> {
    args.get(key)
        .and_then(JsonValue::string_value)
        .map(String::from)
}

fn is_true(args: &JsonValue, key: &str) -> bool {
    args.get(key) == Some(&JsonValue::Bool(true))
}

impl Server {
    /// The text of `path` for loading its notes, or empty when it can't be read.
    fn text_or_empty(path: &str) -> String {
        read_text(path).unwrap_or_default()
    }

    fn resolve_path(&self, args: &JsonValue) -> ToolResult {
        if let Some(p) = args
            .get("path")
            .and_then(JsonValue::string_value)
            .filter(|p| !p.is_empty())
        {
            let url = standardize(&expand_tilde(p));
            let ext = url
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !FILE_EXTENSIONS.contains(&ext.as_str()) {
                return Err("`path` must be a draft (.md, .markdown, .txt or .text).".into());
            }
            return Ok(url.to_string_lossy().into_owned());
        }
        let live = self.store.live_state();
        if let Some(p) = live.as_ref().and_then(|l| l.path.clone()) {
            return Ok(p);
        }
        if live.is_some() {
            return Err("The draft open in Fluent Writer hasn't been saved to a file yet. Ask the author to type a little (it saves within a second), or pass `path`.".into());
        }
        Err("Fluent Writer isn't running, so there's no open draft. Pass `path` to a Markdown file, or use list_documents.".into())
    }

    /// Sends a change through the running app, or applies it directly when the app is closed.
    fn mutate(&self, command: InboxCommand) -> ToolResult {
        if self.store.live_state().is_some() {
            self.store.post(&command).map_err(|e| e.to_string())?;
            let Some(r) = self.store.await_result(&command.id, Duration::from_secs(4)) else {
                return Ok(
                    "Queued. Fluent Writer hasn't picked it up yet; it will apply it shortly."
                        .into(),
                );
            };
            return if r.ok { Ok(r.message) } else { Err(r.message) };
        }
        let text = read_text(&command.path)?;
        let r = self
            .store
            .with_lock(|| -> Result<InboxResult, writer_core::Error> {
                let mut notes = self.store.load(&command.path, &text);
                let r = NotesStore::apply(&command, &mut notes, &text);
                if r.ok {
                    self.store.save(&notes, &text)?;
                }
                Ok(r)
            });
        let r = r.map_err(|e| e.to_string())?;
        if !r.ok {
            return Err(r.message);
        }
        Ok(format!(
            "{} (Fluent Writer is closed; the author will see this next time they open the draft.)",
            r.message
        ))
    }

    fn command(&self, kind: InboxCommandKind, args: &JsonValue) -> Result<InboxCommand, String> {
        Ok(InboxCommand::new(
            kind,
            self.resolve_path(args)?,
            &self.agent_name,
        ))
    }

    pub fn call(&self, name: &str, args: &JsonValue) -> ToolResult {
        match name {
            "get_context" => Ok(self.get_context()),
            "read_document" => {
                let text = read_text(&self.resolve_path(args)?)?;
                if !is_true(args, "line_numbers") {
                    return Ok(text);
                }
                Ok(swift_lines(&text)
                    .iter()
                    .enumerate()
                    .map(|(i, l)| format!("{:5}  {l}", i + 1))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            "list_comments" => {
                let path = self.resolve_path(args)?;
                let text = Self::text_or_empty(&path);
                let notes = self.store.load(&path, &text);
                let all = is_true(args, "include_resolved");
                let list: Vec<&Comment> = if all {
                    notes.comments.iter().collect()
                } else {
                    notes.open_comments()
                };
                if list.is_empty() {
                    return Ok(format!(
                        "No {}comments on this draft.",
                        if all { "" } else { "open " }
                    ));
                }
                Ok(
                    JsonValue::Array(list.iter().map(|c| comment_json(c, &text)).collect())
                        .pretty_printed(),
                )
            }
            "add_comment" => {
                let mut c = self.command(InboxCommandKind::AddComment, args)?;
                c.quote = arg_str(args, "quote");
                c.body = arg_str(args, "comment");
                c.occurrence = int_arg(
                    args.get("occurrence").and_then(JsonValue::number_value),
                    0,
                    1_000_000,
                )
                .map(|n| n as usize);
                self.mutate(c)
            }
            "reply_to_comment" => {
                let mut c = self.command(InboxCommandKind::ReplyToComment, args)?;
                c.comment_id = arg_str(args, "comment_id");
                c.body = arg_str(args, "reply");
                self.mutate(c)
            }
            "resolve_comment" => {
                let mut c = self.command(InboxCommandKind::ResolveComment, args)?;
                c.comment_id = arg_str(args, "comment_id");
                c.body = arg_str(args, "note");
                self.mutate(c)
            }
            "propose_version" => {
                let mut c = self.command(InboxCommandKind::ProposeVersion, args)?;
                c.label = arg_str(args, "label");
                c.body = arg_str(args, "summary");
                c.full_text = arg_str(args, "full_text");
                if let Some(JsonValue::Array(items)) = args.get("edits") {
                    c.edits = Some(
                        items
                            .iter()
                            .map(|e| ProposedEdit {
                                old: arg_str(e, "old"),
                                new: arg_str(e, "new").unwrap_or_default(),
                                position: match e.get("at").and_then(JsonValue::string_value) {
                                    Some("start") => Some(EditPosition::Start),
                                    Some("end") => Some(EditPosition::End),
                                    _ => None,
                                },
                                comment_ids: strings(e.get("comment_ids")).unwrap_or_default(),
                            })
                            .collect(),
                    );
                }
                if c.full_text.is_none() && c.edits.as_ref().is_none_or(Vec::is_empty) {
                    return Err("Pass `edits` or `full_text`.".into());
                }
                if let Some(ids) = strings(args.get("resolves_comment_ids")) {
                    c.resolves_comment_ids = Some(ids);
                }
                self.mutate(c)
            }
            "list_versions" => {
                let path = self.resolve_path(args)?;
                let notes = self.store.load(&path, &Self::text_or_empty(&path));
                if notes.versions.is_empty() {
                    return Ok("No versions yet.".into());
                }
                Ok(
                    JsonValue::Array(notes.versions.iter().map(version_summary).collect())
                        .pretty_printed(),
                )
            }
            "read_version" => {
                let path = self.resolve_path(args)?;
                let notes = self.store.load(&path, &Self::text_or_empty(&path));
                let id = arg_str(args, "version_id");
                notes
                    .versions
                    .iter()
                    .find(|v| Some(&v.id) == id.as_ref())
                    .map(|v| v.body.clone())
                    .ok_or_else(|| "No version with that id. Use list_versions.".into())
            }
            "get_assist_history" => {
                let path = self.resolve_path(args)?;
                let notes = self.store.load(&path, &Self::text_or_empty(&path));
                let limit = int_arg(args.get("limit").and_then(JsonValue::number_value), 1, 500)
                    .unwrap_or(20);
                if notes.transcript.is_empty() {
                    return Ok("No Assist requests for this draft yet.".into());
                }
                let t = &notes.transcript;
                let tail = &t[t.len().saturating_sub(limit.max(1) as usize)..];
                Ok(JsonValue::Array(tail.iter().map(exchange_json).collect()).pretty_printed())
            }
            "list_documents" => {
                let limit = int_arg(args.get("limit").and_then(JsonValue::number_value), 1, 500)
                    .unwrap_or(20);
                let recent: Vec<_> = DraftLibrary::recent_paths(&self.support_directory)
                    .into_iter()
                    .take(limit.max(1) as usize)
                    .collect();
                if recent.is_empty() {
                    return Ok("No recent drafts.".into());
                }
                let rows = recent
                    .iter()
                    .map(|r| {
                        let notes = self.store.load(&r.path, &Self::text_or_empty(&r.path));
                        JsonValue::object([
                            ("path", r.path.clone().into()),
                            ("title", r.title.clone().into()),
                            ("last_opened", iso(r.last_opened)),
                            ("open_comments", num(notes.open_comments().len())),
                            ("versions", num(notes.versions.len())),
                        ])
                    })
                    .collect();
                Ok(JsonValue::Array(rows).pretty_printed())
            }
            _ => Err(format!("Unknown tool {name}.")),
        }
    }

    fn get_context(&self) -> String {
        let live = self.store.live_state();
        let mut o = std::collections::BTreeMap::<String, JsonValue>::new();
        o.insert(
            "app".into(),
            match &live {
                None => "Fluent Writer isn't running.".into(),
                Some(l) => format!("{} is running.", l.app_name).into(),
            },
        );
        let (live, path) = match live {
            Some(l) if l.path.is_some() => {
                let p = l.path.clone().unwrap_or_default();
                (l, p)
            }
            other => {
                if let Some(l) = other {
                    o.insert(
                        "document".into(),
                        format!(
                            "\u{201c}{}\u{201d} is open but hasn't been saved to a file yet.",
                            l.title
                        )
                        .into(),
                    );
                }
                let recent = DraftLibrary::recent_paths(&self.support_directory)
                    .into_iter()
                    .take(10)
                    .map(|r| {
                        JsonValue::object([("path", r.path.into()), ("title", r.title.into())])
                    })
                    .collect();
                o.insert("recent_documents".into(), JsonValue::Array(recent));
                return JsonValue::Object(o).pretty_printed();
            }
        };
        let text = read_text(&path).unwrap_or_default();
        let notes = self.store.load(&path, &text);
        let mut doc = JsonValue::object([
            ("path", path.clone().into()),
            ("title", live.title.clone().into()),
            ("words", num(counts_for_markdown(&text).words)),
            ("lines", num(swift_lines(&text).len())),
        ]);
        if let (Some(r), JsonValue::Object(d)) = (&live.remote, &mut doc) {
            d.insert("remote".into(), r.clone().into());
        }
        o.insert("document".into(), doc);
        let at = utf16_to_byte(&text, live.selection_location);
        if live.selection_length > 0 {
            o.insert(
                "selection".into(),
                JsonValue::object([
                    ("text", live.selected_text.clone().into()),
                    ("line", num(line_number(at, &text))),
                ]),
            );
        } else {
            o.insert("caret_line".into(), num(line_number(at, &text)));
        }
        o.insert(
            "open_comments".into(),
            JsonValue::Array(
                notes
                    .open_comments()
                    .iter()
                    .map(|c| comment_json(c, &text))
                    .collect(),
            ),
        );
        o.insert(
            "versions".into(),
            JsonValue::Array(notes.versions.iter().take(8).map(version_summary).collect()),
        );
        if let Some(p) = &live.previewing_version_id {
            o.insert("author_is_previewing_version".into(), p.clone().into());
        }
        let t = &notes.transcript;
        o.insert(
            "recent_assist".into(),
            JsonValue::Array(
                t[t.len().saturating_sub(5)..]
                    .iter()
                    .map(exchange_json)
                    .collect(),
            ),
        );
        o.insert(
            "how_to_edit".into(),
            "Use propose_version to change this draft; the author reviews it in the app. Read the full text with read_document.".into(),
        );
        JsonValue::Object(o).pretty_printed()
    }
}

// MARK: Schemas

pub fn tools() -> Vec<JsonValue> {
    let path_prop = json!({"type": "string", "description": "Absolute path to the Markdown file. Defaults to the draft open in Fluent Writer."});
    let v = json!([
        {
            "name": "get_context",
            "description": "Start here. Returns the draft open in Fluent Writer: its path and title, what the author has selected, their open comments (each pinned to quoted text), the versions waiting for review, and recent Assist requests and replies.",
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "read_document",
            "description": "Returns the full text of a draft. The open draft autosaves within a second of typing, so this matches what the author sees.",
            "inputSchema": {"type": "object", "properties": {
                "path": path_prop,
                "line_numbers": {"type": "boolean", "description": "Prefix each line with its number."},
            }},
        },
        {
            "name": "list_comments",
            "description": "The author's comments on a draft. Each has an id, the quoted text it's pinned to, its line, the comment, and any replies. Comments are notes about what to change; resolve them by proposing a version.",
            "inputSchema": {"type": "object", "properties": {
                "path": path_prop,
                "include_resolved": {"type": "boolean"},
            }},
        },
        {
            "name": "add_comment",
            "description": "Pins a comment to text in the draft, the way a reviewer would. `quote` must be copied exactly from the draft.",
            "inputSchema": {"type": "object", "required": ["quote", "comment"], "properties": {
                "path": path_prop,
                "quote": {"type": "string", "description": "Exact text from the draft to attach the comment to."},
                "comment": {"type": "string"},
                "occurrence": {"type": "integer", "description": "Which appearance of `quote` to use when it appears more than once (1-based)."},
            }},
        },
        {
            "name": "reply_to_comment",
            "description": "Answers one of the author's comments without changing the text.",
            "inputSchema": {"type": "object", "required": ["comment_id", "reply"], "properties": {
                "path": path_prop, "comment_id": {"type": "string"}, "reply": {"type": "string"},
            }},
        },
        {
            "name": "resolve_comment",
            "description": "Marks a comment resolved, with an optional note. Prefer listing comment ids in propose_version so they resolve only if the author uses that version.",
            "inputSchema": {"type": "object", "required": ["comment_id"], "properties": {
                "path": path_prop, "comment_id": {"type": "string"}, "note": {"type": "string"},
            }},
        },
        {
            "name": "propose_version",
            "description": "The way to change a draft that's open in Fluent Writer. Creates a new version from edits (or full text). The author previews it as a diff and decides whether to use it; their draft isn't touched until then. Don't write the open file directly: the app holds the live text and would overwrite your change.",
            "inputSchema": {"type": "object", "required": ["label"], "properties": {
                "path": path_prop,
                "label": {"type": "string", "description": "Short name for the version, e.g. \"Tighter intro\"."},
                "summary": {"type": "string", "description": "A few sentences on what changed and why."},
                "edits": {
                    "type": "array",
                    "description": "Changes to apply. Each replaces `old` (copied exactly from the draft, long enough to be unique) with `new`, or adds `new` at the start or end when `at` is set.",
                    "items": {"type": "object", "required": ["new"], "properties": {
                        "old": {"type": "string"},
                        "new": {"type": "string"},
                        "at": {"type": "string", "enum": ["start", "end"]},
                        "comment_ids": {"type": "array", "items": {"type": "string"}},
                    }},
                },
                "full_text": {"type": "string", "description": "The complete new draft, instead of edits."},
                "resolves_comment_ids": {"type": "array", "items": {"type": "string"}, "description": "Comments this version addresses. They resolve when the author uses it."},
            }},
        },
        {
            "name": "list_versions",
            "description": "Versions of a draft: agent proposals and the author's saved drafts, newest first.",
            "inputSchema": {"type": "object", "properties": {"path": path_prop}},
        },
        {
            "name": "read_version",
            "description": "The full text of one version.",
            "inputSchema": {"type": "object", "required": ["version_id"], "properties": {"path": path_prop, "version_id": {"type": "string"}}},
        },
        {
            "name": "get_assist_history",
            "description": "What the author asked Fluent Writer's built-in Assist about this draft, and what it replied, oldest first.",
            "inputSchema": {"type": "object", "properties": {"path": path_prop, "limit": {"type": "integer"}}},
        },
        {
            "name": "list_documents",
            "description": "Drafts the author opened recently in Fluent Writer.",
            "inputSchema": {"type": "object", "properties": {"limit": {"type": "integer"}}},
        },
    ]);
    match JsonValue::from(v) {
        JsonValue::Array(a) => a,
        _ => Vec::new(),
    }
}
