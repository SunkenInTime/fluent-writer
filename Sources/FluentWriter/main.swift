import AppKit
import WriterCore

final class AppDelegate: NSObject, NSApplicationDelegate {
    var editor: EditorController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        Theme.registerFonts()
        NSApp.appearance = Preferences.appearance.nsAppearance
        NSApp.mainMenu = MainMenu.build()
        let library: DraftLibrary
        do {
            library = try DraftLibrary.defaultLibrary()
        } catch {
            NSAlert(error: error).runModal()
            NSApp.terminate(nil)
            return
        }
        let doc: DraftDocument
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--open"), i + 1 < args.count, let opened = try? library.open(URL(fileURLWithPath: args[i + 1])) {
            doc = opened
        } else if let orphan = library.orphanedRecoveries().first {
            doc = library.restore(orphan)
        } else {
            doc = library.newDraft()
        }
        let editor = EditorController(library: library, document: doc)
        self.editor = editor
        editor.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func application(_ application: NSApplication, open urls: [URL]) {
        if let url = urls.first { editor?.open(url) }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let editor else { return .terminateNow }
        editor.assist?.stop()
        return editor.confirmLeavingDocument() ? .terminateNow : .terminateCancel
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
