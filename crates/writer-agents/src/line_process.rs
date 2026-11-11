//! A child process speaking newline-delimited text over stdio, and the lookup that finds the
//! command-line tools it runs (Swift `LineProcess.swift`).
//!
//! Swift delivered callbacks on the main queue. Here output is read on background threads and
//! handed to a [`ProcessSink`]; [`crate::AgentHandle`] funnels it into the one thread that drives
//! the session, which gives the same one-at-a-time ordering.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use crate::agent_models::ProcessSink;

/// Something the child process did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessEvent {
    /// One non-empty line of stdout, without its `\n`.
    Line(String),
    /// The process ended. `status` is the exit code, or the signal number when a signal killed
    /// it (Swift's `terminationStatus`); `stderr` is the last 4000 characters it wrote there.
    Exit { status: i32, stderr: String },
}

/// Environment for a child process.
pub type Environment = HashMap<OsString, OsString>;

/// A child process speaking lines over stdio. Cloning gives another handle to the same process.
#[derive(Clone)]
pub struct LineProcess {
    inner: Arc<Inner>,
}

struct Inner {
    executable: PathBuf,
    arguments: Vec<OsString>,
    working_directory: Option<PathBuf>,
    environment: Option<Environment>,
    closed: AtomicBool,
    exited: AtomicBool,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    #[cfg(windows)]
    job: Mutex<Option<job::Job>>,
}

const STDERR_TAIL: usize = 4000;

impl LineProcess {
    pub fn new(
        executable: impl Into<PathBuf>,
        arguments: impl IntoIterator<Item = impl Into<OsString>>,
        working_directory: Option<PathBuf>,
        environment: Option<Environment>,
    ) -> Self {
        LineProcess {
            inner: Arc::new(Inner {
                executable: executable.into(),
                arguments: arguments.into_iter().map(Into::into).collect(),
                working_directory,
                environment,
                closed: AtomicBool::new(false),
                exited: AtomicBool::new(false),
                child: Mutex::new(None),
                stdin: Mutex::new(None),
                #[cfg(windows)]
                job: Mutex::new(None),
            }),
        }
    }

