import AppKit
import WriterCore

enum FolderRoot: Equatable {
    case local(URL)
    case remote(RemoteLocation)

    var name: String {
        switch self {
        case let .local(url): return url.lastPathComponent
        case let .remote(loc): return loc.name == "~" ? "\(loc.hostName) ~" : loc.name
        }
    }

    var detail: String {
        switch self {
        case let .local(url): return (url.path as NSString).abbreviatingWithTildeInPath
        case let .remote(loc): return loc.display
        }
    }
}

struct SidebarItem {
    enum Kind { case file, directory, parent }
    var kind: Kind
    var title: String
    var subtitle: String?
    var url: URL?
    var remote: RemoteLocation?
}

final class SidebarRowView: NSTableRowView {
    var isCurrent = false { didSet { needsDisplay = true } }

    override func drawBackground(in dirtyRect: NSRect) {
        guard isCurrent, !isSelected else { return }
        Theme.sidebarCurrent.setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: 8, dy: 1), xRadius: 6, yRadius: 6).fill()
    }

    override func drawSelection(in dirtyRect: NSRect) {
        Theme.sidebarSelection.setFill()
        NSBezierPath(roundedRect: bounds.insetBy(dx: 8, dy: 1), xRadius: 6, yRadius: 6).fill()
    }

    override var interiorBackgroundStyle: NSView.BackgroundStyle { .normal }
}

final class SidebarTable: NSTableView {
    var onReturn: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36 || event.keyCode == 76, selectedRow >= 0 { onReturn?(); return }
        super.keyDown(with: event)
    }
}

final class SidebarPanel: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        Theme.sidebar.setFill()
        bounds.fill()
        Theme.hairline.setFill()
        NSRect(x: bounds.maxX - 1, y: 0, width: 1, height: bounds.height).fill()
    }
}

