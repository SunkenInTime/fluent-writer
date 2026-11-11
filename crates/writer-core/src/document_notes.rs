//! Everything Fluent Writer keeps about a draft besides its text: comments, versions and the Assist
//! transcript, plus the store, inbox and live state outside agents use (Swift `DocumentNotes.swift`).
//!
//! Notes JSON is pretty-printed with sorted keys and ISO 8601 dates with milliseconds, as Swift's
//! `NotesStore.encoder` wrote it. Comment spans are UTF-16 offsets on disk and byte offsets in
//! memory, so loading and saving take the draft text the spans refer to.

use std::ops::Range;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::dates::{self, new_uuid_string};
use crate::edit_placement;
use crate::error::DraftStoreError;
use crate::offsets::{back_utf16, byte_to_utf16, forward_utf16, utf16_len, utf16_range_to_bytes};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentReply {
    pub author: String,
    pub text: String,
    #[serde(with = "dates::iso8601")]
    pub date: SystemTime,
}

impl CommentReply {
    pub fn new(author: impl Into<String>, text: impl Into<String>) -> Self {
        CommentReply {
            author: author.into(),
            text: text.into(),
            date: SystemTime::now(),
        }
    }
}

/// A note pinned to a span of the draft. The span follows edits while the draft is open and is found
/// again by its quoted text when the file changed elsewhere.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub id: String,
    /// Byte range into the draft (UTF-16 `location`/`length` on disk).
    pub range: Range<usize>,
    pub quote: String,
    pub prefix: String,
    pub suffix: String,
    pub body: String,
    pub author: String,
    pub created: SystemTime,
    pub replies: Vec<CommentReply>,
    pub resolved: Option<SystemTime>,
    /// The quoted text is gone from the draft.
    pub detached: bool,
}

/// How many UTF-16 units of context `capture` keeps on each side.
const CONTEXT_UNITS: usize = 32;

impl Comment {
    /// A comment on `range` of `text`, with a fresh id, created now. Swift's default author was "you".
    pub fn new(
        range: Range<usize>,
        text: &str,
        body: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        let mut c = Comment {
            id: Self::new_id(),
            range,
            quote: String::new(),
            prefix: String::new(),
            suffix: String::new(),
            body: body.into(),
            author: author.into(),
            created: SystemTime::now(),
            replies: Vec::new(),
            resolved: None,
            detached: false,
        };
        c.capture(text);
        c
    }

    /// Eight lower-case hex characters.
    pub fn new_id() -> String {
        new_uuid_string()[..8].to_lowercase()
    }

    pub fn is_open(&self) -> bool {
        self.resolved.is_none()
    }

    /// Refreshes the quote and its surrounding context (32 UTF-16 units each side) from the current text.
    pub fn capture(&mut self, text: &str) {
        if self.detached
            || self.range.end > text.len()
            || !text.is_char_boundary(self.range.start)
            || !text.is_char_boundary(self.range.end)
        {
            return;
        }
        self.quote = text[self.range.clone()].to_string();
        let pre = back_utf16(text, self.range.start, CONTEXT_UNITS);
        self.prefix = text[pre..self.range.start].to_string();
        let post = forward_utf16(text, self.range.end, CONTEXT_UNITS);
        self.suffix = text[self.range.end..post].to_string();
    }

    /// Follows an edit. `edited_range` is the post-edit byte range of new text; `delta` the byte length change.
    pub fn note_edit(&mut self, edited_range: Range<usize>, delta: isize) {
        if self.detached {
            return;
        }
        let pre_len = (edited_range.len() as isize - delta).max(0) as usize;
        let pre = edited_range.start..edited_range.start + pre_len;
        let (start, end) = (self.range.start, self.range.end);
        if pre.end <= start {
            let s = (start as isize + delta).max(0) as usize;
            self.range = s..s + (end - start);
            return;
        }
        if pre.start >= end {
            return;
        }
        // The edit overlaps the span: keep whatever part of the span survives plus the new text inside it.
        let new_start = if pre.start <= start {
            edited_range.start
        } else {
            start
        };
        let new_end = if pre.end >= end {
            edited_range.end as isize
        } else {
            end as isize + delta
        };
        let len = (new_end - new_start as isize).max(0) as usize;
        self.range = new_start..new_start + len;
        if len == 0 {
            self.detached = true;
        }
    }

