//! The UI-free core of Fluent Writer, ported from the Swift `WriterCore` module.
//!
//! Every text offset in this API is a UTF-8 byte offset into a `&str`, and ranges are
//! `Range<usize>`. The Swift app worked in NSString UTF-16 offsets; files it persisted with
//! UTF-16 offsets (the library state, recovery journals, notes and the live state) still store
//! UTF-16 on disk and are converted when they are read or written. [`utf16_to_byte`] and
//! [`byte_to_utf16`] do the conversion.
//!
//! Everything here is blocking and synchronous, so the UI can call it from a worker thread.

pub mod app_variant;
pub mod atomic_file;
pub mod document_notes;
pub mod draft_store;
pub mod edit_placement;
pub mod markdown;
pub mod markdown_document;
pub mod markdown_images;
pub mod plain_text_exporter;
pub mod remote;
pub mod text_editing;
pub mod text_stats;
pub mod text_units;
pub mod title_deriver;
pub mod word_diff;

mod charsets;
mod dates;
mod error;
mod offsets;
mod paths;

pub use app_variant::AppVariant;
pub use atomic_file::FileWriteError;
pub use document_notes::{
    AssistExchange, Comment, CommentReply, DocumentNotes, DraftVersion, InboxCommand,
    InboxCommandKind, InboxResult, LiveState, NotesStore, VersionKind,
};
pub use draft_store::{
    DraftDocument, DraftLibrary, DraftRecord, DraftSummary, RecentPath, RecoverySnapshot,
    SaveResult,
};
pub use edit_placement::{EditFailure, EditPlacement, EditPosition, PlacedEdit, ProposedEdit};
pub use error::{DraftStoreError, Error, Result};
pub use markdown::{MarkdownBlockKind, MarkdownInlineKind, MarkdownInlineSpan, MarkdownLine};
pub use markdown_document::{LineSplice, MarkdownDocument};
pub use markdown_images::{ImageSource, MarkdownImage};
pub use offsets::{
    byte_range_to_utf16, byte_to_utf16, utf16_len, utf16_range_to_bytes, utf16_to_byte,
};
pub use remote::{RemoteEntry, RemoteKind, RemoteLocation, SshTransport};
pub use text_editing::{LinePrefix, TextEdit};
pub use text_stats::TextCounts;
pub use text_units::FocusUnit;
pub use word_diff::{DiffSegment, DiffSegmentKind, FrozenScope};
