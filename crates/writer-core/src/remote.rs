//! Files and folders on another machine, reached with the system `ssh` and the user's own SSH
//! config and keys (Swift `RemoteLocation` and `SSHTransport`).

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::charsets::{
    is_character_newline, is_whitespace, is_whitespace_or_newline, split_newlines, trim,
};
use crate::error::{DraftStoreError, describe_io};
use crate::markdown_images::remove_percent_encoding;

/// A file or folder on another machine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemoteLocation {
    /// `host`, `user@host`, or an alias from `~/.ssh/config`.
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Absolute, or relative to the remote home when it starts with `~`.
    pub path: String,
}

impl RemoteLocation {
    pub fn new(host: impl Into<String>, port: Option<u16>, path: impl Into<String>) -> Self {
        RemoteLocation {
            host: host.into(),
            port,
            path: path.into(),
        }
    }

    /// Accepts `host:path`, `user@host:~/notes.md`, and `ssh://user@host:2222/path`.
    pub fn parse(input: &str) -> Option<RemoteLocation> {
        let text = trim(input, is_whitespace_or_newline);
        if text.to_lowercase().starts_with("ssh://") {
            let (user, host, port, mut path) = parse_ssh_url(&text[6..])?;
            if host.is_empty() {
                return None;
            }
            let user = user.map(|u| format!("{u}@")).unwrap_or_default();
            if path.starts_with("/~") {
                path.remove(0);
            }
            if path.is_empty() {
                path = "~".into();
            }
            return validated(RemoteLocation {
                host: user + &host,
                port,
                path,
            });
        }
        let colon = text.find(':')?;
        let host = text[..colon].to_string();
        let mut path = text[colon + 1..].to_string();
        if path.is_empty() {
            path = "~".into();
        }
        validated(RemoteLocation {
            host,
            port: None,
            path,
        })
    }

    pub fn display(&self) -> String {
        if let Some(port) = self.port {
            let p = self.path.strip_prefix('/').unwrap_or(&self.path);
            return format!("ssh://{}:{}/{}", self.host, port, p);
        }
        format!("{}:{}", self.host, self.path)
    }

    /// The host without the user, for compact labels.
    pub fn host_name(&self) -> &str {
        self.host
            .split('@')
            .rfind(|s| !s.is_empty())
            .unwrap_or(&self.host)
    }

    /// The last path component (NSString `lastPathComponent`).
    pub fn name(&self) -> String {
        let trimmed = if self.path.ends_with('/') && self.path.chars().count() > 1 {
            &self.path[..self.path.len() - 1]
        } else {
            &self.path
        };
        last_path_component(trimmed)
    }

    pub fn appending(&self, component: &str) -> RemoteLocation {
        let mut copy = self.clone();
        copy.path = if self.path.ends_with('/') {
            format!("{}{}", self.path, component)
        } else {
            format!("{}/{}", self.path, component)
        };
        copy
    }

    pub(crate) fn appending_suffix(&self, suffix: &str) -> RemoteLocation {
        let mut copy = self.clone();
        copy.path.push_str(suffix);
        copy
    }

    /// The path as a remote shell word. A leading `~` stays unquoted so the remote shell expands it.
    pub fn shell_path(&self) -> String {
        if self.path == "~" {
            return "~".into();
        }
        if let Some(rest) = self.path.strip_prefix("~/") {
            return format!("~/{}", shell_quote(rest));
        }
        shell_quote(&self.path)
    }
}

/// Quotes `s` as one POSIX shell word.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// NSString `lastPathComponent`: trailing slashes ignored, `/` stays `/`.
fn last_path_component(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let stripped = path.trim_end_matches('/');
    if stripped.is_empty() {
        return "/".into();
    }
    stripped.rsplit('/').next().unwrap_or(stripped).to_string()
}

