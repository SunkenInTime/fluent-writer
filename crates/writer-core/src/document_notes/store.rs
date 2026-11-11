//! The notes store, the inbox outside agents post commands to, and the live state file.

use std::collections::HashSet;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{CommentReply, DocumentNotes, DocumentNotesWire, DraftVersion, VersionKind, encode};
use crate::atomic_file;
use crate::dates::{self, new_uuid_string};
use crate::edit_placement::{self, EditFailure, ProposedEdit};
use crate::error::{DraftStoreError, Result};
use crate::offsets::{byte_to_utf16, floor_char_boundary, utf16_range_to_bytes};
use crate::paths::{path_string, standardize};

// MARK: Store

/// Notes live in the support folder, one file per draft path, so drafts stay plain Markdown.
/// The app is the only writer while it runs; outside agents send it commands through the inbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesStore {
    pub support_directory: PathBuf,
}

impl NotesStore {
    pub fn new(support_directory: impl Into<PathBuf>) -> Self {
        NotesStore {
            support_directory: support_directory.into(),
        }
    }

    pub fn notes_directory(&self) -> PathBuf {
        self.support_directory.join("Notes")
    }

    pub fn inbox_directory(&self) -> PathBuf {
        self.support_directory.join("Inbox")
    }

    pub fn results_directory(&self) -> PathBuf {
        self.inbox_directory().join("Results")
    }

    pub fn live_state_path(&self) -> PathBuf {
        self.support_directory.join("Live.json")
    }

    /// The first 12 bytes of the SHA-256 of the standardized path, in hex.
    pub fn key(path: &str) -> String {
        let standardized = path_string(&standardize(Path::new(path)));
        let digest = Sha256::digest(standardized.as_bytes());
        digest[..12].iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The notes file for a draft path.
    pub fn path_for(&self, path: &str) -> PathBuf {
        self.notes_directory()
            .join(format!("{}.json", Self::key(path)))
    }

    /// The notes for `path`, with comment spans resolved against `text` (the draft's current text).
    /// Empty notes when there are none or the file can't be read.
    pub fn load(&self, path: &str, text: &str) -> DocumentNotes {
        fs::read(self.path_for(path))
            .ok()
            .and_then(|d| DocumentNotes::from_json(&d, text))
            .unwrap_or_else(|| DocumentNotes::new(path))
    }

    /// Saves `notes`, whose comment spans refer to `text`. Empty notes remove the file.
    pub fn save(&self, notes: &DocumentNotes, text: &str) -> Result<()> {
        let target = self.path_for(&notes.path);
        if notes.is_empty() {
            let _ = fs::remove_file(&target);
            return Ok(());
        }
        fs::create_dir_all(self.notes_directory())?;
        let data = notes.to_json(text).map_err(std::io::Error::other)?;
        atomic_file::write(&data, &target)?;
        Ok(())
    }

    /// Runs `body` holding an exclusive lock on the notes folder, so separate processes
    /// (the app, several MCP servers) can read-modify-write notes without losing each other's changes.
    pub fn with_lock<T>(&self, body: impl FnOnce() -> T) -> T {
        let _ = fs::create_dir_all(self.notes_directory());
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.notes_directory().join(".lock"));
        let locked = file.as_ref().ok().filter(|f| f.lock().is_ok());
        let result = body();
        if let Some(f) = locked {
            let _ = f.unlock();
        }
        result
    }

    /// Carries notes along when a draft's file is renamed. The JSON is moved as stored, so no text is needed.
    pub fn move_notes(&self, old: &str, new: &str) {
        if old == new {
            return;
        }
        let Some(mut wire) = fs::read(self.path_for(old))
            .ok()
            .and_then(|d| serde_json::from_slice::<DocumentNotesWire>(&d).ok())
        else {
            return;
        };
        if wire.comments.is_empty() && wire.versions.is_empty() && wire.transcript.is_empty() {
            return;
        }
        wire.path = new.to_string();
        let saved = fs::create_dir_all(self.notes_directory())
            .ok()
            .and_then(|_| encode(&wire).ok())
            .and_then(|data| atomic_file::write(&data, &self.path_for(new)).ok());
        if saved.is_some() {
            let _ = fs::remove_file(self.path_for(old));
        }
    }

    // MARK: Live state

    pub fn write_live_state(&self, state: &LiveState) {
        let _ = fs::create_dir_all(&self.support_directory);
        let Ok(data) = encode(state) else { return };
        let _ = atomic_file::write(&data, &self.live_state_path());
    }

    pub fn clear_live_state(&self) {
        let _ = fs::remove_file(self.live_state_path());
    }

    /// The running app's state, or `None` when it isn't running.
    pub fn live_state(&self) -> Option<LiveState> {
        let data = fs::read(self.live_state_path()).ok()?;
        let state: LiveState = serde_json::from_slice(&data).ok()?;
        process_is_running(state.pid).then_some(state)
    }

    // MARK: Inbox

