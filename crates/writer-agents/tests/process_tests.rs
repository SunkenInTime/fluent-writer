//! Real child processes and the worker thread: not in the Swift tests, which never spawned anything.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use writer_agents::{
    AgentEvent, AgentHandle, ClaudeAgent, CodexAgent, FixtureAgent, LineProcess, ProcessEvent,
};

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// Collects events until a terminal one (or 10 s).
fn drain(h: &AgentHandle) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(e) = h.recv_timeout(Duration::from_millis(50)) {
            let done = e.is_terminal();
            out.push(e);
            if done {
                break;
            }
        }
    }
    out
}

#[test]
fn line_process_delivers_lines_then_exit_with_stderr() {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let p = LineProcess::new(
        "/bin/sh",
        [
            "-c",
            "read x; echo got:$x; echo; printf 'one\\ntwo\\n'; echo boom >&2; exit 3",
        ],
        None,
        None,
    );
    p.run(Arc::new(move |e| tx.lock().unwrap().send(e).unwrap()))
        .unwrap();
    p.send("hi");
    let got: Vec<ProcessEvent> = (0..4)
        .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    assert_eq!(
        got,
        vec![
            ProcessEvent::Line("got:hi".into()),
            ProcessEvent::Line("one".into()),
            ProcessEvent::Line("two".into()),
            ProcessEvent::Exit {
                status: 3,
                stderr: "boom\n".into()
            },
        ]
    );
}

#[test]
fn terminate_stops_delivery_and_kills() {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let p = LineProcess::new(
        "/bin/sh",
        [
            "-c",
            "trap '' TERM; while :; do echo tick; sleep 0.05; done",
        ],
        None,
        None,
    );
    p.run(Arc::new(move |e| {
        let _ = tx.lock().unwrap().send(e);
    }))
    .unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(5)),
        Ok(ProcessEvent::Line(_))
    ));
    p.terminate();
    while rx.try_recv().is_ok() {}
    // SIGTERM is ignored, so the SIGKILL 2 s later has to end it.
    let deadline = Instant::now() + Duration::from_secs(5);
    while p.is_running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!p.is_running());
    assert!(rx.try_recv().is_err());
}

const FAKE_CODEX: &str = r#"[ "$1" = app-server ] || exit 9
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/^{"id":\([0-9]*\),.*/\1/p')
  case "$line" in
    *'"method":"initialize"'*) echo "{\"id\":$id,\"result\":{}}" ;;
    *'"method":"thread/start"'*) echo "{\"id\":$id,\"result\":{\"thread\":{\"id\":\"t1\"}}}" ;;
    *'"method":"turn/start"'*)
      echo "{\"id\":$id,\"result\":{\"turn\":{\"id\":\"u1\"}}}"
      echo '{"id":7,"method":"item/commandExecution/requestApproval","params":{}}'
      echo '{"method":"item/agentMessage/delta","params":{"delta":"<feedback>Hi"}}'
      echo '{"method":"item/agentMessage/delta","params":{"delta":"</feedback>"}}'
      echo "cwd=$(pwd)" >&2
      ;;
    *'"result":{"decision":"decline"}'*)
      echo '{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}' ;;
  esac
done"#;

#[test]
fn codex_over_a_real_process() {
    let dir = tempfile::tempdir().unwrap();
    let codex = script(dir.path(), "codex", FAKE_CODEX);
    let wakes = Arc::new(AtomicUsize::new(0));
    let w = wakes.clone();
    let h = AgentHandle::start(
        Box::new(CodexAgent::new(Some(codex))),
        "PROMPT".into(),
        move || {
            w.fetch_add(1, Ordering::SeqCst);
        },
    );
    let events = drain(&h);
    assert_eq!(
        events,
        vec![
            AgentEvent::Progress("<feedback>Hi".into()),
            AgentEvent::Progress("<feedback>Hi</feedback>".into()),
            AgentEvent::Completed("<feedback>Hi</feedback>".into()),
        ]
    );
    assert_eq!(wakes.load(Ordering::SeqCst), 3);
}

