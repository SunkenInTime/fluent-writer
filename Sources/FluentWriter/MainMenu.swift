import AppKit

enum MainMenu {
    static func item(_ title: String, _ action: Selector?, _ key: String = "", _ mods: NSEvent.ModifierFlags = .command, tag: Int = 0) -> NSMenuItem {
        let i = NSMenuItem(title: title, action: action, keyEquivalent: key)
        i.keyEquivalentModifierMask = mods
        i.tag = tag
        return i
    }

    static func submenu(_ title: String, _ items: [NSMenuItem]) -> NSMenuItem {
        let parent = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        let menu = NSMenu(title: title)
        items.forEach(menu.addItem)
        parent.submenu = menu
        return parent
    }

    static func build() -> NSMenu {
        let main = NSMenu()
        let name = "Fluent Writer"

        main.addItem(submenu(name, [
            item("About \(name)", #selector(NSApplication.orderFrontStandardAboutPanel(_:)), ""),
            .separator(),
            item("Hide \(name)", #selector(NSApplication.hide(_:)), "h"),
            item("Hide Others", #selector(NSApplication.hideOtherApplications(_:)), "h", [.command, .option]),
            item("Show All", #selector(NSApplication.unhideAllApplications(_:))),
            .separator(),
            item("Quit \(name)", #selector(NSApplication.terminate(_:)), "q"),
        ]))

        main.addItem(submenu("File", [
            item("New", #selector(EditorController.newDraft(_:)), "n"),
            item("Open…", #selector(EditorController.openDraftPicker(_:)), "o"),
            item("Open Other File…", #selector(EditorController.openOtherFile(_:)), "o", [.command, .shift]),
            item("Open Folder…", #selector(EditorController.openFolder(_:)), "o", [.command, .option]),
            item("Open over SSH…", #selector(EditorController.openOverSSH(_:)), "o", [.command, .control]),
            .separator(),
            item("Save", #selector(EditorController.saveNow(_:)), "s"),
            item("Save As…", #selector(EditorController.saveDraftAs(_:)), "s", [.command, .shift]),
            item("Rename…", #selector(EditorController.renameDraft(_:)), "r", [.command, .shift]),
            item("Show in Finder", #selector(EditorController.revealInFinder(_:)), "r", [.command, .option]),
            .separator(),
            item("Copy for Twitter", #selector(EditorController.copyForTwitter(_:)), "c", [.command, .shift]),
            .separator(),
            item("Close", #selector(NSWindow.performClose(_:)), "w"),
        ]))

        let find = { (title: String, key: String, mods: NSEvent.ModifierFlags, action: NSTextFinder.Action) -> NSMenuItem in
            item(title, #selector(NSResponder.performTextFinderAction(_:)), key, mods, tag: action.rawValue)
        }
        main.addItem(submenu("Edit", [
            item("Undo", Selector(("undo:")), "z"),
            item("Redo", Selector(("redo:")), "z", [.command, .shift]),
            .separator(),
            item("Cut", #selector(NSText.cut(_:)), "x"),
            item("Copy", #selector(NSText.copy(_:)), "c"),
            item("Paste", #selector(NSText.paste(_:)), "v"),
            item("Delete", #selector(NSText.delete(_:))),
            item("Select All", #selector(NSText.selectAll(_:)), "a"),
            .separator(),
            submenu("Find", [
                find("Find…", "f", .command, .showFindInterface),
                find("Find and Replace…", "f", [.command, .option], .showReplaceInterface),
                find("Find Next", "g", .command, .nextMatch),
                find("Find Previous", "g", [.command, .shift], .previousMatch),
                find("Use Selection for Find", "e", .command, .setSearchString),
                item("Jump to Selection", #selector(NSResponder.centerSelectionInVisibleArea(_:)), "j", [.command, .option]),
            ]),
            submenu("Spelling and Grammar", [
                item("Show Spelling and Grammar", #selector(NSText.showGuessPanel(_:)), ":"),
                item("Check Document Now", #selector(NSText.checkSpelling(_:)), ";"),
                .separator(),
                item("Check Spelling While Typing", #selector(EditorController.toggleSpellCheck(_:))),
                item("Check Grammar With Spelling", #selector(NSTextView.toggleGrammarChecking(_:))),
                item("Correct Spelling Automatically", #selector(NSTextView.toggleAutomaticSpellingCorrection(_:))),
            ]),
            submenu("Substitutions", [
                item("Smart Quotes", #selector(NSTextView.toggleAutomaticQuoteSubstitution(_:))),
                item("Smart Dashes", #selector(NSTextView.toggleAutomaticDashSubstitution(_:))),
                item("Text Replacement", #selector(NSTextView.toggleAutomaticTextReplacement(_:))),
            ]),
        ]))

        main.addItem(submenu("Format", [
            item("Bold", #selector(EditorController.toggleBold(_:)), "b"),
            item("Italic", #selector(EditorController.toggleItalic(_:)), "i"),
            item("Strikethrough", #selector(EditorController.toggleStrikethrough(_:)), "u", [.command, .shift]),
            item("Link", #selector(EditorController.insertLink(_:)), "k"),
            .separator(),
            item("Heading 1", #selector(EditorController.formatHeading1(_:)), "1"),
            item("Heading 2", #selector(EditorController.formatHeading2(_:)), "2"),
            item("Heading 3", #selector(EditorController.formatHeading3(_:)), "3"),
            item("Body", #selector(EditorController.formatBody(_:)), "0", [.command, .option]),
            .separator(),
            item("Bulleted List", #selector(EditorController.formatBulletList(_:)), "l"),
            item("Numbered List", #selector(EditorController.formatNumberedList(_:)), "l", [.command, .option]),
            item("Blockquote", #selector(EditorController.formatQuote(_:)), "'"),
            .separator(),
            item("Indent", #selector(EditorController.indentLines(_:)), "]"),
            item("Outdent", #selector(EditorController.outdentLines(_:)), "["),
        ]))

        let appearanceItems: [NSMenuItem] = Appearance.allCases.map {
            let i = item($0.label, #selector(EditorController.setAppearanceFromMenu(_:)))
            i.representedObject = $0.rawValue
            return i
        }
        main.addItem(submenu("View", [
            item("Show Sidebar", #selector(EditorController.toggleFileSidebar(_:)), "s", [.command, .control]),
            item("Recent Files", #selector(EditorController.showRecentFiles(_:)), "e", [.command, .shift]),
            .separator(),
            item("Toggle Focus", #selector(EditorController.toggleFocus(_:)), "d"),
            submenu("Focus", [
                item("Off", #selector(EditorController.focusOff(_:)), "0", [.command, .control]),
                item("Sentence", #selector(EditorController.focusSentence(_:)), "1", [.command, .control]),
                item("Paragraph", #selector(EditorController.focusParagraph(_:)), "2", [.command, .control]),
                item("Typewriter", #selector(EditorController.focusTypewriter(_:)), "3", [.command, .control]),
            ]),
            .separator(),
            item("Bigger Text", #selector(EditorController.biggerText(_:)), "+"),
            item("Smaller Text", #selector(EditorController.smallerText(_:)), "-"),
            item("Default Text Size", #selector(EditorController.defaultTextSize(_:)), "0"),
            .separator(),
            submenu("Appearance", appearanceItems),
            item("Show Counts", #selector(EditorController.toggleCounts(_:)), "c", [.command, .option]),
            .separator(),
            item("Enter Full Screen", #selector(NSWindow.toggleFullScreen(_:)), "f", [.command, .control]),
        ]))

        main.addItem(submenu("Assist", [
            item("Ask Codex…", #selector(EditorController.askCodex(_:)), "j"),
            item("Ask Claude…", #selector(EditorController.askClaude(_:)), "j", [.command, .shift]),
            item("Show Assist", #selector(EditorController.openAssist(_:)), "j", [.command, .option]),
        ]))

        let window = submenu("Window", [
            item("Minimize", #selector(NSWindow.performMiniaturize(_:)), "m"),
            item("Zoom", #selector(NSWindow.performZoom(_:))),
        ])
        NSApp.windowsMenu = window.submenu
        main.addItem(window)
        return main
    }
}