    pub fn post(&self, command: &InboxCommand) -> Result<()> {
        fs::create_dir_all(self.inbox_directory())?;
        let data = encode(command).map_err(std::io::Error::other)?;
        atomic_file::write(
            &data,
            &self.inbox_directory().join(format!("{}.json", command.id)),
        )?;
        Ok(())
    }

    /// Pending commands, oldest first, with the file each came from.
    pub fn pending_commands(&self) -> Vec<(PathBuf, InboxCommand)> {
        let Ok(entries) = fs::read_dir(self.inbox_directory()) else {
            return Vec::new();
        };
        let mut out: Vec<(PathBuf, InboxCommand)> = entries
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.ends_with(".json") && !n.starts_with('.')
            })
            .filter_map(|e| {
                let data = fs::read(e.path()).ok()?;
                let c: InboxCommand = serde_json::from_slice(&data).ok()?;
                Some((e.path(), c))
            })
            .collect();
        out.sort_by_key(|a| a.1.created);
        out
    }

    pub fn write_result(&self, result: &InboxResult) {
        let _ = fs::create_dir_all(self.results_directory());
        let Ok(data) = encode(result) else { return };
        let _ = atomic_file::write(
            &data,
            &self.results_directory().join(format!("{}.json", result.id)),
        );
    }

    /// Waits (blocking, polling every 50 ms) for the app to answer a command. `None` on timeout.
    pub fn await_result(&self, id: &str, timeout: Duration) -> Option<InboxResult> {
        let u = self.results_directory().join(format!("{id}.json"));
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(r) = fs::read(&u)
                .ok()
                .and_then(|d| serde_json::from_slice::<InboxResult>(&d).ok())
            {
                let _ = fs::remove_file(&u);
                return Some(r);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    // MARK: Applying commands

    /// Applies a command to `notes`, given the draft's current text. Shared by the app and by
    /// outside agents when the app isn't running.
    pub fn apply(command: &InboxCommand, notes: &mut DocumentNotes, text: &str) -> InboxResult {
        match Self::apply_inner(command, notes, text) {
            Ok(r) => r,
            Err(e) => InboxResult::new(&command.id, false, e.message),
        }
    }

    fn apply_inner(
        command: &InboxCommand,
        notes: &mut DocumentNotes,
        text: &str,
    ) -> Result<InboxResult, DraftStoreError> {
        match command.kind {
            InboxCommandKind::AddComment => {
                let (Some(quote), Some(body)) = (
                    command.quote.as_deref().filter(|q| !q.is_empty()),
                    command.body.as_deref().filter(|b| !b.is_empty()),
                ) else {
                    return Err(DraftStoreError::new(
                        "add_comment needs the quoted text and a comment.",
                    ));
                };
                let c = notes.add_comment(
                    quote,
                    command.occurrence.unwrap_or(1),
                    body,
                    &command.agent,
                    text,
                )?;
                let mut r = InboxResult::new(&command.id, true, format!("Added comment {}.", c.id));
                r.comment_id = Some(c.id);
                Ok(r)
            }
            InboxCommandKind::ReplyToComment | InboxCommandKind::ResolveComment => {
                let Some((id, i)) = command.comment_id.as_ref().and_then(|id| {
                    notes
                        .comments
                        .iter()
                        .position(|c| &c.id == id)
                        .map(|i| (id.clone(), i))
                }) else {
                    return Err(DraftStoreError::new("No comment with that id."));
                };
                if command.kind == InboxCommandKind::ResolveComment {
                    notes.resolve(
                        std::slice::from_ref(&id),
                        command.body.as_deref(),
                        &command.agent,
                    );
                    let mut r =
                        InboxResult::new(&command.id, true, format!("Resolved comment {id}."));
                    r.comment_id = Some(id);
                    return Ok(r);
                }
                let Some(body) = command.body.as_deref().filter(|b| !b.is_empty()) else {
                    return Err(DraftStoreError::new("The reply is empty."));
                };
                notes.comments[i]
                    .replies
                    .push(CommentReply::new(&command.agent, body));
                let mut r =
                    InboxResult::new(&command.id, true, format!("Replied to comment {id}."));
                r.comment_id = Some(id);
                Ok(r)
            }
            InboxCommandKind::ProposeVersion => {
                let edits = command.edits.clone().unwrap_or_default();
                let body: String;
                let mut failures: Vec<EditFailure> = Vec::new();
                let placed_ids: Vec<String>;
                if let Some(full) = &command.full_text {
                    body = full.clone();
                    placed_ids = edits.iter().flat_map(|e| e.comment_ids.clone()).collect();
                } else {
                    let placement = edit_placement::place(&edits, text, None, None);
                    failures = placement.failed.clone();
                    if placement.placed.is_empty() {
                        let reason = failures
                            .first()
                            .map_or("The version has no edits.".to_string(), |f| {
                                f.reason.clone()
                            });
                        return Err(DraftStoreError::new(reason));
                    }
                    body = placement.applied(text);
                    placed_ids = placement
                        .placed
                        .iter()
                        .flat_map(|p| edits[p.index].comment_ids.clone())
                        .collect();
                }
                if body == text {
                    return Err(DraftStoreError::new(
                        "The version is identical to the draft.",
                    ));
                }
                // Only edits that landed resolve their comments. (Swift went through a Set, so the
                // order was unspecified; here the first occurrence order is kept.)
                let mut seen = HashSet::new();
                let ids: Vec<String> = command
                    .resolves_comment_ids
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .chain(placed_ids)
                    .filter(|id| seen.insert(id.clone()))
                    .collect();
                let label = command
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("{} version", command.agent));
                let mut v = DraftVersion::new(VersionKind::Proposal, label, &command.agent, body);
                v.feedback = command.body.clone();
                v.resolves_comment_ids = ids;
                let vid = v.id.clone();
                notes.add_version(v);
                let mut message = format!(
                    "Created version {vid}. The author can preview it in Fluent Writer and choose whether to use it."
                );
                if !failures.is_empty() {
                    let n = failures.len();
                    let list: Vec<String> = failures
                        .iter()
                        .map(|f| format!("#{} {}", f.index + 1, f.reason))
                        .collect();
                    message += &format!(
                        " {n} edit{} couldn't be placed: {}",
                        if n == 1 { "" } else { "s" },
                        list.join(" ")
                    );
                }
                let mut r = InboxResult::new(&command.id, true, message);
                r.version_id = Some(vid);
                Ok(r)
            }
        }
    }
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 only checks that the process exists and may be signalled.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the handle is checked before use and closed afterwards.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code) != 0;
        CloseHandle(h);
        ok && code == STILL_ACTIVE as u32
    }
}