/// Recent files and the contents of an opened folder (local or over SSH), one click from the editor.
final class SidebarController: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    enum Mode: String { case recent, folder }

    let panel = SidebarPanel()
    private let modes = NSSegmentedControl(labels: ["Recent", "Folder"], trackingMode: .selectOne, target: nil, action: nil)
    private let header = NSTextField(labelWithString: "")
    private let refreshButton = NSButton()
    private let openFolderButton = NSButton(title: "Open Folder…", target: nil, action: nil)
    private let message = NSTextField(wrappingLabelWithString: "")
    private let table = SidebarTable()
    private let scroll = NSScrollView()

    private weak var editor: EditorController?
    private let library: DraftLibrary
    private let ssh = SSHTransport()
    private(set) var mode: Mode = Preferences.sidebarMode
    private(set) var folderStack: [FolderRoot] = []
    private var items: [SidebarItem] = []
    private var listingGeneration = 0

    static let width: CGFloat = 260
    static let topInset: CGFloat = 40

    init(editor: EditorController, library: DraftLibrary) {
        self.editor = editor
        self.library = library
        super.init()
        if let root = Preferences.folderRoot { folderStack = [root] }
        build()
    }

    private func build() {
        modes.target = self
        modes.action = #selector(modeChanged)
        modes.selectedSegment = mode == .recent ? 0 : 1
        modes.controlSize = .small
        modes.font = Theme.uiFont(size: 11)
        modes.segmentDistribution = .fillEqually
        modes.setAccessibilityLabel("Sidebar view")
        panel.addSubview(modes)

        header.font = Theme.uiFont(size: 11)
        header.textColor = Theme.quiet
        header.lineBreakMode = .byTruncatingMiddle
        panel.addSubview(header)

        refreshButton.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: "Refresh folder")
        refreshButton.isBordered = false
        refreshButton.contentTintColor = Theme.quiet
        refreshButton.target = self
        refreshButton.action = #selector(refreshClicked)
        refreshButton.toolTip = "Refresh"
        panel.addSubview(refreshButton)

        openFolderButton.target = self
        openFolderButton.action = #selector(openFolderClicked)
        openFolderButton.bezelStyle = .rounded
        openFolderButton.controlSize = .small
        panel.addSubview(openFolderButton)

        message.font = Theme.uiFont(size: 12)
        message.textColor = Theme.quiet
        message.alignment = .center
        panel.addSubview(message)

        let column = NSTableColumn(identifier: .init("item"))
        table.addTableColumn(column)
        table.headerView = nil
        table.rowHeight = 42
        table.style = .plain
        table.intercellSpacing = NSSize(width: 0, height: 0)
        table.backgroundColor = .clear
        table.focusRingType = .none
        table.dataSource = self
        table.delegate = self
        table.target = self
        table.action = #selector(rowClicked)
        table.onReturn = { [weak self] in self?.rowClicked() }
        table.setAccessibilityLabel("Files")
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.scrollerStyle = .overlay
        scroll.drawsBackground = false
        panel.addSubview(scroll)
        layout()
    }

    func layout() {
        let w = panel.bounds.width - 1
        let h = panel.bounds.height
        modes.frame = NSRect(x: 14, y: Self.topInset + 2, width: w - 28, height: 22)
        let headerY = Self.topInset + 34
        let showHeader = mode == .folder && !folderStack.isEmpty
        header.isHidden = !showHeader
        refreshButton.isHidden = !showHeader
        header.frame = NSRect(x: 16, y: headerY, width: w - 52, height: 16)
        refreshButton.frame = NSRect(x: w - 32, y: headerY - 2, width: 20, height: 20)
        let listY = showHeader ? headerY + 22 : Self.topInset + 34
        scroll.frame = NSRect(x: 0, y: listY, width: w, height: max(0, h - listY))
        table.tableColumns.first?.width = w
        message.frame = NSRect(x: 20, y: listY + 24, width: w - 40, height: 60)
        openFolderButton.sizeToFit()
        let bw = openFolderButton.frame.width
        openFolderButton.frame.origin = NSPoint(x: (w - bw) / 2, y: listY + 90)
    }

    // MARK: Content

    func reload() {
        listingGeneration += 1
        switch mode {
        case .recent: showRecent()
        case .folder: showFolder()
        }
        layout()
    }

    /// Re-marks the open file without refetching anything.
    func currentDocumentChanged() {
        if mode == .recent { showRecent() } else { markCurrent() }
    }

    private func showRecent() {
        items = library.recentDrafts(limit: 100).map { s in
            let subtitle: String
            if let remote = s.remote { subtitle = remote.display } else { subtitle = (s.url.deletingLastPathComponent().path as NSString).abbreviatingWithTildeInPath }
            return SidebarItem(kind: .file, title: s.title, subtitle: subtitle, url: s.url, remote: s.remote)
        }
        setMessage(items.isEmpty ? "Files you open show up here." : nil)
        openFolderButton.isHidden = true
        table.reloadData()
        markCurrent()
    }

    private func showFolder() {
        guard let dir = folderStack.last else {
            items = []
            table.reloadData()
            header.stringValue = ""
            setMessage("Open a folder to see its files here.")
            openFolderButton.isHidden = false
            return
        }
        openFolderButton.isHidden = true
        header.stringValue = folderStack.count > 1 ? folderStack.map(\.name).joined(separator: " › ") : dir.name
        header.toolTip = dir.detail
        switch dir {
        case let .local(url):
            apply(Self.localEntries(in: url), in: dir)
        case let .remote(loc):
            items = parentItem()
            table.reloadData()
            setMessage("Loading \(loc.hostName)…")
            let generation = listingGeneration
            let ssh = self.ssh
            DispatchQueue.global(qos: .userInitiated).async {
                let result = Result { try ssh.list(loc) }
                DispatchQueue.main.async { [weak self] in
                    guard let self, generation == self.listingGeneration else { return }
                    switch result {
                    case let .success(entries): self.apply(.success(entries), in: dir)
                    case let .failure(error): self.apply(.failure(error), in: dir)
                    }
                }
            }
        }
    }

    static func localEntries(in url: URL) -> Result<[RemoteEntry], Error> {
        Result {
            let urls = try FileManager.default.contentsOfDirectory(at: url, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles])
            return urls.map { RemoteEntry(name: $0.lastPathComponent, isDirectory: (try? $0.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true) }
        }
    }

    private func apply(_ result: Result<[RemoteEntry], Error>, in dir: FolderRoot) {
        switch result {
        case let .failure(error):
            items = parentItem()
            setMessage(error.localizedDescription)
        case let .success(entries):
            let visible = entries
                .filter { $0.isDirectory || DraftLibrary.fileExtensions.contains(($0.name as NSString).pathExtension.lowercased()) }
                .sorted { a, b in a.isDirectory != b.isDirectory ? a.isDirectory : a.name.localizedStandardCompare(b.name) == .orderedAscending }
            items = parentItem() + visible.map { e in
                switch dir {
                case let .local(url):
                    let child = url.appendingPathComponent(e.name, isDirectory: e.isDirectory).standardizedFileURL
                    return SidebarItem(kind: e.isDirectory ? .directory : .file, title: e.name, url: child)
                case let .remote(loc):
                    return SidebarItem(kind: e.isDirectory ? .directory : .file, title: e.name, remote: loc.appending(e.name))
                }
            }
            setMessage(visible.isEmpty ? "No Markdown or text files here." : nil)
        }
        table.reloadData()
        markCurrent()
    }

    private func parentItem() -> [SidebarItem] {
        folderStack.count > 1 ? [SidebarItem(kind: .parent, title: "..", subtitle: nil)] : []
    }

    private func setMessage(_ text: String?) {
        message.stringValue = text ?? ""
        message.isHidden = text == nil
    }

    private func isCurrent(_ item: SidebarItem) -> Bool {
        guard item.kind == .file, let editor else { return false }
        if let remote = item.remote { return editor.draft.remote == remote }
        return item.url != nil && item.url == editor.draft.url
    }

    private func markCurrent() {
        for row in 0..<items.count {
            (table.rowView(atRow: row, makeIfNecessary: false) as? SidebarRowView)?.isCurrent = isCurrent(items[row])
        }
        table.deselectAll(nil)
    }

    // MARK: Actions

    func setFolder(_ root: FolderRoot) {
        folderStack = [root]
        Preferences.folderRoot = root
        setMode(.folder)
    }

    func setMode(_ m: Mode) {
        mode = m
        Preferences.sidebarMode = m
        modes.selectedSegment = m == .recent ? 0 : 1
        reload()
    }

    @objc private func modeChanged() { setMode(modes.selectedSegment == 0 ? .recent : .folder) }
    @objc private func refreshClicked() { reload() }
    @objc private func openFolderClicked() { editor?.openFolder(nil) }

    @objc private func rowClicked() {
        let row = table.clickedRow >= 0 ? table.clickedRow : table.selectedRow
        guard row >= 0, row < items.count else { return }
        activate(items[row])
    }

    private func activate(_ item: SidebarItem) {
        switch item.kind {
        case .parent:
            folderStack.removeLast()
            reload()
        case .directory:
            if let url = item.url { folderStack.append(.local(url)) }
            if let remote = item.remote { folderStack.append(.remote(remote)) }
            reload()
        case .file:
            table.deselectAll(nil)
            if let remote = item.remote { editor?.openRemote(remote) } else if let url = item.url { editor?.open(url) }
        }
    }

    // MARK: Table

    func numberOfRows(in tableView: NSTableView) -> Int { items.count }

    func tableView(_ tableView: NSTableView, rowViewForRow row: Int) -> NSTableRowView? {
        let view = SidebarRowView()
        view.isCurrent = isCurrent(items[row])
        return view
    }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        items[row].subtitle == nil ? 28 : 42
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let item = items[row]
        let cell = NSTableCellView()
        let icon = NSImageView()
        let symbol: String
        switch item.kind {
        case .parent: symbol = "arrow.turn.left.up"
        case .directory: symbol = "folder"
        case .file: symbol = item.remote != nil ? "network" : "doc.text"
        }
        icon.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
        icon.contentTintColor = Theme.quiet
        icon.symbolConfiguration = .init(pointSize: 11, weight: .regular)
        let title = NSTextField(labelWithString: item.title)
        title.font = Theme.uiFont(size: 13)
        title.textColor = isCurrent(item) ? Theme.text : Theme.text.withAlphaComponent(0.8)
        title.lineBreakMode = .byTruncatingTail
        let w = tableView.bounds.width
        if let subtitle = item.subtitle {
            icon.frame = NSRect(x: 18, y: 22, width: 14, height: 14)
            title.frame = NSRect(x: 38, y: 21, width: w - 56, height: 17)
            let sub = NSTextField(labelWithString: subtitle)
            sub.font = Theme.uiFont(size: 11)
            sub.textColor = Theme.quiet
            sub.lineBreakMode = .byTruncatingMiddle
            sub.frame = NSRect(x: 38, y: 4, width: w - 56, height: 15)
            cell.addSubview(sub)
        } else {
            icon.frame = NSRect(x: 18, y: 7, width: 14, height: 14)
            title.frame = NSRect(x: 38, y: 5, width: w - 56, height: 17)
        }
        cell.addSubview(icon)
        cell.addSubview(title)
        cell.setAccessibilityLabel([item.title, item.subtitle].compactMap { $0 }.joined(separator: ", "))
        return cell
    }

    func tableView(_ tableView: NSTableView, shouldTypeSelectFor event: NSEvent, withCurrentSearch searchString: String?) -> Bool { true }

    func tableView(_ tableView: NSTableView, typeSelectStringFor tableColumn: NSTableColumn?, row: Int) -> String? { items[row].title }
}
