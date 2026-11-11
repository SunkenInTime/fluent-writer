import XCTest
@testable import WriterAgents
import WriterCore

final class AgentPromptTests: XCTestCase {
    func testPromptSendsWholeDraftWithSelectionAsFocus() {
        let req = AgentRequest(provider: .codex, action: .shorten, instruction: "keep it casual", document: "Intro.\n\nHello world.", selection: "Hello world.", title: "Note")
        let p = AgentPrompt.build(req)
        XCTAssertTrue(p.contains("<draft>\nIntro.\n\nHello world.\n</draft>"))
        XCTAssertTrue(p.contains("<selection>\nHello world.\n</selection>"))
        XCTAssertTrue(p.contains("<edit at=\"end\">"))
        XCTAssertTrue(p.contains("keep it casual"))
        XCTAssertTrue(p.contains("Do not run commands"))
    }

    func testCritiqueDoesNotAskForEdits() {
        let req = AgentRequest(provider: .claude, action: .critique, instruction: "", document: "x", title: "")
        let p = AgentPrompt.build(req)
        XCTAssertFalse(p.contains("<old>"))
        XCTAssertTrue(p.contains("Do not include any edits."))
    }

    func testCommentsAndHistoryAreIncluded() {
        let req = AgentRequest(provider: .codex, action: .resolveComments, instruction: "", document: "A b c.", title: "",
                               comments: [PromptComment(id: "c1", quote: "b", body: "make bolder")],
                               history: [(instruction: "add a line", feedback: "Added.")])
        let p = AgentPrompt.build(req)
        XCTAssertTrue(p.contains("<comment id=\"c1\">"))
        XCTAssertTrue(p.contains("make bolder"))
        XCTAssertTrue(p.contains("<edit for=\"COMMENT_ID\">"))
        XCTAssertTrue(p.contains("Author asked: add a line"))
    }

    func testParseFeedbackEditsAndReplies() {
        let out = """
        <feedback>
        Tighter now.
        </feedback>
        <edit for="c1">
        <old>Long line here.</old>
        <new>Short line.</new>
        </edit>
        <edit at="end">
        <new>
        A closing thought.
        </new>
        </edit>
        <reply for="c2">It reads fine to me.</reply>
        """
        let r = AgentPrompt.parse(out)
        XCTAssertEqual(r.feedback, "Tighter now.")
        XCTAssertEqual(r.edits, [
            ProposedEdit(old: "Long line here.", new: "Short line.", commentIDs: ["c1"]),
            ProposedEdit(old: nil, new: "A closing thought.", position: .end),
        ])
        XCTAssertEqual(r.replies, ["c2": "It reads fine to me."])
    }

    func testParseStreamingPartialEditIsWithheld() {
        let r = AgentPrompt.parse("<feedback>Good.</feedback>\n<edit>\n<old>Half</old>\n<new>a sen")
        XCTAssertEqual(r.feedback, "Good.")
        XCTAssertTrue(r.edits.isEmpty)
        XCTAssertTrue(AgentPrompt.isDraftingEdits("<feedback>Good.</feedback>\n<edit>"))
    }

    func testParsePartialFeedback() {
        XCTAssertEqual(AgentPrompt.parse("<feedback>\nThe opening is").feedback, "The opening is")
    }

    func testParseUntaggedFallsBackToFeedback() {
        let r = AgentPrompt.parse("Just some notes.")
        XCTAssertEqual(r.feedback, "Just some notes.")
        XCTAssertTrue(r.edits.isEmpty)
    }

    func testEmptyNewIsADeletion() {
        let r = AgentPrompt.parse("<feedback>Cut it.</feedback><edit><old>Remove me.</old><new></new></edit>")
        XCTAssertEqual(r.edits, [ProposedEdit(old: "Remove me.", new: "")])
    }
}

final class CodexProtocolTests: XCTestCase {
    func makeAgent() -> (CodexAgent, Box) {
        let agent = CodexAgent(executable: nil)
        let box = Box()
        agent.sendOverride = { box.sent.append(JSONValue.parse($0)!) }
        return (agent, box)
    }

    final class Box { var sent: [JSONValue] = []; var events: [AgentEvent] = [] }

