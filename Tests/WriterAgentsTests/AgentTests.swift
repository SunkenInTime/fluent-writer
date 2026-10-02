import XCTest
@testable import WriterAgents

final class AgentPromptTests: XCTestCase {
    func testPromptIncludesScopeAndFormat() {
        let req = AgentRequest(provider: .codex, action: .shorten, instruction: "keep it casual", scopeText: "Hello world.", isSelection: true, title: "Note")
        let p = AgentPrompt.build(req)
        XCTAssertTrue(p.contains("a passage selected from a draft"))
        XCTAssertTrue(p.contains("<revision>"))
        XCTAssertTrue(p.contains("keep it casual"))
        XCTAssertTrue(p.contains("<text>\nHello world.\n</text>"))
        XCTAssertTrue(p.contains("Do not run commands"))
    }

    func testCritiqueDoesNotAskForRevision() {
        let req = AgentRequest(provider: .claude, action: .critique, instruction: "", scopeText: "x", isSelection: false, title: "")
        let p = AgentPrompt.build(req)
        XCTAssertFalse(p.contains("<revision>"))
        XCTAssertTrue(p.contains("a complete draft"))
    }

    func testParseFeedbackAndRevision() {
        let r = AgentPrompt.parse("<feedback>\nTighter now.\n</feedback>\n<revision>\nShort line.\n\nSecond.\n</revision>")
        XCTAssertEqual(r.feedback, "Tighter now.")
        XCTAssertEqual(r.revision, "Short line.\n\nSecond.")
    }

    func testParseStreamingPartialRevisionIsWithheld() {
        let r = AgentPrompt.parse("<feedback>Good.</feedback>\n<revision>\nHalf a sen")
        XCTAssertEqual(r.feedback, "Good.")
        XCTAssertNil(r.revision)
    }

    func testParsePartialFeedback() {
        let r = AgentPrompt.parse("<feedback>\nThe opening is")
        XCTAssertEqual(r.feedback, "The opening is")
    }

    func testParseUntaggedFallsBackToFeedback() {
        let r = AgentPrompt.parse("Just some notes.")
        XCTAssertEqual(r.feedback, "Just some notes.")
        XCTAssertNil(r.revision)
    }

    func testEmptyRevisionIsNil() {
        let r = AgentPrompt.parse("<feedback>Fine as is.</feedback><revision>\n</revision>")
        XCTAssertNil(r.revision)
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