#[test]
fn codex_crash_reports_last_stderr_line() {
    let dir = tempfile::tempdir().unwrap();
    let codex = script(
        dir.path(),
        "codex",
        "read x; echo 'first' >&2; echo 'Error: config is broken' >&2; exit 1",
    );
    let h = AgentHandle::start(Box::new(CodexAgent::new(Some(codex))), "p".into(), || {});
    assert_eq!(
        drain(&h),
        vec![AgentEvent::Failed(
            "Codex stopped unexpectedly: Error: config is broken".into()
        )]
    );
    let dir = tempfile::tempdir().unwrap();
    let codex = script(dir.path(), "codex", "exit 4");
    let h = AgentHandle::start(Box::new(CodexAgent::new(Some(codex))), "p".into(), || {});
    assert_eq!(
        drain(&h),
        vec![AgentEvent::Failed(
            "Codex stopped unexpectedly: exit 4".into()
        )]
    );
}

#[test]
fn claude_bridge_protocol_over_a_fake_node() {
    let dir = tempfile::tempdir().unwrap();
    // Stands in for `node bridge.mjs`: checks the request line, then streams.
    let node = script(
        dir.path(),
        "node",
        r#"read line
case "$line" in *'"cwd":"'*'"prompt":"P","type":"request"}') ;; *) echo '{"type":"error","message":"bad request"}'; exit 0 ;; esac
echo '{"type":"delta","text":"<feedback>"}'
echo '{"type":"delta","text":"Fine."}'
echo '{"type":"done","text":"<feedback>Fine.</feedback>"}'"#,
    );
    let bridge = dir.path().join("bridge.mjs");
    std::fs::write(&bridge, "").unwrap();
    let h = AgentHandle::start(
        Box::new(ClaudeAgent::new(Some(node), Some(bridge))),
        "P".into(),
        || {},
    );
    assert_eq!(
        drain(&h),
        vec![
            AgentEvent::Progress("<feedback>".into()),
            AgentEvent::Progress("<feedback>Fine.".into()),
            AgentEvent::Completed("<feedback>Fine.</feedback>".into()),
        ]
    );
}

#[test]
fn fixture_replays_word_by_word_and_stop_silences() {
    let h = AgentHandle::start(
        Box::new(FixtureAgent::new("one two three")),
        String::new(),
        || {},
    );
    assert_eq!(
        drain(&h),
        vec![
            AgentEvent::Progress("one".into()),
            AgentEvent::Progress("one two".into()),
            AgentEvent::Completed("one two three".into()),
        ]
    );
    let h = AgentHandle::start(
        Box::new(FixtureAgent::new("a b c d e f g h")),
        String::new(),
        || {},
    );
    assert!(matches!(
        h.recv_timeout(Duration::from_secs(2)),
        Some(AgentEvent::Progress(_))
    ));
    h.stop();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(h.try_recv(), None);
}

#[test]
fn stop_kills_a_hanging_agent() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    let codex = script(
        dir.path(),
        "codex",
        &format!(
            "echo $$ > '{}'; while IFS= read -r l; do :; done; sleep 30",
            pidfile.display()
        ),
    );
    let h = AgentHandle::start(Box::new(CodexAgent::new(Some(codex))), "p".into(), || {});
    let deadline = Instant::now() + Duration::from_secs(5);
    while !pidfile.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid: i32 = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    h.stop();
    let deadline = Instant::now() + Duration::from_secs(5);
    // SAFETY: signal 0 only checks the pid.
    while unsafe { libc_kill(pid, 0) } == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_ne!(unsafe { libc_kill(pid, 0) }, 0, "agent process still alive");
    assert_eq!(h.try_recv(), None);
}

unsafe extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

/// Needs a signed-in `codex`; run with `--ignored`. Stops a real request mid-stream and checks
/// that no `codex app-server` is left running.
#[test]
#[ignore]
fn real_codex_stop_leaves_no_process() {
    use writer_agents::{AgentAction, AgentProvider, AgentRequest};
    let count = || {
        std::process::Command::new("pgrep")
            .args(["-fc", "codex app-server"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<u32>()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    };
    let before = count();
    let req = AgentRequest::new(
        AgentProvider::Codex,
        AgentAction::Custom,
        "Write a 600-word story about the sea.",
        "Draft.",
        "",
    );
    let h = AgentHandle::start_request(&req, || {});
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match h.recv_timeout(Duration::from_millis(100)) {
            Some(AgentEvent::Progress(_)) => break,
            Some(other) => panic!("ended before streaming: {other:?}"),
            None if Instant::now() > deadline => panic!("no progress"),
            None => {}
        }
    }
    assert!(count() > before);
    h.stop();
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(h.try_recv(), None);
    assert_eq!(count(), before, "codex app-server still running after stop");
}
