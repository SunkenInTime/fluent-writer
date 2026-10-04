import AppKit
import WriterCore

enum RemoteStatus: Equatable {
    case idle, uploading, uploaded, offline
    case failed(String)
}

/// Uploads saved text one at a time; while an upload runs, only the newest pending text is kept.
final class RemoteUploader {
    let ssh = SSHTransport()
    private let queue = DispatchQueue(label: "FluentWriter.upload")
    private var inFlight = false
    private var queued: (DraftDocument, String)?
    var onFinish: ((DraftDocument, String, Error?) -> Void)?

    func upload(_ doc: DraftDocument, body: String) {
        guard let loc = doc.remote else { return }
        if inFlight { queued = (doc, body); return }
        inFlight = true
        let ssh = self.ssh
        queue.async { [weak self] in
            var failure: Error?
            do { try ssh.write(Data(body.utf8), to: loc) } catch { failure = error }
            DispatchQueue.main.async {
                guard let self else { return }
                self.inFlight = false
                self.onFinish?(doc, body, failure)
                if let next = self.queued {
                    self.queued = nil
                    self.upload(next.0, body: next.1)
                }
            }
        }
    }

    /// Waits for any running upload, then uploads `body` before returning.
    func uploadNow(_ doc: DraftDocument, body: String) throws {
        guard let loc = doc.remote else { return }
        queued = nil
        let ssh = self.ssh
        try queue.sync { try ssh.write(Data(body.utf8), to: loc) }
    }
}

extension EditorController {
    // MARK: Sidebar

    @objc func toggleFileSidebar(_ sender: Any?) {
        if sidebarVisible {
            sidebar.panel.removeFromSuperview()
        } else {
            canvas.addSubview(sidebar.panel)
            sidebar.reload()
        }
        Preferences.showSidebar = sidebarVisible
        trafficLightButtons.forEach { $0.alphaValue = 1 }
        showChrome(autoHide: true)
        frameChanged()
    }

    func showSidebar(_ mode: SidebarController.Mode) {
        if !sidebarVisible { toggleFileSidebar(nil) }
        sidebar.setMode(mode)
    }

    @objc func showRecentFiles(_ sender: Any?) { showSidebar(.recent) }

    private var trafficLightButtons: [NSView] {
        guard let w = window else { return [] }
        return [.closeButton, .miniaturizeButton, .zoomButton].compactMap { w.standardWindowButton($0) }
    }

    // MARK: Folders

    @objc func openFolder(_ sender: Any?) {
        guard let window else { return }
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Open Folder"
        panel.message = "Choose a folder to list its Markdown and text files in the sidebar."
        if case let .local(url)? = sidebar.folderStack.first { panel.directoryURL = url }
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let url = panel.url, let self else { return }
            if !self.sidebarVisible { self.toggleFileSidebar(nil) }
            self.sidebar.setFolder(.local(url.standardizedFileURL))
        }
    }

    // MARK: SSH

    @objc func openOverSSH(_ sender: Any?) {
        guard let window else { return }
        let alert = NSAlert()
        alert.messageText = "Open over SSH"
        alert.informativeText = "Enter a file to edit or a folder to browse, like me@server:~/notes or ssh://me@server:2222/srv/notes/draft.md. Your SSH config and keys are used; password prompts aren't supported."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 340, height: 24))
        field.placeholderString = "user@host:~/notes/draft.md"
        field.stringValue = Preferences.lastSSHLocation
        field.setAccessibilityLabel("SSH location")
        alert.accessoryView = field
        alert.addButton(withTitle: "Open")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] response in
            guard response == .alertFirstButtonReturn, let self else { return }
            let text = field.stringValue
            guard let loc = RemoteLocation.parse(text) else {
                let bad = NSAlert()
                bad.messageText = "That doesn't look like an SSH location."
                bad.informativeText = "Use host:path, user@host:path, or ssh://user@host:port/path."
                bad.beginSheetModal(for: window)
                return
            }
            Preferences.lastSSHLocation = text
            self.resolveRemote(loc)
        }
    }

    private func resolveRemote(_ loc: RemoteLocation) {
        flashMessage("Connecting to \(loc.hostName)…")
        let ssh = uploader.ssh
        DispatchQueue.global(qos: .userInitiated).async {
            let result = Result { try ssh.kind(loc) }
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                switch result {
                case .success(.directory):
                    if !self.sidebarVisible { self.toggleFileSidebar(nil) }
                    self.sidebar.setFolder(.remote(loc))
                case .success(.file):
                    self.openRemote(loc)
                case .success(.missing):
                    self.presentError(DraftStoreError(message: "Nothing at \(loc.display)."))
                case let .failure(error):
                    self.presentError(error)
                }
            }
        }
    }

    func openRemote(_ loc: RemoteLocation) {
        if draft.remote == loc { window?.makeFirstResponder(textView); return }
        guard confirmLeavingDocument() else { return }
        remoteOpenGeneration += 1
        let generation = remoteOpenGeneration
        flashMessage("Opening \(loc.name) from \(loc.hostName)…")
        let ssh = uploader.ssh
        DispatchQueue.global(qos: .userInitiated).async {
            let result = Result { try ssh.read(loc) }
            DispatchQueue.main.async { [weak self] in
                guard let self, generation == self.remoteOpenGeneration else { return }
                self.finishOpeningRemote(loc, result: result)
            }
        }
    }

    private func finishOpeningRemote(_ loc: RemoteLocation, result: Result<Data, Error>) {
        guard flush() else { return }
        do {
            switch result {
            case let .success(data):
                let doc = try library.openRemote(loc, contents: data)
                load(doc)
                if doc.remoteUploadPending { flashMessage("Kept your edits that hadn't reached \(loc.hostName)") }
            case let .failure(error):
                let mirror = library.mirrorURL(for: loc)
                guard library.record(for: loc) != nil, FileManager.default.fileExists(atPath: mirror.path) else { throw error }
                load(try library.open(mirror))
                remoteStatus = .offline
                updateStatus()
            }
        } catch {
            presentError(error)
        }
    }

    func uploadFinished(_ doc: DraftDocument, body: String, error: Error?) {
        library.remoteUploadFinished(doc, uploadedBody: body, succeeded: error == nil)
        guard doc === draft else { return }
        if let error {
            remoteStatus = .failed(error.localizedDescription)
        } else {
            remoteStatus = doc.remoteUploadPending ? .uploading : .uploaded
        }
        updateStatus()
    }

    /// Before leaving a remote file, makes sure its text reached the server, or that the writer chose to keep it local.
    func confirmRemoteUploaded() -> Bool {
        guard draft.remote != nil, draft.remoteUploadPending, let host = draft.remote?.hostName else { return true }
        let body = draft.savedBody
        do {
            try uploader.uploadNow(draft, body: body)
            uploadFinished(draft, body: body, error: nil)
            return true
        } catch {
            uploadFinished(draft, body: body, error: error)
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = "Your latest edits aren't on \(host) yet."
            alert.informativeText = error.localizedDescription + "\n\nThey're saved on this Mac and will be sent the next time you open this file."
            alert.addButton(withTitle: "Keep Editing")
            alert.addButton(withTitle: "Continue")
            return alert.runModal() == .alertSecondButtonReturn
        }
    }
}
