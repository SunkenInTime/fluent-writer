//! fluent-writer-mcp: an MCP server (stdio) that lets outside agents such as Claude Code or Codex see
//! the draft open in Fluent Writer, its comments, versions and Assist history, and leave comments or
//! propose versions the author reviews in the app (port of Swift `FluentWriterMCP/main.swift`).
//!
//! Usage: fluent-writer-mcp [--support <dir>] [--variant dev]

mod tools;

use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::json;
use writer_agents::JsonValue;
use writer_core::{AppVariant, NotesStore};

use tools::Server;

const INSTRUCTIONS: &str = "Fluent Writer is the author's Markdown writing app. Call get_context first to see the open draft, the \
author's selection and comments, versions awaiting review, and their recent Assist conversation. \
Comments are the author's notes about what to change, pinned to quoted text. To change the open draft, \
call propose_version: the author previews it in the app and chooses whether to use it. Don't edit the \
open file directly.";

fn main() {
    let mut support_directory = AppVariant::Standard.support_directory();
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--support") => {
                if let Some(v) = args.next() {
                    support_directory = absolute(expand_tilde(PathBuf::from(v)));
                }
            }
            Some("--variant") => {
                if let Some(variant) = args
                    .next()
                    .and_then(|v| v.to_str()?.parse::<AppVariant>().ok())
                {
                    support_directory = variant.support_directory();
                }
            }
            Some("--help") | Some("-h") => {
                println!(
                    "fluent-writer-mcp [--support <dir>] [--variant standard|dev]\nSpeaks the Model Context Protocol over stdio."
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }

    let mut server = Server {
        store: NotesStore::new(&support_directory),
        support_directory,
        agent_name: "Outside agent".to_string(),
    };

    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match input.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
        }
        let line = String::from_utf8_lossy(&buf);
        if line.is_empty() {
            continue;
        }
        let Some(msg) = JsonValue::parse(&line) else {
            continue;
        };
        if let JsonValue::Array(batch) = msg {
            for m in &batch {
                handle(&mut server, m);
            }
        } else {
            handle(&mut server, &msg);
        }
    }
}

/// `expandingTildeInPath` for `~` and `~/…`.
fn expand_tilde(p: PathBuf) -> PathBuf {
    let Ok(rest) = p.strip_prefix("~") else {
        return p;
    };
    match dirs::home_dir() {
        Some(home) if rest.as_os_str().is_empty() => home,
        Some(home) => home.join(rest),
        None => p,
    }
}

/// `URL(fileURLWithPath:)`: relative paths are taken from the current directory.
fn absolute(p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        return p;
    }
    std::env::current_dir().map(|d| d.join(&p)).unwrap_or(p)
}

fn send(v: &JsonValue) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(format!("{}\n", v.serialized()).as_bytes());
    let _ = out.flush();
}

fn handle(server: &mut Server, msg: &JsonValue) {
    let Some(method) = msg.get("method").and_then(JsonValue::string_value) else {
        return;
    };
    let id = msg.get("id");
    let params = msg.get("params").cloned().unwrap_or_default();
    let reply = |result: JsonValue| {
        if let Some(id) = id {
            send(&JsonValue::object([
                ("jsonrpc", "2.0".into()),
                ("id", id.clone()),
                ("result", result),
            ]));
        }
    };
    match method {
        "initialize" => {
            let client = params.get("clientInfo");
            let field = |k: &str| {
                client
                    .and_then(|c| c.get(k))
                    .and_then(JsonValue::string_value)
            };
            let chosen = field("title").or_else(|| field("name"));
            let raw = chosen.unwrap_or("").to_lowercase();
            if raw.contains("claude") {
                server.agent_name = "Claude Code".into();
            } else if raw.contains("codex") {
                server.agent_name = "Codex CLI".into();
            } else if let Some(t) = chosen.filter(|t| !t.is_empty()) {
                server.agent_name = t.to_string();
            }
            let mut result = JsonValue::from(json!({
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "fluent-writer", "title": "Fluent Writer", "version": "0.2.0"},
                "instructions": INSTRUCTIONS,
            }));
            if let JsonValue::Object(o) = &mut result {
                o.insert(
                    "protocolVersion".into(),
                    params
                        .get("protocolVersion")
                        .cloned()
                        .unwrap_or_else(|| "2025-06-18".into()),
                );
            }
            reply(result);
        }
        "ping" => reply(JsonValue::object::<String>([])),
        "tools/list" => reply(JsonValue::object([(
            "tools",
            JsonValue::Array(tools::tools()),
        )])),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(JsonValue::string_value)
                .unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| JsonValue::object::<String>([]));
            let (text, is_error) = match server.call(name, &args) {
                Ok(t) => (t, false),
                Err(e) => (e, true),
            };
            reply(JsonValue::object([
                (
                    "content",
                    JsonValue::Array(vec![JsonValue::object([
                        ("type", "text".into()),
                        ("text", text.into()),
                    ])]),
                ),
                ("isError", is_error.into()),
            ]));
        }
        _ => {
            if method.starts_with("notifications/") {
                return;
            }
            if let Some(id) = id {
                send(&JsonValue::object([
                    ("jsonrpc", "2.0".into()),
                    ("id", id.clone()),
                    (
                        "error",
                        JsonValue::object([
                            ("code", JsonValue::Number(-32601.0)),
                            ("message", format!("Method not found: {method}").into()),
                        ]),
                    ),
                ]));
            }
        }
    }
}
