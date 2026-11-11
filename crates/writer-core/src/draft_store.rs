//! Drafts on disk: opening, atomic saving, recovery journals, the recents list, and local copies of
//! remote files (Swift `DraftDocument` and `DraftLibrary`).
//!
//! On-disk layout (unchanged from the Swift app), under the support directory:
//! - `state.json`: `{"records": {path: DraftRecord}, "lastOpenedPath": path}`, dates as seconds
//!   since 2001, selections in UTF-16 units.
//! - `Recovery/<UUID>.json`: one `RecoverySnapshot` per unsaved draft.
//! - `Remote/<host>[-<port>]/{home,root}/…`: local copies of remote files.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::SystemTime;

use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::app_variant::AppVariant;
use crate::atomic_file;
use crate::charsets::{is_character_newline, is_whitespace_or_newline, trim};
use crate::dates::{self, uuid_string};
use crate::error::{DraftStoreError, Error, Result, describe_io};
use crate::offsets::{
    byte_range_to_utf16, byte_to_utf16, utf16_len, utf16_range_to_bytes, utf16_to_byte,
};
use crate::paths::{extension, file_name, file_stem, path_string, standardize};
use crate::plain_text_exporter;
use crate::remote::RemoteLocation;
use crate::title_deriver;

/// What the library remembers about a file it has opened. Stored as-is in `state.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftRecord {
    pub path: String,
    pub title: String,
    pub has_manual_title: bool,
    /// UTF-16 units, as the Swift app stored it. Convert with [`utf16_range_to_bytes`] against the body.
    pub selection_location: usize,
    /// UTF-16 units.
    pub selection_length: usize,
    #[serde(with = "dates::reference")]
    pub last_opened: SystemTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteLocation>,
    /// The local mirror has changes the remote copy doesn't have yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_upload_pending: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryState {
    records: BTreeMap<String, DraftRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_opened_path: Option<String>,
}

/// A copy of unsaved text kept outside the draft file, used after a failed save or an interruption.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoverySnapshot {
    pub draft_id: Uuid,
    pub path: Option<String>,
    pub title: String,
    pub has_manual_title: bool,
    pub body: String,
    /// Byte offset into `body` (stored on disk in UTF-16 units).
    pub selection_location: usize,
    pub saved_at: SystemTime,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoverySnapshotWire {
    #[serde(rename = "draftID", with = "dates::uuid_upper")]
    draft_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    title: String,
    has_manual_title: bool,
    body: String,
    selection_location: usize,
    #[serde(with = "dates::reference")]
    saved_at: SystemTime,
}

impl RecoverySnapshot {
    fn to_wire(&self) -> RecoverySnapshotWire {
        RecoverySnapshotWire {
            draft_id: self.draft_id,
            path: self.path.clone(),
            title: self.title.clone(),
            has_manual_title: self.has_manual_title,
            body: self.body.clone(),
            selection_location: byte_to_utf16(&self.body, self.selection_location),
            saved_at: self.saved_at,
        }
    }

