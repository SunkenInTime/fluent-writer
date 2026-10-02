import AppKit
import WriterCore

/// Compact, keyboard-first picker for recent drafts, shown as a sheet.
final class OpenPicker: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate {
    enum Choice { case draft(URL), browse, new }

    private let library: DraftLibrary
    private let current: URL?
    private let completion: (Choice) -> Void
    private var all: [DraftSummary] = []
    private var shown: [DraftSummary] = []
    private let search = NSSearchField()
    private let table = NSTableView()
    private let empty = NSTextField(labelWithString: "No drafts yet")
    private static var active: OpenPicker?

    static func present(on window: NSWindow, library: DraftLibrary, current: URL?, completion: @escaping (Choice) -> Void) {
        let picker = OpenPicker(library: library, current: current, completion: completion)
        active = picker
        let sheet = NSWindow(contentViewController: picker)
        sheet.styleMask = [.titled]
        window.beginSheet(sheet)
    }

    init(library: DraftLibrary, current: URL?, completion: @escaping (Choice) -> Void) {
        self.library = library
        self.current = current
        self.completion = completion
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { fatalError() }

    override func loadView() {
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 520, height: 420))
        search.placeholderString = "Find a draft"
        search.font = Theme.uiFont(size: 14)
        search.delegate = self
        search.focusRingType = .none
        search.setAccessibilityLabel("Find a draft")
        search.translatesAutoresizingMaskIntoConstraints = false

        let column = NSTableColumn(identifier: .init("draft"))
        table.addTableColumn(column)
        table.headerView = nil
        table.rowHeight = 50
        table.style = .plain
        table.intercellSpacing = NSSize(width: 0, height: 0)
        table.backgroundColor = .clear
        table.dataSource = self
        table.delegate = self
        table.target = self
        table.doubleAction = #selector(chooseSelected)
        table.setAccessibilityLabel("Recent drafts")
        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        scroll.translatesAutoresizingMaskIntoConstraints = false

        empty.textColor = Theme.quiet
        empty.font = Theme.uiFont(size: 13)
        empty.translatesAutoresizingMaskIntoConstraints = false

        let browse = NSButton(title: "Other File…", target: self, action: #selector(browse))
        let newButton = NSButton(title: "New Draft", target: self, action: #selector(newDraft))
        let cancel = NSButton(title: "Cancel", target: self, action: #selector(cancel))
        cancel.keyEquivalent = "\u{1b}"
        let open = NSButton(title: "Open", target: self, action: #selector(chooseSelected))
        open.keyEquivalent = "\r"
        let buttons = NSStackView(views: [browse, newButton, NSView(), cancel, open])
        buttons.translatesAutoresizingMaskIntoConstraints = false

        for v in [search, scroll, empty, buttons] { root.addSubview(v) }
        NSLayoutConstraint.activate([
            search.topAnchor.constraint(equalTo: root.topAnchor, constant: 16),
            search.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 16),
            search.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -16),
            scroll.topAnchor.constraint(equalTo: search.bottomAnchor, constant: 10),
            scroll.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 8),
            scroll.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -8),
            scroll.bottomAnchor.constraint(equalTo: buttons.topAnchor, constant: -12),
            empty.centerXAnchor.constraint(equalTo: scroll.centerXAnchor),
            empty.centerYAnchor.constraint(equalTo: scroll.centerYAnchor),
            buttons.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 16),
            buttons.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -16),
            buttons.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -14),
        ])
        view = root
        all = library.recentDrafts()
        filter()
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(search)
    }

    private func filter() {
        let q = search.stringValue.trimmingCharacters(in: .whitespaces).lowercased()
        shown = q.isEmpty ? all : all.filter { $0.title.lowercased().contains(q) || $0.preview.lowercased().contains(q) }
        table.reloadData()
        empty.isHidden = !shown.isEmpty
        empty.stringValue = all.isEmpty ? "No drafts yet" : "No matching drafts"
        if !shown.isEmpty {
            let currentIndex = shown.firstIndex { $0.url == current }
            let row = (currentIndex == 0 && shown.count > 1 && q.isEmpty) ? 1 : 0
            table.selectRowIndexes([row], byExtendingSelection: false)
            table.scrollRowToVisible(row)
        }
    }

    func controlTextDidChange(_ obj: Notification) { filter() }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        switch selector {
        case #selector(NSResponder.moveDown(_:)): moveSelection(1); return true
        case #selector(NSResponder.moveUp(_:)): moveSelection(-1); return true
        case #selector(NSResponder.insertNewline(_:)): chooseSelected(); return true
        case #selector(NSResponder.cancelOperation(_:)): cancel(); return true
        default: return false
        }
    }

    private func moveSelection(_ d: Int) {
        guard !shown.isEmpty else { return }
        let row = max(0, min(shown.count - 1, table.selectedRow + d))
        table.selectRowIndexes([row], byExtendingSelection: false)
        table.scrollRowToVisible(row)
    }

    func numberOfRows(in tableView: NSTableView) -> Int { shown.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let item = shown[row]
        let cell = NSTableCellView()
        let title = NSTextField(labelWithString: item.title)
        title.font = Theme.uiFont(size: 13, weight: .medium)
        title.lineBreakMode = .byTruncatingTail
        let df = RelativeDateTimeFormatter()
        df.unitsStyle = .short
        let when = item.modified == .distantPast ? "" : df.localizedString(for: item.modified, relativeTo: Date())
        let detail = NSTextField(labelWithString: [when, item.preview].filter { !$0.isEmpty }.joined(separator: " — "))
        detail.font = Theme.uiFont(size: 11)
        detail.textColor = .secondaryLabelColor
        detail.lineBreakMode = .byTruncatingTail
        let stack = NSStackView(views: [title, detail])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 3
        stack.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 10),
            stack.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -10),
            stack.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
        ])
        cell.setAccessibilityLabel("\(item.title), \(when)")
        return cell
    }

    private func finish(_ choice: Choice?) {
        guard let sheet = view.window, let parent = sheet.sheetParent else { return }
        parent.endSheet(sheet)
        OpenPicker.active = nil
        if let choice { DispatchQueue.main.async { self.completion(choice) } }
    }

    @objc func chooseSelected() {
        guard table.selectedRow >= 0, table.selectedRow < shown.count else { return }
        finish(.draft(shown[table.selectedRow].url))
    }

    @objc func browse() { finish(.browse) }
    @objc func newDraft() { finish(.new) }
    @objc func cancel() { finish(nil) }
}
