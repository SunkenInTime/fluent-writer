import AppKit
import WriterAgents
import WriterCore

/// Shows how to give Claude Code or Codex access to drafts, comments and versions through the
/// bundled MCP server, and can register it with either CLI.
enum OutsideAgentSetup {
    static var serverURL: URL? {
        guard let exe = Bundle.main.executableURL else { return nil }
        let url = exe.deletingLastPathComponent().appendingPathComponent("fluent-writer-mcp")
        return FileManager.default.isExecutableFile(atPath: url.path) ? url : nil
    }

    static var serverName: String { AppVariant.current == .dev ? "fluent-writer-dev" : "fluent-writer" }

    static func quoted(_ s: String) -> String { "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'" }

    static func serverArguments(_ server: URL, support: URL) -> [String] { [server.path, "--support", support.path] }

    static func claudeCommand(_ server: URL, support: URL) -> String {
        "claude mcp add --scope user \(serverName) -- " + serverArguments(server, support: support).map(quoted).joined(separator: " ")
    }

    static func codexCommand(_ server: URL, support: URL) -> String {
        "codex mcp add \(serverName) -- " + serverArguments(server, support: support).map(quoted).joined(separator: " ")
    }
}

extension EditorController {
    @objc func connectOutsideAgent(_ sender: Any?) {
        guard let window else { return }
        let support = library.supportDirectory
        guard let server = OutsideAgentSetup.serverURL else {
            let alert = NSAlert()
            alert.messageText = "The agent connector is missing from this build."
            alert.informativeText = "Build the app with script/build-app.sh, which bundles fluent-writer-mcp."
            alert.beginSheetModal(for: window)
            return
        }
        let claude = OutsideAgentSetup.claudeCommand(server, support: support)
        let codex = OutsideAgentSetup.codexCommand(server, support: support)

        let alert = NSAlert()
        alert.messageText = "Connect an outside agent"
        alert.informativeText = "Claude Code or Codex can then see the draft open here, your selection, comments, versions and Assist history. They can leave comments and propose versions, which you preview and choose here, the same as Assist's. Run one of these in Terminal, or click a button to do it now."
        let width: CGFloat = 460
        let accessory = NSView(frame: NSRect(x: 0, y: 0, width: width, height: 150))
        func block(_ title: String, _ command: String, y: CGFloat) {
            let t = NSTextField(labelWithString: title)
            t.font = NSFont.systemFont(ofSize: 11, weight: .medium)
            t.frame = NSRect(x: 0, y: y + 50, width: width, height: 16)
            accessory.addSubview(t)
            let f = NSTextField(wrappingLabelWithString: command)
            f.font = Theme.uiFont(size: 10)
            f.isSelectable = true
            f.textColor = .secondaryLabelColor
            f.frame = NSRect(x: 0, y: y, width: width, height: 46)
            accessory.addSubview(f)
        }
        block("Claude Code", claude, y: 84)
        block("Codex", codex, y: 10)
        alert.accessoryView = accessory
        alert.addButton(withTitle: "Add to Claude Code")
        alert.addButton(withTitle: "Add to Codex")
        alert.addButton(withTitle: "Copy Commands")
        alert.addButton(withTitle: "Done")
        alert.beginSheetModal(for: window) { [weak self] response in
            switch response {
            case .alertFirstButtonReturn:
                self?.register(tool: "claude", args: ["mcp", "add", "--scope", "user", OutsideAgentSetup.serverName, "--"] + OutsideAgentSetup.serverArguments(server, support: support), name: "Claude Code")
            case .alertSecondButtonReturn:
                self?.register(tool: "codex", args: ["mcp", "add", OutsideAgentSetup.serverName, "--"] + OutsideAgentSetup.serverArguments(server, support: support), name: "Codex")
            case .alertThirdButtonReturn:
                let pb = NSPasteboard.general
                pb.clearContents()
                pb.setString("# Claude Code\n\(claude)\n\n# Codex\n\(codex)\n", forType: .string)
                self?.flashMessage("Copied the connect commands")
            default: break
            }
        }
    }

    private func register(tool: String, args: [String], name: String) {
        guard let exe = ExecutableLocator.find(tool) else {
            flashMessage("\(name) isn't installed")
            return
        }
        let p = Process()
        p.executableURL = exe
        p.arguments = args
        p.environment = ExecutableLocator.environment(adding: [exe.deletingLastPathComponent()])
        let out = Pipe()
        p.standardOutput = out
        p.standardError = out
        p.terminationHandler = { [weak self] proc in
            let text = String(data: out.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            DispatchQueue.main.async {
                guard let self else { return }
                if proc.terminationStatus == 0 {
                    self.flashMessage("Added to \(name). Start a new \(name) session to use it.")
                } else {
                    let alert = NSAlert()
                    alert.messageText = "\(name) didn't add the connector."
                    alert.informativeText = text.isEmpty ? "It exited with status \(proc.terminationStatus)." : String(text.suffix(600))
                    if let w = self.window { alert.beginSheetModal(for: w) } else { alert.runModal() }
                }
            }
        }
        do { try p.run() } catch { flashMessage("Couldn't run \(tool): \(error.localizedDescription)") }
    }
}