    func id(_ v: JSONValue) -> Int { Int(v["id"]!.numberValue!) }

    func handshake(_ agent: CodexAgent, _ box: Box) {
        agent.start(prompt: "PROMPT") { box.events.append($0) }
        XCTAssertEqual(box.sent[0]["method"], "initialize")
        agent.receive(#"{"id":\#(id(box.sent[0])),"result":{}}"#)
        XCTAssertEqual(box.sent[1]["method"], "initialized")
        let thread = box.sent[2]
        XCTAssertEqual(thread["method"], "thread/start")
        XCTAssertEqual(thread["params"]?["sandbox"], "read-only")
        XCTAssertEqual(thread["params"]?["approvalPolicy"], "never")
        agent.receive(#"{"id":\#(id(thread)),"result":{"thread":{"id":"t1"}}}"#)
        let turn = box.sent[3]
        XCTAssertEqual(turn["method"], "turn/start")
        XCTAssertEqual(turn["params"]?["threadId"], "t1")
        XCTAssertEqual(turn["params"]?["sandboxPolicy"]?["type"], "readOnly")
        XCTAssertEqual(turn["params"]?["input"], [["type": "text", "text": "PROMPT"]])
        agent.receive(#"{"id":\#(id(turn)),"result":{"turn":{"id":"u1"}}}"#)
    }

    func testStreamsAndCompletes() {
        let (agent, box) = makeAgent()
        handshake(agent, box)
        agent.receive(#"{"method":"item/agentMessage/delta","params":{"delta":"<feedback>Hi"}}"#)
        agent.receive(#"{"method":"item/agentMessage/delta","params":{"delta":"</feedback>"}}"#)
        agent.receive(#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#)
        XCTAssertEqual(box.events, [.progress("<feedback>Hi"), .progress("<feedback>Hi</feedback>"), .completed("<feedback>Hi</feedback>")])
    }

    func testDeclinesApprovals() {
        let (agent, box) = makeAgent()
        handshake(agent, box)
        agent.receive(#"{"id":99,"method":"item/commandExecution/requestApproval","params":{}}"#)
        XCTAssertEqual(box.sent.last?["result"]?["decision"], "decline")
        agent.receive(#"{"id":100,"method":"item/tool/requestUserInput","params":{}}"#)
        XCTAssertNotNil(box.sent.last?["error"])
    }

    func testCancelInterruptsAndDropsLateOutput() {
        let (agent, box) = makeAgent()
        handshake(agent, box)
        agent.receive(#"{"method":"item/agentMessage/delta","params":{"delta":"a"}}"#)
        agent.cancel()
        XCTAssertEqual(box.sent.last?["method"], "turn/interrupt")
        XCTAssertEqual(box.sent.last?["params"]?["turnId"], "u1")
        agent.receive(#"{"method":"item/agentMessage/delta","params":{"delta":"late"}}"#)
        agent.receive(#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#)
        XCTAssertEqual(box.events, [.progress("a")])
    }

    func testAuthErrorIsFriendly() {
        let (agent, box) = makeAgent()
        agent.start(prompt: "p") { box.events.append($0) }
        agent.receive(#"{"id":\#(id(box.sent[0])),"error":{"code":1,"message":"Not logged in: run codex login"}}"#)
        guard case let .failed(msg)? = box.events.last else { return XCTFail() }
        XCTAssertTrue(msg.contains("codex login"))
    }

    func testMissingExecutableFails() {
        let agent = CodexAgent(executable: nil)
        var got: AgentEvent?
        agent.start(prompt: "p") { got = $0 }
        guard case .failed? = got else { return XCTFail() }
    }
}

final class ClaudeBridgeTests: XCTestCase {
    func testBridgeEventsAndCancel() {
        let agent = ClaudeAgent(node: nil, bridgeScript: nil)
        var events: [AgentEvent] = []
        agent.start(prompt: "p") { events.append($0) }
        guard case .failed? = events.last else { return XCTFail() }
        agent.receive(#"{"type":"delta","text":"late"}"#)
        XCTAssertEqual(events.count, 1)
    }

    func testJSONRoundTrip() {
        let v: JSONValue = ["a": [1, "two", true], "b": .null]
        XCTAssertEqual(JSONValue.parse(v.serialized()), v)
    }
}