    /// Finds the span again in `text`. Returns false when the quote no longer appears.
    pub fn reanchor(&mut self, text: &str) -> bool {
        if !self.quote.is_empty() && text.get(self.range.clone()) == Some(self.quote.as_str()) {
            self.detached = false;
            return true;
        }
        if self.quote.is_empty() {
            self.detached = true;
            return false;
        }
        let prefix_units = utf16_len(&self.prefix);
        let suffix_units = utf16_len(&self.suffix);
        let location_units = byte_to_utf16(text, self.range.start) as isize;
        let mut best: Option<Range<usize>> = None;
        let mut best_score = isize::MIN;
        let mut from = 0;
        while from < text.len() {
            let Some(i) = text[from..].find(&self.quote) else {
                break;
            };
            let r = from + i..from + i + self.quote.len();
            let before = &text[back_utf16(text, r.start, prefix_units)..r.start];
            let after = &text[r.end..forward_utf16(text, r.end, suffix_units)];
            // Distance in UTF-16 units, as Swift scored it.
            let distance = (byte_to_utf16(text, r.start) as isize - location_units).abs();
            let score = if before == self.prefix { 2 } else { 0 }
                + if after == self.suffix { 2 } else { 0 }
                - distance / 1000;
            if score > best_score {
                best_score = score;
                best = Some(r.clone());
            }
            // Overlapping occurrences count too: step one character, not past the match.
            from = r.start + text[r.start..].chars().next().map_or(1, char::len_utf8);
        }
        match best {
            Some(b) => {
                self.range = b;
                self.detached = false;
                true
            }
            None => {
                self.detached = true;
                false
            }
        }
    }

    fn to_wire(&self, text: &str) -> CommentWire {
        let location = byte_to_utf16(text, self.range.start);
        let end = byte_to_utf16(text, self.range.end.max(self.range.start));
        CommentWire {
            id: self.id.clone(),
            location,
            length: end - location,
            quote: self.quote.clone(),
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            body: self.body.clone(),
            author: self.author.clone(),
            created: self.created,
            replies: self.replies.clone(),
            resolved: self.resolved,
            detached: self.detached,
        }
    }

    fn from_wire(w: CommentWire, text: &str) -> Self {
        Comment {
            range: utf16_range_to_bytes(text, w.location, w.length),
            id: w.id,
            quote: w.quote,
            prefix: w.prefix,
            suffix: w.suffix,
            body: w.body,
            author: w.author,
            created: w.created,
            replies: w.replies,
            resolved: w.resolved,
            detached: w.detached,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct CommentWire {
    id: String,
    location: usize,
    length: usize,
    quote: String,
    prefix: String,
    suffix: String,
    body: String,
    author: String,
    #[serde(with = "dates::iso8601")]
    created: SystemTime,
    replies: Vec<CommentReply>,
    #[serde(
        default,
        with = "dates::iso8601_opt",
        skip_serializing_if = "Option::is_none"
    )]
    resolved: Option<SystemTime>,
    detached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionKind {
    Proposal,
    Snapshot,
}

/// An alternative to the draft. Agent replies arrive as proposals; using one keeps the text it replaced
/// as a snapshot, so every switch can be undone from the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftVersion {
    pub id: String,
    pub kind: VersionKind,
    pub label: String,
    /// "you", "Codex", "Claude", or the name an outside agent gave.
    pub source: String,
    #[serde(with = "dates::iso8601")]
    pub created: SystemTime,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback: Option<String>,
    #[serde(rename = "resolvesCommentIDs")]
    pub resolves_comment_ids: Vec<String>,
    #[serde(
        default,
        with = "dates::iso8601_opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub used_at: Option<SystemTime>,
}

impl DraftVersion {
    /// A version with a fresh id, created now, not yet used.
    pub fn new(
        kind: VersionKind,
        label: impl Into<String>,
        source: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        DraftVersion {
            id: Comment::new_id(),
            kind,
            label: label.into(),
            source: source.into(),
            created: SystemTime::now(),
            body: body.into(),
            feedback: None,
            resolves_comment_ids: Vec::new(),
            used_at: None,
        }
    }
}

/// One request to Assist and what came back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistExchange {
    #[serde(with = "dates::iso8601")]
    pub date: SystemTime,
    pub provider: String,
    pub action: String,
    pub instruction: String,
    pub feedback: String,
    #[serde(rename = "versionID", default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(rename = "commentIDs")]
    pub comment_ids: Vec<String>,
}

/// Everything Fluent Writer keeps about a draft besides its text.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentNotes {
    pub path: String,
    pub comments: Vec<Comment>,
    pub versions: Vec<DraftVersion>,
    pub transcript: Vec<AssistExchange>,
}

#[derive(Serialize, Deserialize)]
struct DocumentNotesWire {
    path: String,
    comments: Vec<CommentWire>,
    versions: Vec<DraftVersion>,
    transcript: Vec<AssistExchange>,
}

