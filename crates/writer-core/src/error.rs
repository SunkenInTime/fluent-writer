use std::fmt;

use crate::atomic_file::FileWriteError;

/// A failure with a message ready to show the writer (Swift `DraftStoreError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftStoreError {
    pub message: String,
}

impl DraftStoreError {
    pub fn new(message: impl Into<String>) -> Self {
        DraftStoreError {
            message: message.into(),
        }
    }
}

impl fmt::Display for DraftStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DraftStoreError {}

/// Every error this crate returns. `Display` gives the message the Swift app showed
/// (its `localizedDescription`).
#[derive(Debug)]
pub enum Error {
    /// An atomic write failed (Swift `FileWriteError`).
    FileWrite(FileWriteError),
    /// A failure described for the writer (Swift `DraftStoreError`).
    Store(DraftStoreError),
    /// Any other file system error (what Swift's `FileManager` threw).
    Io(std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::FileWrite(e) => e.fmt(f),
            Error::Store(e) => e.fmt(f),
            Error::Io(e) => f.write_str(&describe_io(e)),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::FileWrite(e) => Some(e),
            Error::Store(e) => Some(e),
            Error::Io(e) => Some(e),
        }
    }
}

impl From<FileWriteError> for Error {
    fn from(e: FileWriteError) -> Self {
        Error::FileWrite(e)
    }
}

impl From<DraftStoreError> for Error {
    fn from(e: DraftStoreError) -> Self {
        Error::Store(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// An io error's reason without Rust's " (os error N)" suffix.
pub(crate) fn describe_io(e: &std::io::Error) -> String {
    let s = e.to_string();
    match s.rfind(" (os error ") {
        Some(i) if s.ends_with(')') => s[..i].to_string(),
        _ => s,
    }
}
