//! Path helpers standing in for Foundation's `URL` path handling.

use std::path::{Component, Path, PathBuf};

/// `URL(fileURLWithPath:).standardizedFileURL`: absolute, with `.` and `..` removed lexically.
/// Symlinks are not resolved.
pub(crate) fn standardize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|d| d.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    normalize(&absolute)
}

/// Removes `.` and resolves `..` lexically. `..` at the root stays at the root.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut normals = 0usize;
    for c in path.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => out.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normals > 0 {
                    out.pop();
                    normals -= 1;
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            Component::Normal(n) => {
                out.push(n);
                normals += 1;
            }
        }
    }
    out
}

/// `URL.path`: the path as a string.
pub(crate) fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `deletingPathExtension().lastPathComponent`.
pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `lastPathComponent`.
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path_string(path))
}

/// `pathExtension`.
pub(crate) fn extension(path: &Path) -> String {
    path.extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `resolvingSymlinksInPath()` for a path that may not exist: resolves the longest existing
/// ancestor and keeps the rest as written.
pub(crate) fn resolve_symlinks(path: &Path) -> PathBuf {
    let path = standardize(path);
    let mut existing = path.clone();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&existing) {
            let mut out = real;
            for c in rest.iter().rev() {
                out.push(c);
            }
            return out;
        }
        match (
            existing.file_name().map(|n| n.to_os_string()),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return path,
        }
    }
}
