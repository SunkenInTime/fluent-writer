//! Crash-safe file writes.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::dates::new_uuid_string;
use crate::error::describe_io;
use crate::paths::{file_name, path_string};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWriteError {
    pub path: String,
    pub operation: String,
    /// The OS error code (`errno` on Unix, a Win32 error code on Windows); -1 when unknown.
    pub code: i32,
}

impl FileWriteError {
    fn new(path: &Path, operation: &str, e: &io::Error) -> Self {
        FileWriteError {
            path: path_string(path),
            operation: operation.to_string(),
            code: e.raw_os_error().unwrap_or(-1),
        }
    }

    /// The OS's description of `code` (Swift used `strerror`).
    pub fn reason(&self) -> String {
        if self.code < 0 {
            return "Unknown error".to_string();
        }
        describe_io(&io::Error::from_raw_os_error(self.code))
    }
}

impl fmt::Display for FileWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Couldn't {} “{}”: {}.",
            self.operation,
            file_name(Path::new(&self.path)),
            self.reason()
        )
    }
}

impl std::error::Error for FileWriteError {}

pub const TEMPORARY_MARKER: &str = ".fw-tmp-";

/// Writes `data` to a temporary file beside `path`, flushes it to disk, then renames it over `path`.
/// Readers see either the previous complete file or the new complete file, never a partial one.
pub fn write(data: &[u8], path: &Path) -> Result<(), FileWriteError> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => Path::new(".").to_path_buf(),
    };
    let tmp = dir.join(format!(
        ".{}{}{}",
        file_name(path),
        TEMPORARY_MARKER,
        new_uuid_string()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    let mut file = options
        .open(&tmp)
        .map_err(|e| FileWriteError::new(path, "save", &e))?;
    let written = file.write_all(data).and_then(|_| full_sync(&file));
    drop(file);
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(FileWriteError::new(path, "save", &e));
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(FileWriteError::new(path, "save", &e));
    }
    sync_directory(&dir);
    Ok(())
}

/// `F_FULLFSYNC` on Apple platforms, falling back to `fsync` where the file system lacks it.
#[cfg(target_vendor = "apple")]
fn full_sync(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let fd = file.as_raw_fd();
    // SAFETY: fd is a valid open descriptor owned by `file` for the duration of these calls.
    unsafe {
        if libc::fcntl(fd, libc::F_FULLFSYNC) != 0 && libc::fsync(fd) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(target_vendor = "apple"))]
fn full_sync(file: &File) -> io::Result<()> {
    file.sync_all()
}

/// Flushes the rename itself. Windows has no directory handles to sync, and NTFS journals renames.
#[cfg(unix)]
fn sync_directory(dir: &Path) {
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_directory(_dir: &Path) {}

/// Removes temporary files left behind by an interrupted write. The real file is untouched.
pub fn remove_stale_temporaries(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && name.contains(TEMPORARY_MARKER) {
            let p = entry.path();
            if p.is_dir() {
                let _ = fs::remove_dir_all(&p);
            } else {
                let _ = fs::remove_file(&p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_replaces_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a.md");
        write(b"one", &target).unwrap();
        write(b"two", &target).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "two");
        let stale = dir.path().join(format!(".a.md{TEMPORARY_MARKER}crash"));
        fs::write(&stale, "par").unwrap();
        remove_stale_temporaries(dir.path());
        assert!(!stale.exists());
        assert!(target.exists());
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn error_message() {
        let e = FileWriteError {
            path: "/x/My post.md".into(),
            operation: "save".into(),
            code: 13,
        };
        let s = e.to_string();
        assert!(s.starts_with("Couldn't save “My post.md”: "), "{s}");
        assert!(s.ends_with('.'));
        assert!(!s.contains("os error"));
    }
}
