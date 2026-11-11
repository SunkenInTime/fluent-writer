//! Port of `Tests/WriterAgentsTests/AgentTests.swift`.

use std::sync::{Arc, Mutex};

use serde_json::json;
use writer_agents::{
    AgentAction, AgentEvent, AgentPrompt, AgentProvider, AgentRequest, AgentSession, ClaudeAgent,
    CodexAgent, HistoryEntry, JsonValue, ProcessSink, PromptComment,
};
use writer_core::{EditPosition, ProposedEdit};

fn no_sink() -> ProcessSink {
    Arc::new(|_| {})
}

// MARK: AgentPromptTests

#[test]
fn prompt_sends_whole_draft_with_selection_as_focus() {
    let mut req = AgentRequest::new(
        AgentProvider::Codex,
        AgentAction::Shorten,
        "keep it casual",
        "Intro.\n\nHello world.",
        "Note",
    );
    req.selection = Some("Hello world.".into());
    let p = AgentPrompt::build(&req);
    assert!(p.contains("<draft>\nIntro.\n\nHello world.\n</draft>"));
    assert!(p.contains("<selection>\nHello world.\n</selection>"));
    assert!(p.contains("<edit at=\"end\">"));
    assert!(p.contains("keep it casual"));
    assert!(p.contains("Do not run commands"));
}

#[test]
fn critique_does_not_ask_for_edits() {
    let req = AgentRequest::new(AgentProvider::Claude, AgentAction::Critique, "", "x", "");
    let p = AgentPrompt::build(&req);
    assert!(!p.contains("<old>"));
    assert!(p.contains("Do not include any edits."));
}

#[test]
fn comments_and_history_are_included() {
    let mut req = AgentRequest::new(
        AgentProvider::Codex,
        AgentAction::ResolveComments,
        "",
        "A b c.",
        "",
    );
    req.comments = vec![PromptComment::new("c1", "b", "make bolder")];
    req.history = vec![HistoryEntry::new("add a line", "Added.")];
    let p = AgentPrompt::build(&req);
    assert!(p.contains("<comment id=\"c1\">"));
    assert!(p.contains("make bolder"));
    assert!(p.contains("<edit for=\"COMMENT_ID\">"));
    assert!(p.contains("Author asked: add a line"));
}

#[test]
fn parse_feedback_edits_and_replies() {
    let out = "<feedback>
Tighter now.
</feedback>
<edit for=\"c1\">
<old>Long line here.</old>
<new>Short line.</new>
</edit>
<edit at=\"end\">
<new>
A closing thought.
</new>
</edit>
<reply for=\"c2\">It reads fine to me.</reply>";
    let r = AgentPrompt::parse(out);
    assert_eq!(r.feedback, "Tighter now.");
    assert_eq!(
        r.edits,
        vec![
            ProposedEdit::replace("Long line here.", "Short line.")
                .with_comment_ids(vec!["c1".into()]),
            ProposedEdit::insert("A closing thought.", EditPosition::End),
        ]
    );
    assert_eq!(r.replies.len(), 1);
    assert_eq!(r.replies["c2"], "It reads fine to me.");
}

#[test]
fn parse_streaming_partial_edit_is_withheld() {
    let r = AgentPrompt::parse("<feedback>Good.</feedback>\n<edit>\n<old>Half</old>\n<new>a sen");
    assert_eq!(r.feedback, "Good.");
    assert!(r.edits.is_empty());
    assert!(AgentPrompt::is_drafting_edits(
        "<feedback>Good.</feedback>\n<edit>"
    ));
}

#[test]
fn parse_partial_feedback() {
    assert_eq!(
        AgentPrompt::parse("<feedback>\nThe opening is").feedback,
        "The opening is"
    );
}

#[test]
fn parse_untagged_falls_back_to_feedback() {
    let r = AgentPrompt::parse("Just some notes.");
    assert_eq!(r.feedback, "Just some notes.");
    assert!(r.edits.is_empty());
}

#[test]
fn empty_new_is_a_deletion() {
    let r = AgentPrompt::parse(
        "<feedback>Cut it.</feedback><edit><old>Remove me.</old><new></new></edit>",
    );
    assert_eq!(r.edits, vec![ProposedEdit::replace("Remove me.", "")]);
}

// MARK: CodexProtocolTests

#[derive(Default)]
struct Recorder {
    sent: Vec<JsonValue>,
    events: Vec<AgentEvent>,
}

type Shared = Arc<Mutex<Recorder>>;

fn make_agent() -> (CodexAgent, Shared) {
    let mut agent = CodexAgent::new(None);
    let shared: Shared = Arc::default();
    let s = shared.clone();
    agent.send_override = Some(Box::new(move |line| {
        s.lock().unwrap().sent.push(JsonValue::parse(line).unwrap());
    }));
    (agent, shared)
}

fn start(agent: &mut CodexAgent, shared: &Shared, prompt: &str) {
    let s = shared.clone();
    agent.start(
        prompt,
        Box::new(move |e| s.lock().unwrap().events.push(e)),
        no_sink(),
    );
}

fn id(v: &JsonValue) -> i64 {
    v.get("id").unwrap().number_value().unwrap() as i64
}

fn sent(shared: &Shared, i: usize) -> JsonValue {
    shared.lock().unwrap().sent[i].clone()
}

fn last_sent(shared: &Shared) -> JsonValue {
    shared.lock().unwrap().sent.last().unwrap().clone()
}