    fn from_wire(w: RecoverySnapshotWire) -> Self {
        let selection_location = utf16_to_byte(&w.body, w.selection_location);
        RecoverySnapshot {
            draft_id: w.draft_id,
            path: w.path,
            title: w.title,
            has_manual_title: w.has_manual_title,
            body: w.body,
            selection_location,
            saved_at: w.saved_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DraftSummary {
    pub path: PathBuf,
    pub title: String,
    /// The later of the file's modification time and when it was last opened.
    pub modified: SystemTime,
    pub preview: String,
    pub remote: Option<RemoteLocation>,
}

/// One entry of [`DraftLibrary::recent_paths`].
#[derive(Debug, Clone, PartialEq)]
pub struct RecentPath {
    pub path: String,
    pub title: String,
    pub last_opened: SystemTime,
}

/// An open draft. `body`, `title`, `has_manual_title` and `selection` belong to the editor; the rest
/// is managed by [`DraftLibrary`].
#[derive(Debug, Clone, PartialEq)]
pub struct DraftDocument {
    id: Uuid,
    path: Option<PathBuf>,
    pub body: String,
    pub title: String,
    pub has_manual_title: bool,
    /// Byte range into `body`.
    pub selection: Range<usize>,
    saved_body: String,
    recovered_at: Option<SystemTime>,
    remote: Option<RemoteLocation>,
    remote_upload_pending: bool,
}

impl DraftDocument {
    fn new(
        id: Uuid,
        path: Option<PathBuf>,
        body: String,
        saved_body: String,
        title: String,
        has_manual_title: bool,
        selection: Range<usize>,
    ) -> Self {
        DraftDocument {
            id,
            path,
            body,
            title,
            has_manual_title,
            selection,
            saved_body,
            recovered_at: None,
            remote: None,
            remote_upload_pending: false,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The file on disk (Swift `url`). For remote files, the local mirror.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn saved_body(&self) -> &str {
        &self.saved_body
    }

    /// Set when the body was restored from a recovery snapshot rather than the file.
    pub fn recovered_at(&self) -> Option<SystemTime> {
        self.recovered_at
    }

    /// Set for files edited over SSH; `path` is then the local mirror.
    pub fn remote(&self) -> Option<&RemoteLocation> {
        self.remote.as_ref()
    }

    /// The mirror holds saved text that hasn't reached the remote file yet.
    pub fn remote_upload_pending(&self) -> bool {
        self.remote_upload_pending
    }

    pub fn is_dirty(&self) -> bool {
        self.body != self.saved_body
    }

    pub fn is_empty_untitled(&self) -> bool {
        self.path.is_none() && self.body.is_empty()
    }

    /// Keeps the title in step with the first line until the writer names the draft.
    pub fn refresh_derived_title(&mut self) {
        if self.has_manual_title {
            return;
        }
        self.title = title_deriver::derived_title(&self.body);
    }

    /// The selection as UTF-16 `(location, length)`, as persisted.
    fn selection_utf16(&self) -> (usize, usize) {
        byte_range_to_utf16(&self.body, &self.selection)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveResult {
    Unchanged,
    Saved,
}

static NUMBER_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r" \d+$").expect("valid pattern"));

pub struct DraftLibrary {
    drafts_directory: PathBuf,
    support_directory: PathBuf,
    state: LibraryState,
}

pub const FILE_EXTENSIONS: [&str; 4] = ["md", "markdown", "txt", "text"];

fn load_state(support_directory: &Path) -> Option<LibraryState> {
    let data = fs::read(support_directory.join("state.json")).ok()?;
    serde_json::from_slice(&data).ok()
}

impl DraftLibrary {
    pub fn new(
        drafts_directory: impl Into<PathBuf>,
        support_directory: impl Into<PathBuf>,
    ) -> Result<Self> {
        let drafts_directory = drafts_directory.into();
        let support_directory = support_directory.into();
        let mut lib = DraftLibrary {
            drafts_directory,
            support_directory,
            state: LibraryState::default(),
        };
        fs::create_dir_all(&lib.drafts_directory)?;
        fs::create_dir_all(&lib.support_directory)?;
        fs::create_dir_all(lib.recovery_directory())?;
        atomic_file::remove_stale_temporaries(&lib.drafts_directory);
        atomic_file::remove_stale_temporaries(&lib.support_directory);
        atomic_file::remove_stale_temporaries(&lib.recovery_directory());
        if let Some(state) = load_state(&lib.support_directory) {
            lib.state = state;
        }
        Ok(lib)
    }

    pub fn default_library(variant: AppVariant) -> Result<Self> {
        Self::new(variant.drafts_directory(), variant.support_directory())
    }

    pub fn drafts_directory(&self) -> &Path {
        &self.drafts_directory
    }

    pub fn support_directory(&self) -> &Path {
        &self.support_directory
    }

    pub fn recovery_directory(&self) -> PathBuf {
        self.support_directory.join("Recovery")
    }

    fn state_path(&self) -> PathBuf {
        self.support_directory.join("state.json")
    }

    pub fn remote_mirror_directory(&self) -> PathBuf {
        self.support_directory.join("Remote")
    }

    /// Recently opened paths, newest first, read without touching anything on disk.
    pub fn recent_paths(support_directory: &Path) -> Vec<RecentPath> {
        let Some(state) = load_state(support_directory) else {
            return Vec::new();
        };
        let mut records: Vec<&DraftRecord> = state.records.values().collect();
        records.sort_by_key(|r| std::cmp::Reverse(r.last_opened));
        records
            .into_iter()
            .map(|r| RecentPath {
                path: r.path.clone(),
                title: r.title.clone(),
                last_opened: r.last_opened,
            })
            .collect()
    }

    // MARK: Opening

    pub fn new_draft(&self) -> DraftDocument {
        DraftDocument::new(
            Uuid::new_v4(),
            None,
            String::new(),
            String::new(),
            title_deriver::UNTITLED.to_string(),
            false,
            0..0,
        )
    }

    pub fn last_opened_path(&self) -> Option<PathBuf> {
        let p = self.state.last_opened_path.as_ref()?;
        Path::new(p).exists().then(|| PathBuf::from(p))
    }

    pub fn open(&mut self, path: &Path) -> Result<DraftDocument> {
        let standardized = standardize(path);
        let key = path_string(&standardized);
        let data = fs::read(&standardized).map_err(|e| {
            DraftStoreError::new(format!(
                "Couldn't open “{}”: {}",
                file_name(&standardized),
                describe_io(&e)
            ))
        })?;
        // Not UTF-8: ISO Latin 1 decodes any bytes, as the Swift fallback did.
        let body = String::from_utf8(data)
            .unwrap_or_else(|e| e.into_bytes().iter().map(|&b| b as char).collect());
        let record = self.state.records.get(&key).cloned();
        let title = record
            .as_ref()
            .map(|r| r.title.clone())
            .unwrap_or_else(|| file_stem(&standardized));
        let length = utf16_len(&body);
        let loc = record
            .as_ref()
            .map_or(length, |r| r.selection_location)
            .min(length);
        let len = record
            .as_ref()
            .map_or(0, |r| r.selection_length)
            .min(length - loc);
        let selection = utf16_range_to_bytes(&body, loc, len);
        let mut doc = DraftDocument::new(
            Uuid::new_v4(),
            Some(standardized),
            body.clone(),
            body.clone(),
            title,
            record.as_ref().is_some_and(|r| r.has_manual_title),
            selection,
        );
        doc.remote = record.as_ref().and_then(|r| r.remote.clone());
        doc.remote_upload_pending =
            record.as_ref().and_then(|r| r.remote_upload_pending) == Some(true);
        if let Some(snapshot) = self
            .recovery_snapshots()
            .into_iter()
            .find(|s| s.path.as_deref() == Some(key.as_str()))
        {
            if snapshot.body != body {
                let at = snapshot.selection_location.min(snapshot.body.len());
                doc.body = snapshot.body;
                doc.recovered_at = Some(snapshot.saved_at);
                doc.title = snapshot.title;
                doc.has_manual_title = snapshot.has_manual_title;
                doc.selection = at..at;
            } else {
                self.discard_recovery(snapshot.draft_id);
            }
        }
        self.mark_opened(&doc);
        Ok(doc)
    }

    /// Restores an unsaved draft that never reached a file.
    pub fn restore(&self, snapshot: &RecoverySnapshot) -> DraftDocument {
        let at = snapshot.selection_location.min(snapshot.body.len());
        let mut doc = DraftDocument::new(
            snapshot.draft_id,
            None,
            snapshot.body.clone(),
            String::new(),
            snapshot.title.clone(),
            snapshot.has_manual_title,
            at..at,
        );
        doc.recovered_at = Some(snapshot.saved_at);
        doc
    }

    /// Recovery snapshots for drafts that were never saved to a file.
    pub fn orphaned_recoveries(&self) -> Vec<RecoverySnapshot> {
        self.recovery_snapshots()
            .into_iter()
            .filter(|s| s.path.as_ref().is_none_or(|p| !Path::new(p).exists()))
            .collect()
    }

    /// Every readable recovery snapshot, newest first.
    pub fn recovery_snapshots(&self) -> Vec<RecoverySnapshot> {
        let Ok(entries) = fs::read_dir(self.recovery_directory()) else {
            return Vec::new();
        };
        let mut out: Vec<RecoverySnapshot> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
            .filter_map(|e| {
                let data = fs::read(e.path()).ok()?;
                serde_json::from_slice::<RecoverySnapshotWire>(&data)
                    .ok()
                    .map(RecoverySnapshot::from_wire)
            })
            .collect();
        out.sort_by_key(|s| std::cmp::Reverse(s.saved_at));
        out
    }

    // MARK: Saving

    /// Saves the document atomically. On failure, a recovery snapshot is written and the error is returned.
    pub fn save(&mut self, doc: &mut DraftDocument) -> Result<SaveResult> {
        if doc.is_empty_untitled() {
            return Ok(SaveResult::Unchanged);
        }
        if doc.path.is_some()
            && !doc.is_dirty()
            && doc.recovered_at.is_none()
            && !doc.remote_upload_pending
        {
            return Ok(SaveResult::Unchanged);
        }
        let body = doc.body.clone();
        if doc.path.is_none() {
            doc.path =
                Some(self.unique_path(&doc.title, &self.drafts_directory.clone(), None, "md"));
        }
        let path = doc.path.clone().expect("set above");
        if doc.is_dirty() || doc.recovered_at.is_some() || !path.exists() {
            if let Err(e) = atomic_file::write(body.as_bytes(), &path) {
                let _ = self.write_recovery(doc);
                return Err(Error::FileWrite(e));
            }
            if doc.remote.is_some() {
                doc.remote_upload_pending = true;
            }
        }
        doc.saved_body = body;
        doc.recovered_at = None;
        self.discard_recovery(doc.id);
        let key = path_string(&path);
        if let Some(stale) = self
            .recovery_snapshots()
            .into_iter()
            .find(|s| s.path.as_deref() == Some(key.as_str()))
        {
            self.discard_recovery(stale.draft_id);
        }
        self.rename_to_match_derived_title_if_needed(doc);
        self.mark_opened(doc);
        Ok(SaveResult::Saved)
    }

    pub fn write_recovery(&self, doc: &DraftDocument) -> Result<()> {
        let snapshot = RecoverySnapshot {
            draft_id: doc.id,
            path: doc.path.as_deref().map(path_string),
            title: doc.title.clone(),
            has_manual_title: doc.has_manual_title,
            body: doc.body.clone(),
            selection_location: doc.selection.start,
            saved_at: SystemTime::now(),
        };
        let data = serde_json::to_vec(&snapshot.to_wire()).map_err(std::io::Error::other)?;
        atomic_file::write(&data, &self.recovery_path(doc.id))?;
        Ok(())
    }

    fn recovery_path(&self, id: Uuid) -> PathBuf {
        self.recovery_directory()
            .join(format!("{}.json", uuid_string(&id)))
    }

    pub fn discard_recovery(&self, id: Uuid) {
        let _ = fs::remove_file(self.recovery_path(id));
    }

    pub fn has_recovery(&self, doc: &DraftDocument) -> bool {
        self.recovery_path(doc.id).exists()
    }

    /// Untitled drafts inside the drafts folder follow their first line; files elsewhere keep their name.
    fn rename_to_match_derived_title_if_needed(&mut self, doc: &mut DraftDocument) {
        let Some(path) = doc.path.clone() else { return };
        if doc.has_manual_title || !self.is_in_drafts_directory(&path) {
            return;
        }
        let desired_stem = title_deriver::file_name_stem(&doc.title);
        let current_stem = file_stem(&path);
        if current_stem == desired_stem {
            return;
        }
        if let Some(m) = NUMBER_SUFFIX.find(&current_stem)
            && current_stem[..m.start()] == desired_stem
        {
            return;
        }
        let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let target = self.unique_path(&doc.title, &parent, Some(&path), "md");
        self.move_doc(doc, &target);
    }

    pub fn rename(&mut self, doc: &mut DraftDocument, new_title: &str) -> Result<()> {
        let trimmed = trim(new_title, is_whitespace_or_newline);
        if trimmed.is_empty() {
            doc.has_manual_title = false;
            doc.refresh_derived_title();
        } else {
            doc.title = trimmed.to_string();
            doc.has_manual_title = true;
        }
        let Some(path) = doc.path.clone().filter(|_| doc.remote.is_none()) else {
            self.mark_opened(doc);
            return Ok(());
        };
        let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let target = self.unique_path(&doc.title, &parent, Some(&path), &extension(&path));
        if target != path {
            move_item(&path, &target).map_err(|e| {
                DraftStoreError::new(format!(
                    "Couldn't rename “{}”: {}",
                    file_name(&path),
                    describe_io(&e)
                ))
            })?;
            self.forget(&path);
            doc.path = Some(target);
        }
        self.mark_opened(doc);
        Ok(())
    }

    fn move_doc(&mut self, doc: &mut DraftDocument, target: &Path) {
        let Some(path) = doc.path.clone() else { return };
        if target == path {
            return;
        }
        if move_item(&path, target).is_ok() {
            self.forget(&path);
            doc.path = Some(target.to_path_buf());
        }
    }

    fn is_in_drafts_directory(&self, path: &Path) -> bool {
        path.parent().map(standardize) == Some(standardize(&self.drafts_directory))
    }

    fn unique_path(
        &self,
        title: &str,
        directory: &Path,
        excluding: Option<&Path>,
        path_extension: &str,
    ) -> PathBuf {
        let stem = title_deriver::file_name_stem(title);
        let ext = if path_extension.is_empty() {
            "md"
        } else {
            path_extension
        };
        let excluded = excluding.map(standardize);
        let mut candidate = directory.join(format!("{stem}.{ext}"));
        let mut n = 2;
        while candidate.exists() && Some(standardize(&candidate)) != excluded {
            candidate = directory.join(format!("{stem} {n}.{ext}"));
            n += 1;
        }
        standardize(&candidate)
    }

    // MARK: State

    pub fn mark_opened(&mut self, doc: &DraftDocument) {
        let Some(path) = doc.path.as_deref() else {
            return;
        };
        let key = path_string(path);
        let (loc, len) = doc.selection_utf16();
        self.state.records.insert(
            key.clone(),
            DraftRecord {
                path: key.clone(),
                title: doc.title.clone(),
                has_manual_title: doc.has_manual_title,
                selection_location: loc,
                selection_length: len,
                last_opened: SystemTime::now(),
                remote: doc.remote.clone(),
                remote_upload_pending: doc.remote_upload_pending.then_some(true),
            },
        );
        self.state.last_opened_path = Some(key);
        self.persist_state();
    }

    /// Remembers the caret without touching the draft file.
    pub fn remember_selection(&mut self, doc: &DraftDocument) {
        let Some(path) = doc.path.as_deref() else {
            return;
        };
        let (loc, len) = doc.selection_utf16();
        let Some(record) = self.state.records.get_mut(&path_string(path)) else {
            return;
        };
        record.selection_location = loc;
        record.selection_length = len;
        record.title = doc.title.clone();
        record.has_manual_title = doc.has_manual_title;
        self.persist_state();
    }

    fn forget(&mut self, path: &Path) {
        self.state.records.remove(&path_string(path));
    }

    fn persist_state(&self) {
        let Ok(data) = serde_json::to_vec(&self.state) else {
            return;
        };
        let _ = atomic_file::write(&data, &self.state_path());
    }

    /// What the library remembers about `path`, if anything.
    pub fn record_for_path(&self, path: &Path) -> Option<&DraftRecord> {
        self.state.records.get(&path_string(&standardize(path)))
    }

    // MARK: Listing

    pub fn recent_drafts(&self, limit: usize) -> Vec<DraftSummary> {
        let mut paths = BTreeSet::new();
        if let Ok(entries) = fs::read_dir(&self.drafts_directory) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let p = standardize(&self.drafts_directory.join(&name));
                if FILE_EXTENSIONS.contains(&extension(&p).to_lowercase().as_str()) {
                    paths.insert(p);
                }
            }
        }
        for key in self.state.records.keys() {
            if Path::new(key).exists() {
                paths.insert(standardize(Path::new(key)));
            }
        }
        let mut summaries: Vec<DraftSummary> = paths
            .into_iter()
            .map(|p| {
                let modified = fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let record = self.state.records.get(&path_string(&p));
                let opened = record.map_or(SystemTime::UNIX_EPOCH, |r| r.last_opened);
                let title = record
                    .map(|r| r.title.clone())
                    .or_else(|| record.and_then(|r| r.remote.as_ref()).map(|r| r.name()))
                    .unwrap_or_else(|| file_stem(&p));
                DraftSummary {
                    preview: preview_text(&p),
                    title,
                    modified: modified.max(opened),
                    remote: record.and_then(|r| r.remote.clone()),
                    path: p,
                }
            })
            .collect();
        summaries.sort_by_key(|s| std::cmp::Reverse(s.modified));
        summaries.truncate(limit);
        summaries
    }

    // MARK: Remote files

    /// Where the local copy of a remote file lives. Saves land here first, then upload.
    pub fn mirror_path(&self, loc: &RemoteLocation) -> PathBuf {
        let host_dir = match loc.port {
            Some(port) => format!("{}-{}", loc.host, port),
            None => loc.host.clone(),
        }
        .replace('/', "_");
        let mut path = loc.path.as_str();
        let root = if path == "~" || path.starts_with("~/") {
            path = if path.len() >= 2 { &path[2..] } else { "" };
            "home"
        } else {
            "root"
        };
        let mut out = self
            .remote_mirror_directory()
            .join(windows_safe(&host_dir))
            .join(root);
        for c in path
            .split('/')
            .filter(|c| *c != "." && *c != ".." && !c.is_empty())
        {
            out.push(windows_safe(c));
        }
        standardize(&out)
    }

    pub fn record_for_remote(&self, loc: &RemoteLocation) -> Option<&DraftRecord> {
        self.state.records.get(&path_string(&self.mirror_path(loc)))
    }

    /// Opens a remote file from freshly fetched contents. Local edits that never reached the remote are kept instead.
    pub fn open_remote(&mut self, loc: &RemoteLocation, contents: &[u8]) -> Result<DraftDocument> {
        let mirror = self.mirror_path(loc);
        let pending = self
            .state
            .records
            .get(&path_string(&mirror))
            .and_then(|r| r.remote_upload_pending)
            == Some(true)
            && mirror.exists();
        if !pending {
            if let Some(parent) = mirror.parent() {
                fs::create_dir_all(parent)?;
            }
            atomic_file::write(contents, &mirror)?;
        }
        let mut doc = self.open(&mirror)?;
        doc.remote = Some(loc.clone());
        doc.remote_upload_pending = pending;
        if !doc.has_manual_title {
            doc.title = loc.name();
        }
        self.mark_opened(&doc);
        Ok(doc)
    }

    /// Records the result of uploading `uploaded_body`. The flag clears only if nothing newer was saved since.
    pub fn remote_upload_finished(
        &mut self,
        doc: &mut DraftDocument,
        uploaded_body: &str,
        succeeded: bool,
    ) {
        if doc.remote.is_none() {
            return;
        }
        if succeeded && uploaded_body == doc.saved_body {
            doc.remote_upload_pending = false;
        }
        if let Some(path) = doc.path.as_deref()
            && let Some(record) = self.state.records.get_mut(&path_string(path))
        {
            record.remote_upload_pending = doc.remote_upload_pending.then_some(true);
            self.persist_state();
        }
    }
}

/// Renames `from` to `to`, failing (like `FileManager.moveItem`) when `to` already exists.
fn move_item(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "a file with that name already exists",
        ));
    }
    fs::rename(from, to)
}

/// On Windows, replaces characters Windows forbids in file names. Unchanged elsewhere, so mirror
/// paths stay where the Swift app put them on macOS.
fn windows_safe(component: &str) -> String {
    if cfg!(windows) {
        component
            .chars()
            .map(|c| {
                if matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\\') || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .collect()
    } else {
        component.to_string()
    }
}

fn preview_text(path: &Path) -> String {
    use std::io::Read;
    let Ok(file) = fs::File::open(path) else {
        return String::new();
    };
    let mut data = Vec::new();
    let _ = file.take(600).read_to_end(&mut data);
    let text = String::from_utf8_lossy(&data);
    let plain = plain_text_exporter::export(&text);
    let joined = plain
        .split(is_character_newline)
        .filter(|l| !l.is_empty())
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");
    joined.graphemes(true).take(140).collect()
}