/// The authority and path of an `ssh://` URL, as `URLComponents` reports them (percent-decoded).
#[allow(clippy::type_complexity)]
fn parse_ssh_url(rest: &str) -> Option<(Option<String>, String, Option<u16>, String)> {
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    let after = &rest[auth_end..];
    let path_raw = after.split(['?', '#']).next().unwrap_or("");
    let (userinfo, hostport) = match authority.rfind('@') {
        Some(i) => (Some(&authority[..i]), &authority[i + 1..]),
        None => (None, authority),
    };
    let user = match userinfo {
        Some(u) => Some(remove_percent_encoding(u.split(':').next().unwrap_or(""))?),
        None => None,
    };
    let (host_raw, port_raw) = if hostport.starts_with('[') {
        let close = hostport.find(']')?;
        let tail = &hostport[close + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p),
            None if tail.is_empty() => None,
            None => return None,
        };
        (&hostport[..close + 1], port)
    } else {
        match hostport.rfind(':') {
            Some(i) => (&hostport[..i], Some(&hostport[i + 1..])),
            None => (hostport, None),
        }
    };
    let port = match port_raw {
        None | Some("") => None,
        Some(p) => {
            if !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some(p.parse::<u16>().ok()?)
        }
    };
    let host = remove_percent_encoding(host_raw)?;
    let path = remove_percent_encoding(path_raw)?;
    Some((user, host, port, path))
}