#[cfg(not(any(unix, windows)))]
fn process_is_running(_pid: u32) -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InboxCommandKind {
    AddComment,
    ReplyToComment,
    ResolveComment,
    ProposeVersion,
}

/// A change an outside agent asks the app to make.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxCommand {
    pub id: String,
    #[serde(with = "dates::iso8601")]
    pub created: SystemTime,
    pub kind: InboxCommandKind,
    pub path: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(rename = "commentID", default, skip_serializing_if = "Option::is_none")]
    pub comment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<ProposedEdit>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_text: Option<String>,
    #[serde(
        rename = "resolvesCommentIDs",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub resolves_comment_ids: Option<Vec<String>>,
}

impl InboxCommand {
    /// A command with a fresh upper-case UUID id, created now.
    pub fn new(kind: InboxCommandKind, path: impl Into<String>, agent: impl Into<String>) -> Self {
        InboxCommand {
            id: new_uuid_string(),
            created: SystemTime::now(),
            kind,
            path: path.into(),
            agent: agent.into(),
            quote: None,
            occurrence: None,
            body: None,
            comment_id: None,
            label: None,
            edits: None,
            full_text: None,
            resolves_comment_ids: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxResult {
    pub id: String,
    pub ok: bool,
    pub message: String,
    #[serde(rename = "commentID", default, skip_serializing_if = "Option::is_none")]
    pub comment_id: Option<String>,
    #[serde(rename = "versionID", default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
}

impl InboxResult {
    pub fn new(id: impl Into<String>, ok: bool, message: impl Into<String>) -> Self {
        InboxResult {
            id: id.into(),
            ok,
            message: message.into(),
            comment_id: None,
            version_id: None,
        }
    }
}

/// What the running app is showing, for outside agents. The selection is stored in UTF-16 units,
/// as the Swift app wrote it; [`LiveState::new`] and [`LiveState::selection`] convert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveState {
    pub pid: u32,
    pub app_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    pub title: String,
    /// UTF-16 units.
    pub selection_location: usize,
    /// UTF-16 units.
    pub selection_length: usize,
    pub selected_text: String,
    #[serde(
        rename = "previewingVersionID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub previewing_version_id: Option<String>,
    #[serde(with = "dates::iso8601")]
    pub updated: SystemTime,
}

impl LiveState {
    /// `selection` is a byte range of `text`, the draft being shown. `updated` is now.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pid: u32,
        app_name: impl Into<String>,
        path: Option<String>,
        remote: Option<String>,
        title: impl Into<String>,
        text: &str,
        selection: Range<usize>,
        previewing_version_id: Option<String>,
    ) -> Self {
        let start = floor_char_boundary(text, selection.start);
        let end = floor_char_boundary(text, selection.end.max(selection.start));
        let location = byte_to_utf16(text, start);
        LiveState {
            pid,
            app_name: app_name.into(),
            path,
            remote,
            title: title.into(),
            selection_location: location,
            selection_length: byte_to_utf16(text, end) - location,
            selected_text: text[start..end].to_string(),
            previewing_version_id,
            updated: SystemTime::now(),
        }
    }

    /// The selection as a byte range of `text`.
    pub fn selection(&self, text: &str) -> Range<usize> {
        utf16_range_to_bytes(text, self.selection_location, self.selection_length)
    }
}
