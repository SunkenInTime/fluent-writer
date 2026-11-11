//! The few things that differ by operating system outside the UI framework.

use std::path::Path;
use std::process::Command;

/// The File menu's name for showing the draft in the system's file manager.
pub const REVEAL_TITLE: &str = if cfg!(target_os = "macos") {
    "Show in Finder"
} else if cfg!(target_os = "windows") {
    "Show in File Explorer"
} else {
    "Show in Files"
};

/// Shows `path` selected in the system's file manager.
pub fn reveal(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // The file managers' own interface selects the file; without it, open the folder.
        let uri = format!("file://{}", path.display());
        let shown = Command::new("dbus-send")
            .args(["--session", "--dest=org.freedesktop.FileManager1", "--type=method_call", "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1.ShowItems"])
            .arg(format!("array:string:{uri}"))
            .arg("string:")
            .status()
            .is_ok_and(|s| s.success());
        if !shown && let Some(dir) = path.parent() {
            let _ = Command::new("xdg-open").arg(dir).spawn();
        }
    }
}

/// `n` with thousands separators: 5,902.
pub fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