fn validated(loc: RemoteLocation) -> Option<RemoteLocation> {
    let bad = |c: char| is_whitespace_or_newline(c) || "/'\"`$;|&<>()\\".contains(c);
    if loc.host.is_empty() || loc.host.starts_with('-') || loc.host.chars().any(bad) {
        return None;
    }
    if loc.path.contains('\n') || loc.path.contains('\0') {
        return None;
    }
    Some(loc)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RemoteEntry {
    pub name: String,
    pub is_directory: bool,
}

impl RemoteEntry {
    pub fn new(name: impl Into<String>, is_directory: bool) -> Self {
        RemoteEntry {
            name: name.into(),
            is_directory,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RemoteKind {
    File,
    Directory,
    Missing,
}

/// Runs `ssh` non-interactively. Login must work without a prompt (keys, agent, or an existing
/// control master). Every call blocks until `ssh` exits; run them off the UI thread.
///
/// The remote side must have a POSIX shell (`cat`, `mv`, `ls`, `[`); the local side can be
/// macOS, Linux or Windows (Windows ships OpenSSH as `ssh.exe`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshTransport {
    pub executable: String,
    pub connect_timeout: u32,
}

impl Default for SshTransport {
    fn default() -> Self {
        SshTransport {
            executable: default_ssh_executable(),
            connect_timeout: 10,
        }
    }
}

/// `/usr/bin/ssh` on macOS and Linux. On Windows, the OpenSSH client in System32 when present,
/// else `ssh.exe` from `PATH`.
pub fn default_ssh_executable() -> String {
    #[cfg(windows)]
    {
        if let Some(root) = std::env::var_os("SystemRoot") {
            let p = std::path::Path::new(&root)
                .join("System32")
                .join("OpenSSH")
                .join("ssh.exe");
            if p.exists() {
                return p.to_string_lossy().into_owned();
            }
        }
        "ssh.exe".to_string()
    }
    #[cfg(not(windows))]
    {
        "/usr/bin/ssh".to_string()
    }
}

impl SshTransport {
    pub fn new(executable: impl Into<String>, connect_timeout: u32) -> Self {
        SshTransport {
            executable: executable.into(),
            connect_timeout,
        }
    }

    /// The `ssh` argument list. Each element is passed to the process as one argument (no local
    /// shell is involved on any platform); `command` is the single remote shell command.
    pub fn arguments(&self, loc: &RemoteLocation, command: &str) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            format!("ConnectTimeout={}", self.connect_timeout),
            "-o".into(),
            "ServerAliveInterval=5".into(),
            "-o".into(),
            "ServerAliveCountMax=2".into(),
        ];
        if let Some(port) = loc.port {
            args.push("-p".into());
            args.push(port.to_string());
        }
        args.push(loc.host.clone());
        args.push(command.to_string());
        args
    }

    pub fn read(&self, loc: &RemoteLocation) -> Result<Vec<u8>, DraftStoreError> {
        self.run(loc, &format!("cat -- {}", loc.shell_path()), None)
    }

    /// Writes to a temporary file beside the target, then renames it over the target.
    pub fn write(&self, data: &[u8], loc: &RemoteLocation) -> Result<(), DraftStoreError> {
        let id = crate::dates::new_uuid_string();
        let tmp = loc
            .appending_suffix(&format!(".fwtmp-{}", &id[..8]))
            .shell_path();
        let target = loc.shell_path();
        self.run(
            loc,
            &format!("cat > {tmp} && mv -f -- {tmp} {target} || {{ rm -f -- {tmp}; exit 1; }}"),
            Some(data),
        )?;
        Ok(())
    }

    pub fn kind(&self, loc: &RemoteLocation) -> Result<RemoteKind, DraftStoreError> {
        let p = loc.shell_path();
        let out = self.run(
            loc,
            &format!("if [ -d {p} ]; then echo d; elif [ -f {p} ]; then echo f; else echo n; fi"),
            None,
        )?;
        Ok(
            match trim(&String::from_utf8_lossy(&out), is_whitespace_or_newline) {
                "d" => RemoteKind::Directory,
                "f" => RemoteKind::File,
                _ => RemoteKind::Missing,
            },
        )
    }

    pub fn list(&self, loc: &RemoteLocation) -> Result<Vec<RemoteEntry>, DraftStoreError> {
        let out = self.run(loc, &format!("cd -- {} && ls -1Ap", loc.shell_path()), None)?;
        Ok(Self::parse_listing(&String::from_utf8_lossy(&out)))
    }

    /// Parses `ls -1Ap`: one name per line, directories marked with a trailing `/`, hidden names skipped.
    pub fn parse_listing(output: &str) -> Vec<RemoteEntry> {
        split_newlines(output)
            .filter_map(|line| {
                if line.starts_with('.') {
                    return None;
                }
                let is_dir = line.ends_with('/');
                let name = if is_dir {
                    &line[..line.len() - 1]
                } else {
                    line
                };
                if name.is_empty() {
                    return None;
                }
                Some(RemoteEntry {
                    name: name.to_string(),
                    is_directory: is_dir,
                })
            })
            .collect()
    }

    /// Runs one remote command, feeding `input` to its stdin. Swift drained stderr and fed stdin on
    /// background queues; here two threads do the same while this one reads stdout.
    fn run(
        &self,
        loc: &RemoteLocation,
        command: &str,
        input: Option<&[u8]>,
    ) -> Result<Vec<u8>, DraftStoreError> {
        let mut cmd = Command::new(&self.executable);
        cmd.args(self.arguments(loc, command))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: a GUI app must not flash a console window for every ssh call.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().map_err(|e| {
            DraftStoreError::new(format!("Couldn't start ssh: {}", describe_io(&e)))
        })?;
        let mut stdin = child.stdin.take();
        let mut stdout = child.stdout.take().expect("piped");
        let mut stderr = child.stderr.take().expect("piped");
        let input = input.map(|d| d.to_vec());
        let (data, err) = std::thread::scope(|s| {
            let err = s.spawn(move || {
                let mut buf = Vec::new();
                let _ = stderr.read_to_end(&mut buf);
                buf
            });
            s.spawn(move || {
                if let (Some(input), Some(stdin)) = (input.as_ref(), stdin.as_mut()) {
                    let _ = stdin.write_all(input);
                }
                drop(stdin);
            });
            let mut data = Vec::new();
            let _ = stdout.read_to_end(&mut data);
            (data, err.join().unwrap_or_default())
        });
        let status = child.wait().map_err(|e| {
            DraftStoreError::new(format!("Couldn't start ssh: {}", describe_io(&e)))
        })?;
        if !status.success() {
            let code = status.code().unwrap_or_else(|| signal_of(&status));
            return Err(DraftStoreError::new(describe_failure(
                &String::from_utf8_lossy(&err),
                loc.host_name(),
                code,
            )));
        }
        Ok(data)
    }
}

#[cfg(unix)]
fn signal_of(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.signal().unwrap_or(-1)
}

#[cfg(not(unix))]
fn signal_of(_status: &std::process::ExitStatus) -> i32 {
    -1
}

/// Turns ssh's stderr into a message for the writer.
pub fn describe_failure(stderr: &str, host: &str, status: i32) -> String {
    let lines: Vec<&str> = stderr
        .split(is_character_newline)
        .filter(|l| !l.is_empty() && !l.starts_with("Warning: Permanently added"))
        .collect();
    let detail = lines.last().map(|l| trim(l, is_whitespace)).unwrap_or("");
    if detail.contains("Permission denied") {
        return format!(
            "{host} refused the login. Fluent Writer can't type passwords, so use an SSH key or ssh-agent (ssh-copy-id {host})."
        );
    }
    if detail.contains("Host key verification failed") {
        return format!(
            "{host} isn't a known host yet. Run `ssh {host}` in Terminal once to trust it."
        );
    }
    if detail.is_empty() {
        return format!("ssh to {host} failed (exit {status}).");
    }
    format!("{host}: {detail}")
}