    /// Starts the process. Lines and the exit go to `sink` until [`LineProcess::terminate`].
    pub fn run(&self, sink: ProcessSink) -> std::io::Result<()> {
        let inner = &self.inner;
        let mut cmd = Command::new(&inner.executable);
        cmd.args(&inner.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &inner.working_directory {
            cmd.current_dir(dir);
        }
        if let Some(env) = &inner.environment {
            cmd.env_clear().envs(env);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console window flashes up for codex.cmd or node.exe.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn()?;
        #[cfg(windows)]
        {
            *lock(&inner.job) = job::Job::for_child(&child);
        }

        let mut stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        *lock(&inner.child) = Some(child);

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        *lock(&inner.stdin) = Some(tx);
        thread::spawn(move || {
            for data in rx {
                if stdin.write_all(&data).and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
        });

        let (done_tx, done_rx) = mpsc::channel::<()>();
        let reader_inner = self.inner.clone();
        let reader_sink = sink.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if buf.last() == Some(&b'\n') {
                    buf.pop();
                }
                if reader_inner.closed.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(line) = std::str::from_utf8(&buf)
                    && !line.is_empty()
                {
                    reader_sink(ProcessEvent::Line(line.to_string()));
                }
            }
            let _ = done_tx.send(());
        });

        let tail = Arc::new(Mutex::new(Vec::<u8>::new()));
        let tail_writer = tail.clone();
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut t = lock(&tail_writer);
                t.extend_from_slice(&chunk[..n]);
                // Keep enough bytes for 4000 characters of any width.
                let keep = STDERR_TAIL * 16;
                if t.len() > keep {
                    let cut = t.len() - keep;
                    t.drain(..cut);
                }
            }
        });

        let waiter = self.inner.clone();
        thread::spawn(move || {
            let status = loop {
                let polled = match lock(&waiter.child).as_mut() {
                    Some(c) => c.try_wait(),
                    None => Ok(None),
                };
                match polled {
                    Ok(Some(s)) => break Some(s),
                    Ok(None) => thread::sleep(Duration::from_millis(25)),
                    Err(_) => break None,
                }
            };
            waiter.exited.store(true, Ordering::SeqCst);
            // Let the last lines through first. A grandchild holding stdout open can't stall us.
            let _ = done_rx.recv_timeout(Duration::from_millis(500));
            if waiter.closed.load(Ordering::SeqCst) {
                return;
            }
            let bytes = lock(&tail).clone();
            let text = String::from_utf8_lossy(&bytes);
            let stderr = crate::swift_text::suffix(&text, STDERR_TAIL).to_string();
            sink(ProcessEvent::Exit {
                status: status.map_or(-1, exit_code),
                stderr,
            });
        });
        Ok(())
    }

    /// Writes `line` and a newline to stdin. Ignored once the process has stopped.
    pub fn send(&self, line: &str) {
        if self.inner.closed.load(Ordering::SeqCst) || !self.is_running() {
            return;
        }
        if let Some(tx) = lock(&self.inner.stdin).as_ref() {
            let _ = tx.send(format!("{line}\n").into_bytes());
        }
    }

    pub fn is_running(&self) -> bool {
        lock(&self.inner.child).is_some() && !self.inner.exited.load(Ordering::SeqCst)
    }

    /// Detaches the sink and stops the process; nothing is delivered afterwards. Closes stdin and
    /// asks the process to stop (SIGTERM on Unix), then kills it if it's still running 2 s later.
    /// On Windows the kill takes the whole process tree, since `codex.cmd` and `node` start children.
    pub fn terminate(&self) {
        if self.inner.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        lock(&self.inner.stdin).take();
        #[cfg(unix)]
        if let Some(c) = lock(&self.inner.child).as_mut()
            && matches!(c.try_wait(), Ok(None))
            && let Ok(pid) = libc::pid_t::try_from(c.id())
        {
            // SAFETY: the child hasn't been reaped (we hold its lock), so the pid is still ours.
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
        let inner = self.inner.clone();
        thread::spawn(move || {
            for _ in 0..80 {
                if inner.exited.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
            #[cfg(windows)]
            if let Some(j) = lock(&inner.job).as_ref() {
                j.terminate();
            }
            if let Some(c) = lock(&inner.child).as_mut()
                && matches!(c.try_wait(), Ok(None))
            {
                let _ = c.kill();
            }
        });
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn exit_code(s: ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = s.signal() {
            return sig;
        }
    }
    s.code().unwrap_or(-1)
}

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    /// A job object holding the child and everything it starts, killed together.
    pub struct Job(HANDLE);

    // SAFETY: a job handle may be used from any thread.
    unsafe impl Send for Job {}

    impl Job {
        pub fn for_child(child: &Child) -> Option<Job> {
            // SAFETY: plain Win32 calls; the handle is closed on failure and on drop.
            unsafe {
                let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if h.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if AssignProcessToJobObject(h, child.as_raw_handle() as HANDLE) == 0 {
                    CloseHandle(h);
                    return None;
                }
                Some(Job(h))
            }
        }

        pub fn terminate(&self) {
            // SAFETY: the handle is valid until drop.
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: closing our own handle once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// Finds command-line tools the way a login shell would, since GUI apps start with a bare PATH.
pub struct ExecutableLocator;

impl ExecutableLocator {
    /// Searches `extra_directories`, then `PATH`, then the usual install folders, then asks a login
    /// shell. On Windows each folder is tried with the `PATHEXT` extensions (`.exe`, `.cmd`, …).
    pub fn find(name: &str, extra_directories: &[PathBuf]) -> Option<PathBuf> {
        let mut dirs: Vec<PathBuf> = extra_directories.to_vec();
        if let Some(path) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&path).filter(|d| !d.as_os_str().is_empty()));
        }
        dirs.extend(Self::common_directories());
        for dir in dirs {
            for candidate in candidates(&dir, name) {
                if is_executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
        Self::login_shell_lookup(name)
    }

    /// The install folders searched after `PATH`.
    pub fn common_directories() -> Vec<PathBuf> {
        let home = dirs::home_dir().unwrap_or_default();
        let mut out: Vec<PathBuf> = Vec::new();
        #[cfg(not(windows))]
        out.extend(
            ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
                .into_iter()
                .map(PathBuf::from),
        );
        out.extend(
            [
                ".local/bin",
                ".npm-global/bin",
                ".bun/bin",
                ".volta/bin",
                ".claude/local",
                ".codex/bin",
            ]
            .into_iter()
            .map(|d| home.join(d)),
        );
        #[cfg(target_os = "linux")]
        out.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
        #[cfg(windows)]
        {
            let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
            out.extend(env("APPDATA").map(|d| d.join("npm")));
            out.extend(env("LOCALAPPDATA").map(|d| d.join("Volta").join("bin")));
            out.extend(env("NVM_SYMLINK"));
            out.extend(env("ProgramFiles").map(|d| d.join("nodejs")));
        }
        out
    }

    /// `command -v <name>` in a login shell: `/bin/zsh` on macOS, `$SHELL` (or `/bin/sh`) on
    /// Linux. Not used on Windows, where GUI apps inherit the user's PATH.
    pub fn login_shell_lookup(name: &str) -> Option<PathBuf> {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return None;
        }
        #[cfg(windows)]
        {
            None
        }
        #[cfg(not(windows))]
        {
            #[cfg(target_os = "macos")]
            let shell = OsString::from("/bin/zsh");
            #[cfg(not(target_os = "macos"))]
            let shell = std::env::var_os("SHELL")
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "/bin/sh".into());
            let out = Command::new(shell)
                .args(["-lc", &format!("command -v {name}")])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()?;
            let s = String::from_utf8_lossy(&out.stdout);
            let s = crate::swift_text::trim(&s);
            (out.status.success() && s.starts_with('/') && is_executable_file(Path::new(s)))
                .then(|| PathBuf::from(s))
        }
    }

    /// The inherited environment with `dirs` and the usual tool folders put at the front of PATH.
    pub fn environment(adding: &[PathBuf]) -> Environment {
        let mut env: Environment = std::env::vars_os().collect();
        let base = take_path(&mut env);
        let mut parts: Vec<OsString> = adding.iter().map(|d| d.as_os_str().to_owned()).collect();
        #[cfg(not(windows))]
        {
            parts.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(OsString::from));
            parts.push(base.unwrap_or_else(|| "/usr/bin:/bin:/usr/sbin:/sbin".into()));
        }
        #[cfg(windows)]
        parts.extend(base);
        let separator = if cfg!(windows) { ";" } else { ":" };
        env.insert("PATH".into(), parts.join(OsStr::new(separator)));
        env
    }

    /// An empty scratch directory so agents start with nothing on disk to see.
    pub fn scratch_directory() -> PathBuf {
        let id = uuid::Uuid::new_v4().hyphenated().to_string().to_uppercase();
        let dir = std::env::temp_dir().join(format!("fluent-writer-agent-{id}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}

/// Removes PATH from `env` (any capitalisation on Windows, where `Path` is usual) and returns it.
fn take_path(env: &mut Environment) -> Option<OsString> {
    let key = env
        .keys()
        .find(|k| {
            if cfg!(windows) {
                k.to_string_lossy().eq_ignore_ascii_case("PATH")
            } else {
                k.as_os_str() == "PATH"
            }
        })?
        .clone();
    env.remove(&key)
}

fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    if cfg!(windows) && Path::new(name).extension().is_none() {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        pathext
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| dir.join(format!("{name}{}", e.to_lowercase())))
            .collect()
    } else {
        vec![dir.join(name)]
    }
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `c` is a valid NUL-terminated path.
    let executable = unsafe { libc::access(c.as_ptr(), libc::X_OK) == 0 };
    executable && path.is_file()
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}
