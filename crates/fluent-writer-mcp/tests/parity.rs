//! The server's protocol and tool messages against `FluentWriterMCP/main.swift` and
//! `WriterCore/DocumentNotes.swift`, line by line where PARITY.md lists them.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Mcp {
    fn start(support: &std::path::Path) -> Mcp {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fluent-writer-mcp"))
            .arg("--support")
            .arg(support)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Mcp { child, stdin, stdout }
    }

    fn send_line(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn rpc(&mut self, msg: Value) -> Value {
        self.send_line(&msg.to_string());
        self.read()
    }

    fn call(&mut self, name: &str, args: Value) -> (String, bool) {
        let r = self.rpc(json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": name, "arguments": args}}));
        (r["result"]["content"][0]["text"].as_str().unwrap().to_string(), r["result"]["isError"].as_bool().unwrap())
    }

    fn init(&mut self, client: Value) -> Value {
        self.rpc(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": client, "protocolVersion": "2024-11-05"}}))
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// `FluentWriterMCP/main.swift:13-24`
#[test]
fn help_prints_usage_and_exits_0() {
    for flag in ["--help", "-h"] {
        let out = Command::new(env!("CARGO_BIN_EXE_fluent-writer-mcp")).arg(flag).output().unwrap();
        assert!(out.status.success());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "fluent-writer-mcp [--support <dir>] [--variant standard|dev]\nSpeaks the Model Context Protocol over stdio.\n");
    }
}

/// `FluentWriterMCP/main.swift:357-406`
#[test]
fn the_protocol_answers_as_the_swift_server_did() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = Mcp::start(dir.path());
    let init = m.init(json!({"name": "x", "title": "Claude Desktop"}));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05", "the client's version is echoed");
    assert_eq!(init["result"]["serverInfo"], json!({"name": "fluent-writer", "title": "Fluent Writer", "version": "0.2.0"}));
    assert_eq!(init["result"]["capabilities"], json!({"tools": {"listChanged": false}}));
    assert!(init["result"]["instructions"].as_str().unwrap().starts_with("Fluent Writer is the author's Markdown writing app."));
    // Blank and unparsable lines and notifications get no answer; the next reply is the ping's.
    m.send_line("");
    m.send_line("{not json");
    m.send_line(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string());
    assert_eq!(m.rpc(json!({"jsonrpc": "2.0", "id": 2, "method": "ping"})), json!({"jsonrpc": "2.0", "id": 2, "result": {}}));
    // A batch answers each request in it.
    m.send_line(&json!([{"jsonrpc": "2.0", "id": 3, "method": "ping"}, {"jsonrpc": "2.0", "id": 4, "method": "nope"}]).to_string());
    assert_eq!(m.read()["id"], 3);
    let err = m.read();
    assert_eq!(err["error"], json!({"code": -32601, "message": "Method not found: nope"}));
    let names: Vec<String> = m.rpc(json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list"}))["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, ["get_context", "read_document", "list_comments", "add_comment", "reply_to_comment", "resolve_comment", "propose_version", "list_versions", "read_version", "get_assist_history", "list_documents"]);
}

/// "Claude Code" for anything named claude, "Codex CLI" for codex, else the client's title or
/// name, else "Outside agent". `FluentWriterMCP/main.swift:27`, `:372-376`
#[test]
fn comments_are_signed_with_the_clients_name() {
    for (client, author) in [
        (json!({"name": "claude-code"}), "Claude Code"),
        (json!({"name": "x", "title": "OpenAI Codex"}), "Codex CLI"),
        (json!({"name": "zed", "title": "Zed Agent"}), "Zed Agent"),
        (json!({}), "Outside agent"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let draft = dir.path().join("a.md");
        std::fs::write(&draft, "Some text.").unwrap();
        let mut m = Mcp::start(&dir.path().join("support"));
        m.init(client);
        let (_, err) = m.call("add_comment", json!({"path": draft, "quote": "Some", "comment": "Hm"}));
        assert!(!err);
        let (text, _) = m.call("list_comments", json!({"path": draft}));
        let list: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(list[0]["author"], author);
    }
}

/// `FluentWriterMCP/main.swift:56-69`, `:271-348`, `WriterCore/DocumentNotes.swift:356-405`
#[test]
fn tool_messages_read_as_the_swift_wrote_them() {
    let dir = tempfile::tempdir().unwrap();
    let draft = dir.path().join("a.md");
    std::fs::write(&draft, "First line\nthe cat and the cat\n").unwrap();
    let mut m = Mcp::start(&dir.path().join("support"));
    m.init(json!({"name": "agent"}));
    let call = |m: &mut Mcp, name: &str, args: Value| m.call(name, args);

    assert_eq!(call(&mut m, "read_document", json!({})), ("Fluent Writer isn't running, so there's no open draft. Pass `path` to a Markdown file, or use list_documents.".into(), true));
    assert_eq!(call(&mut m, "read_document", json!({"path": "/tmp/x.pdf"})), ("`path` must be a draft (.md, .markdown, .txt or .text).".into(), true));
    assert_eq!(call(&mut m, "read_document", json!({"path": "/nonexistent/x.md"})), ("Couldn't read /nonexistent/x.md.".into(), true));
    assert_eq!(call(&mut m, "read_document", json!({"path": draft, "line_numbers": true})).0, "    1  First line\n    2  the cat and the cat\n    3  ");
    assert_eq!(call(&mut m, "list_comments", json!({"path": draft})).0, "No open comments on this draft.");
    assert_eq!(call(&mut m, "list_comments", json!({"path": draft, "include_resolved": true})).0, "No comments on this draft.");
    assert_eq!(call(&mut m, "add_comment", json!({"path": draft, "quote": "dog", "comment": "x"})), ("That text isn't in the draft.".into(), true));
    assert_eq!(call(&mut m, "add_comment", json!({"path": draft, "quote": "cat", "comment": "x", "occurrence": 3})), ("The text appears 2 times; occurrence must be 1–2.".into(), true));
    assert_eq!(call(&mut m, "add_comment", json!({"path": draft, "quote": "cat", "comment": ""})), ("add_comment needs the quoted text and a comment.".into(), true));
    let (added, err) = call(&mut m, "add_comment", json!({"path": draft, "quote": "cat", "comment": "Which?", "occurrence": 2.4}));
    assert!(!err && added.starts_with("Added comment "), "{added}");
    let id = added["Added comment ".len()..].split('.').next().unwrap().to_string();
    let (list, _) = call(&mut m, "list_comments", json!({"path": draft}));
    let list: Value = serde_json::from_str(&list).unwrap();
    assert_eq!((list[0]["line"].as_i64(), list[0]["status"].as_str()), (Some(2), Some("open")));
    assert!(list[0]["created"].as_str().unwrap().ends_with('Z'), "ISO 8601");
    assert_eq!(call(&mut m, "reply_to_comment", json!({"path": draft, "comment_id": "nope", "reply": "r"})), ("No comment with that id.".into(), true));
    assert_eq!(call(&mut m, "reply_to_comment", json!({"path": draft, "comment_id": id, "reply": ""})), ("The reply is empty.".into(), true));
    assert!(call(&mut m, "reply_to_comment", json!({"path": draft, "comment_id": id, "reply": "The second."})).0.starts_with(&format!("Replied to comment {id}.")));
    assert!(call(&mut m, "resolve_comment", json!({"path": draft, "comment_id": id})).0.starts_with(&format!("Resolved comment {id}.")));
    assert_eq!(call(&mut m, "propose_version", json!({"path": draft, "label": "Same", "full_text": "First line\nthe cat and the cat\n"})), ("The version is identical to the draft.".into(), true));
    assert_eq!(call(&mut m, "list_versions", json!({"path": draft})).0, "No versions yet.");
    let (made, err) = call(&mut m, "propose_version", json!({"path": draft, "edits": [{"old": "First", "new": "Opening"}]}));
    assert!(!err && made.contains("The author can preview it in Fluent Writer and choose whether to use it."), "{made}");
    let (versions, _) = call(&mut m, "list_versions", json!({"path": draft}));
    let versions: Value = serde_json::from_str(&versions).unwrap();
    assert_eq!(versions[0]["label"], "agent version", "the default label");
    assert_eq!(call(&mut m, "read_version", json!({"path": draft, "version_id": versions[0]["id"]})).0, "Opening line\nthe cat and the cat\n");
    assert_eq!(call(&mut m, "read_version", json!({"path": draft, "version_id": "x"})), ("No version with that id. Use list_versions.".into(), true));
    assert_eq!(call(&mut m, "get_assist_history", json!({"path": draft, "limit": f64::NAN})).0, "No Assist requests for this draft yet.");
    assert_eq!(call(&mut m, "list_documents", json!({"limit": -5})).0, "No recent drafts.");
}

/// With the app running (its live state names a live process) a change goes through the
/// inbox, and is reported queued when nothing answers in 4 s. `FluentWriterMCP/main.swift:116-135`
#[test]
fn a_running_app_gets_changes_through_its_inbox() {
    let dir = tempfile::tempdir().unwrap();
    let support = dir.path().join("support");
    let draft = dir.path().join("a.md");
    std::fs::write(&draft, "Text here.").unwrap();
    let store = writer_core::NotesStore::new(&support);
    let path = draft.to_string_lossy().into_owned();
    store.write_live_state(&writer_core::LiveState::new(std::process::id(), "Fluent Writer", Some(path.clone()), None, "a", "Text here.", 0..4, None));
    let mut m = Mcp::start(&support);
    m.init(json!({"name": "agent"}));
    let (ctx, _) = m.call("get_context", json!({}));
    let ctx: Value = serde_json::from_str(&ctx).unwrap();
    assert_eq!(ctx["app"], "Fluent Writer is running.");
    assert_eq!(ctx["document"]["path"], path.as_str());
    assert_eq!(ctx["selection"], json!({"text": "Text", "line": 1}));
    let started = std::time::Instant::now();
    let (text, err) = m.call("add_comment", json!({"quote": "here", "comment": "x"}));
    assert_eq!((text.as_str(), err), ("Queued. Fluent Writer hasn't picked it up yet; it will apply it shortly.", false));
    assert!(started.elapsed() >= std::time::Duration::from_secs(4));
    assert_eq!(std::fs::read_dir(support.join("Inbox")).unwrap().flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")).count(), 1, "left in the inbox for the app");
}
