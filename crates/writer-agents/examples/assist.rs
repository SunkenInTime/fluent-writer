//! Sends one Assist request from the command line and prints its events.
//!
//! `cargo run -p writer-agents --example assist -- codex|claude "<instruction>" < draft.md`

use std::io::Read;
use std::sync::mpsc;

use writer_agents::{AgentAction, AgentEvent, AgentHandle, AgentPrompt, AgentRequest};

fn main() {
    let mut args = std::env::args().skip(1);
    let provider = args.next().and_then(|p| p.parse().ok()).unwrap_or_default();
    let instruction = args.next().unwrap_or_default();
    let mut draft = String::new();
    std::io::stdin()
        .read_to_string(&mut draft)
        .expect("draft on stdin");

    let action = if instruction.is_empty() {
        AgentAction::Critique
    } else {
        AgentAction::Custom
    };
    let request = AgentRequest::new(provider, action, instruction, draft, "");
    // The wake callback would post a redraw to the UI; here it nudges the main thread.
    let (wake_tx, wake_rx) = mpsc::channel();
    let handle = AgentHandle::start_request(&request, move || {
        let _ = wake_tx.send(());
    });
    while wake_rx.recv().is_ok() {
        while let Some(event) = handle.try_recv() {
            match event {
                AgentEvent::Progress(text) => eprintln!("progress: {} chars", text.len()),
                AgentEvent::Completed(text) => {
                    let reply = AgentPrompt::parse(&text);
                    println!("feedback: {}", reply.feedback);
                    for e in reply.edits {
                        println!("edit: {e:?}");
                    }
                    return;
                }
                AgentEvent::Failed(message) => {
                    println!("failed: {message}");
                    return;
                }
            }
        }
    }
}