fn events(shared: &Shared) -> Vec<AgentEvent> {
    shared.lock().unwrap().events.clone()
}

fn handshake(agent: &mut CodexAgent, shared: &Shared) {
    start(agent, shared, "PROMPT");
    let init = sent(shared, 0);
    assert_eq!(init.at(&["method"]).unwrap(), &"initialize");
    agent.receive(&format!(r#"{{"id":{},"result":{{}}}}"#, id(&init)));
    assert_eq!(sent(shared, 1).at(&["method"]).unwrap(), &"initialized");
    let thread = sent(shared, 2);
    assert_eq!(thread.at(&["method"]).unwrap(), &"thread/start");
    assert_eq!(thread.at(&["params", "sandbox"]).unwrap(), &"read-only");
    assert_eq!(thread.at(&["params", "approvalPolicy"]).unwrap(), &"never");
    agent.receive(&format!(
        r#"{{"id":{},"result":{{"thread":{{"id":"t1"}}}}}}"#,
        id(&thread)
    ));
    let turn = sent(shared, 3);
    assert_eq!(turn.at(&["method"]).unwrap(), &"turn/start");
    assert_eq!(turn.at(&["params", "threadId"]).unwrap(), &"t1");
    assert_eq!(
        turn.at(&["params", "sandboxPolicy", "type"]).unwrap(),
        &"readOnly"
    );
    assert_eq!(
        turn.at(&["params", "input"]).unwrap(),
        &JsonValue::from(json!([{"type": "text", "text": "PROMPT"}]))
    );
    agent.receive(&format!(
        r#"{{"id":{},"result":{{"turn":{{"id":"u1"}}}}}}"#,
        id(&turn)
    ));
}

#[test]
fn streams_and_completes() {
    let (mut agent, shared) = make_agent();
    handshake(&mut agent, &shared);
    agent.receive(r#"{"method":"item/agentMessage/delta","params":{"delta":"<feedback>Hi"}}"#);
    agent.receive(r#"{"method":"item/agentMessage/delta","params":{"delta":"</feedback>"}}"#);
    agent.receive(
        r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
    );
    assert_eq!(
        events(&shared),
        vec![
            AgentEvent::Progress("<feedback>Hi".into()),
            AgentEvent::Progress("<feedback>Hi</feedback>".into()),
            AgentEvent::Completed("<feedback>Hi</feedback>".into()),
        ]
    );
}

#[test]
fn declines_approvals() {
    let (mut agent, shared) = make_agent();
    handshake(&mut agent, &shared);
    agent.receive(r#"{"id":99,"method":"item/commandExecution/requestApproval","params":{}}"#);
    assert_eq!(
        last_sent(&shared).at(&["result", "decision"]).unwrap(),
        &"decline"
    );
    agent.receive(r#"{"id":100,"method":"item/tool/requestUserInput","params":{}}"#);
    assert!(last_sent(&shared).get("error").is_some());
}

#[test]
fn cancel_interrupts_and_drops_late_output() {
    let (mut agent, shared) = make_agent();
    handshake(&mut agent, &shared);
    agent.receive(r#"{"method":"item/agentMessage/delta","params":{"delta":"a"}}"#);
    agent.cancel();
    assert_eq!(
        last_sent(&shared).at(&["method"]).unwrap(),
        &"turn/interrupt"
    );
    assert_eq!(last_sent(&shared).at(&["params", "turnId"]).unwrap(), &"u1");
    agent.receive(r#"{"method":"item/agentMessage/delta","params":{"delta":"late"}}"#);
    agent.receive(
        r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
    );
    assert_eq!(events(&shared), vec![AgentEvent::Progress("a".into())]);
}

#[test]
fn auth_error_is_friendly() {
    let (mut agent, shared) = make_agent();
    start(&mut agent, &shared, "p");
    agent.receive(&format!(
        r#"{{"id":{},"error":{{"code":1,"message":"Not logged in: run codex login"}}}}"#,
        id(&sent(&shared, 0))
    ));
    let Some(AgentEvent::Failed(msg)) = events(&shared).last().cloned() else {
        panic!("expected a failure");
    };
    assert!(msg.contains("codex login"));
}

#[test]
fn missing_executable_fails() {
    let mut agent = CodexAgent::new(None);
    let got: Arc<Mutex<Option<AgentEvent>>> = Arc::default();
    let g = got.clone();
    agent.start(
        "p",
        Box::new(move |e| *g.lock().unwrap() = Some(e)),
        no_sink(),
    );
    assert!(matches!(*got.lock().unwrap(), Some(AgentEvent::Failed(_))));
}

// MARK: ClaudeBridgeTests

#[test]
fn bridge_events_and_cancel() {
    let mut agent = ClaudeAgent::new(None, None);
    let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::default();
    let e = events.clone();
    agent.start(
        "p",
        Box::new(move |ev| e.lock().unwrap().push(ev)),
        no_sink(),
    );
    assert!(matches!(
        events.lock().unwrap().last(),
        Some(AgentEvent::Failed(_))
    ));
    agent.receive(r#"{"type":"delta","text":"late"}"#);
    assert_eq!(events.lock().unwrap().len(), 1);
}

#[test]
fn json_round_trip() {
    let v: JsonValue = json!({"a": [1, "two", true], "b": null}).into();
    assert_eq!(JsonValue::parse(&v.serialized()), Some(v));
}