impl DocumentNotes {
    pub const MAX_VERSIONS: usize = 40;
    pub const MAX_TRANSCRIPT: usize = 200;

    pub fn new(path: impl Into<String>) -> Self {
        DocumentNotes {
            path: path.into(),
            comments: Vec::new(),
            versions: Vec::new(),
            transcript: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.comments.is_empty() && self.versions.is_empty() && self.transcript.is_empty()
    }

    pub fn open_comments(&self) -> Vec<&Comment> {
        self.comments.iter().filter(|c| c.is_open()).collect()
    }

    /// Adds a version, newest first, dropping the oldest unused ones past the cap.
    pub fn add_version(&mut self, v: DraftVersion) {
        self.versions.insert(0, v);
        while self.versions.len() > Self::MAX_VERSIONS {
            let i = self
                .versions
                .iter()
                .rposition(|v| v.used_at.is_none())
                .unwrap_or(self.versions.len() - 1);
            self.versions.remove(i);
        }
    }

    pub fn add_exchange(&mut self, e: AssistExchange) {
        self.transcript.push(e);
        if self.transcript.len() > Self::MAX_TRANSCRIPT {
            let extra = self.transcript.len() - Self::MAX_TRANSCRIPT;
            self.transcript.drain(..extra);
        }
    }

    pub fn resolve(&mut self, ids: &[String], note: Option<&str>, author: &str) {
        for c in self
            .comments
            .iter_mut()
            .filter(|c| ids.contains(&c.id) && c.is_open())
        {
            if let Some(note) = note.filter(|n| !n.is_empty()) {
                c.replies.push(CommentReply::new(author, note));
            }
            c.resolved = Some(SystemTime::now());
        }
    }

    /// Adds a comment on the `occurrence`-th appearance (1-based) of `quote`.
    pub fn add_comment(
        &mut self,
        quote: &str,
        occurrence: usize,
        body: &str,
        author: &str,
        text: &str,
    ) -> Result<Comment, DraftStoreError> {
        let ranges = edit_placement::occurrences(quote, text);
        if ranges.is_empty() {
            return Err(DraftStoreError::new("That text isn't in the draft."));
        }
        if occurrence < 1 || occurrence > ranges.len() {
            let n = ranges.len();
            return Err(DraftStoreError::new(format!(
                "The text appears {n} time{}; occurrence must be 1–{n}.",
                if n == 1 { "" } else { "s" }
            )));
        }
        let c = Comment::new(ranges[occurrence - 1].clone(), text, body, author);
        self.comments.push(c.clone());
        Ok(c)
    }

    /// The notes as the JSON the Swift app wrote, with comment spans converted to UTF-16 against `text`.
    pub fn to_json(&self, text: &str) -> serde_json::Result<Vec<u8>> {
        encode(&self.to_wire(text))
    }

    /// Reads notes JSON, converting comment spans from UTF-16 against `text`.
    pub fn from_json(data: &[u8], text: &str) -> Option<DocumentNotes> {
        let w: DocumentNotesWire = serde_json::from_slice(data).ok()?;
        Some(Self::from_wire(w, text))
    }

    fn to_wire(&self, text: &str) -> DocumentNotesWire {
        DocumentNotesWire {
            path: self.path.clone(),
            comments: self.comments.iter().map(|c| c.to_wire(text)).collect(),
            versions: self.versions.clone(),
            transcript: self.transcript.clone(),
        }
    }

    fn from_wire(w: DocumentNotesWire, text: &str) -> Self {
        DocumentNotes {
            path: w.path,
            comments: w
                .comments
                .into_iter()
                .map(|c| Comment::from_wire(c, text))
                .collect(),
            versions: w.versions,
            transcript: w.transcript,
        }
    }
}

/// Pretty-printed with sorted keys, as `NotesStore.encoder` wrote.
pub(crate) fn encode<T: Serialize>(value: &T) -> serde_json::Result<Vec<u8>> {
    fn sorted(v: serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Object(map) => {
                let mut entries: Vec<_> = map.into_iter().collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                serde_json::Value::Object(
                    entries.into_iter().map(|(k, v)| (k, sorted(v))).collect(),
                )
            }
            serde_json::Value::Array(a) => {
                serde_json::Value::Array(a.into_iter().map(sorted).collect())
            }
            other => other,
        }
    }
    serde_json::to_vec_pretty(&sorted(serde_json::to_value(value)?))
}

mod store;

pub use store::{InboxCommand, InboxCommandKind, InboxResult, LiveState, NotesStore};
