//! Drives the built server over stdio with the app closed.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

#[test]
fn speaks_mcp_and_writes_notes() {
    let dir = tempfile::tempdir().unwrap();
    let support = dir.path().join("support");
    let draft = dir.path().join("a.md");
    std::fs::write(&draft, "One line.\nTwo line.\n").unwrap();
    let draft = draft.to_string_lossy().into_owned();

    let mut child = Command::new(env!("CARGO_BIN_EXE_fluent-writer-mcp"))
        .arg("--support")
        .arg(&support)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut rpc = |msg: Value| -> Value {
        writeln!(stdin, "{msg}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let call = |rpc: &mut dyn FnMut(Value) -> Value, name: &str, args: Value| -> (String, bool) {
        let r = rpc(
            json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": name, "arguments": args}}),
        );
        (
            r["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .to_string(),
            r["result"]["isError"].as_bool().unwrap(),
        )
    };

    let init = rpc(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"clientInfo": {"name": "My Agent"}}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["version"], "0.2.0");
    let list = rpc(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 11);
    let missing = rpc(json!({"jsonrpc": "2.0", "id": 3, "method": "nope"}));
    assert_eq!(missing["error"]["code"], -32601);

    let (text, err) = call(
        &mut rpc,
        "add_comment",
        json!({"path": draft, "quote": "Two", "comment": "Three?"}),
    );
    assert!(!err, "{text}");
    assert!(
        text.starts_with("Added comment ")
            && text.ends_with(
                "(Fluent Writer is closed; the author will see this next time they open the draft.)"
            )
    );
    let (text, _) = call(&mut rpc, "list_comments", json!({"path": draft}));
    let comments: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(comments[0]["author"], "My Agent");
    assert_eq!(comments[0]["line"], 2);
    let (text, err) = call(
        &mut rpc,
        "propose_version",
        json!({"path": draft, "label": "L", "full_text": "New."}),
    );
    assert!(!err && text.starts_with("Created version "), "{text}");
    let (text, err) = call(
        &mut rpc,
        "propose_version",
        json!({"path": draft, "label": "L", "edits": []}),
    );
    assert!(err);
    assert_eq!(text, "Pass `edits` or `full_text`.");
    let (text, err) = call(&mut rpc, "get_context", json!({}));
    assert!(!err);
    assert!(text.contains("\"app\" : \"Fluent Writer isn't running.\""));

    drop(stdin);
    assert!(child.wait().unwrap().success());
    let notes: Vec<_> = std::fs::read_dir(support.join("Notes"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .collect();
    assert_eq!(notes.len(), 1);
}
