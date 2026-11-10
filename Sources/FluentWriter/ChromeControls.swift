import AppKit
import WriterCore

/// Quiet borderless icon button: grey at rest, a soft rounded plate on hover, tinted while its panel is open.
final class ChromeButton: NSButton {
    var isActive = false { didSet { refresh() } }
    private var hovering = false { didSet { refresh() } }
    private var tracking: NSTrackingArea?
    private let badge = NSTextField(labelWithString: "")
    static let slot = NSSize(width: 28, height: 24)

    convenience init(symbol: String, label: String, tip: String) {
        self.init(frame: .zero)
        image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)?
            .withSymbolConfiguration(.init(pointSize: 13, weight: .regular))
        imagePosition = .imageOnly
        isBordered = false
        title = ""
        badge.font = Theme.uiFont(size: 9, weight: .medium)
        badge.alignment = .center
        badge.isHidden = true
        addSubview(badge)
        toolTip = tip
        setAccessibilityLabel(label)
        wantsLayer = true
        layer?.cornerRadius = 6
        refresh()
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let t = NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect], owner: self)
        addTrackingArea(t)
        tracking = t
    }

    override func mouseEntered(with event: NSEvent) { hovering = true }
    override func mouseExited(with event: NSEvent) { hovering = false }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        refresh()
    }

    private func refresh() {
        contentTintColor = isActive ? Theme.caret : (hovering ? Theme.text : Theme.icon)
        badge.textColor = isActive ? Theme.caret : Theme.text
        let plate = isActive || hovering ? Theme.sidebarSelection.withAlphaComponent(isActive ? 0.9 : 0.7) : .clear
        effectiveAppearance.performAsCurrentDrawingAppearance { layer?.backgroundColor = plate.cgColor }
    }

    override func layout() {
        super.layout()
        let w = max(12, ceil(badge.attributedStringValue.size().width) + 2)
        badge.frame = NSRect(x: bounds.width - w - 1, y: isFlipped ? 0 : bounds.height - 11, width: w, height: 11)
    }

    /// A small count in the corner, or nothing.
    func setCount(_ n: Int) {
        badge.stringValue = n > 99 ? "99+" : "\(n)"
        badge.isHidden = n == 0
        setAccessibilityValue(n > 0 ? "\(n)" : nil)
        needsLayout = true
        refresh()
    }
}

/// Buttons on the right of the title bar for comments, versions and Assist.
final class EditorToolbar {
    let addComment = ChromeButton(symbol: "plus.bubble", label: "Add comment", tip: "Comment on the selection or this line (⌥⌘M)")
    let comments = ChromeButton(symbol: "text.bubble", label: "Comments", tip: "Comments (⇧⌘M)")
    let versions = ChromeButton(symbol: "clock.arrow.circlepath", label: "Versions", tip: "Versions (⌥⌘V)")
    let assist = ChromeButton(symbol: "sparkles", label: "Assist", tip: "Assist (⌥⌘J)")

    var all: [ChromeButton] { [addComment, comments, versions, assist] }
}

extension EditorController {
    // MARK: Toolbar

    func buildToolbar() {
        toolbar.addComment.target = self
        toolbar.addComment.action = #selector(addComment(_:))
        toolbar.comments.target = self
        toolbar.comments.action = #selector(toggleCommentsPanel(_:))
        toolbar.versions.target = self
        toolbar.versions.action = #selector(toggleVersionsPanel(_:))
        toolbar.assist.target = self
        toolbar.assist.action = #selector(toggleAskPanel(_:))
        toolbar.all.forEach { canvas.addSubview($0) }

        sidebarButton.target = self
        sidebarButton.action = #selector(toggleFileSidebar(_:))
        canvas.addSubview(sidebarButton)
    }

    /// Pinned to the window's top-right corner so they never move when panels open, right to left:
    /// Assist, Versions, Comments, then Add Comment a little apart.
    func layoutToolbar() {
        let slot = ChromeButton.slot
        let midY = sidebarButton.frame.midY
        var x = canvas.bounds.width - 12
        for (i, b) in [toolbar.assist, toolbar.versions, toolbar.comments, toolbar.addComment].enumerated() {
            x -= slot.width
            b.frame = NSRect(x: x, y: (midY - slot.height / 2).rounded(), width: slot.width, height: slot.height)
            x -= i == 2 ? 10 : 2
        }
        toolbar.all.forEach { if canvas.subviews.last !== $0 { canvas.addSubview($0, positioned: .above, relativeTo: nil) } }
        // Keep the centered title clear of whatever part of the buttons sits over the editor's title bar.
        let w = topBar.bounds.width
        let overlap = max(0, topBar.frame.maxX - (x + 8))
        let leftReserve = max(0, sidebarButton.frame.maxX + 12 - topBar.frame.minX)
        let reserve = max(overlap, leftReserve, 24)
        let titleW = max(0, min(420, w - 2 * reserve))
        titleField.frame = NSRect(x: (w - titleW) / 2, y: (midY - 9).rounded(), width: titleW, height: 18)
        titleField.isHidden = titleW < 60
    }

    /// Next to the traffic lights, vertically centered on them; at the left edge in full screen.
    func layoutSidebarButton() {
        var frame = NSRect(origin: NSPoint(x: 12, y: 8), size: ChromeButton.slot)
        if window?.styleMask.contains(.fullScreen) != true,
           let zoom = window?.standardWindowButton(.zoomButton), let host = zoom.superview {
            let r = canvas.convert(host.convert(zoom.frame, to: nil), from: nil)
            frame.origin = NSPoint(x: r.maxX + 12, y: (r.midY - frame.height / 2).rounded())
        }
        sidebarButton.frame = frame
        // Stay above the sidebar panel, which is added and removed beneath it.
        if sidebarButton.superview === canvas, canvas.subviews.last !== sidebarButton {
            canvas.addSubview(sidebarButton, positioned: .above, relativeTo: nil)
        }
    }

    func updateToolbarState() {
        let open = notes?.openComments.count ?? 0
        toolbar.comments.setCount(open)
        toolbar.versions.setCount(0)
        let tab = assistVisible ? assist?.tab : nil
        toolbar.comments.isActive = tab == .comments
        toolbar.versions.isActive = tab == .versions || preview?.isActive == true
        toolbar.assist.isActive = tab == .ask
        sidebarButton.isActive = sidebarVisible
        layoutToolbar()
    }

    /// Opening the side panel in a narrow window widens the window so the draft keeps a usable column.
    func makeRoomForPanel() {
        guard let window, !window.styleMask.contains(.fullScreen), let screen = window.screen else { return }
        let needed: CGFloat = 720 + (sidebarVisible ? SidebarController.width : 0)
        var f = window.frame
        guard f.width < needed else { return }
        let visible = screen.visibleFrame
        f.size.width = min(needed, visible.width)
        if f.maxX > visible.maxX { f.origin.x = max(visible.minX, visible.maxX - f.width) }
        window.setFrame(f, display: true, animate: !reduceMotion)
    }

    private func togglePanel(_ tab: AssistController.Tab) {
        if assistVisible, assist?.tab == tab { assist?.close() } else { ensureAssist().show(provider: nil, tab: tab) }
        updateToolbarState()
    }

    @objc func toggleCommentsPanel(_ sender: Any?) { togglePanel(.comments) }
    @objc func toggleVersionsPanel(_ sender: Any?) { togglePanel(.versions) }
    @objc func toggleAskPanel(_ sender: Any?) { togglePanel(.ask) }

    // MARK: Sidebar peek

    /// Pointer at the window's left edge slides the hidden sidebar in over the text, like Arc.
    func trackSidebarPeek() {
        guard let window, window.isKeyWindow, NSEvent.pressedMouseButtons == 0 else { return }
        let mouse = canvas.convert(window.mouseLocationOutsideOfEventStream, from: nil)
        if sidebarPeeking {
            if mouse.x > SidebarController.width + 36 {
                if peekTimer == nil {
                    peekTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: false) { [weak self] _ in
                        self?.peekTimer = nil
                        self?.endSidebarPeek()
                    }
                }
            } else {
                peekTimer?.invalidate()
                peekTimer = nil
            }
            return
        }
        guard !sidebarVisible, mouse.x <= 4, mouse.y > 0, mouse.y < canvas.bounds.height else {
            if !sidebarVisible { peekTimer?.invalidate(); peekTimer = nil }
            return
        }
        guard peekTimer == nil else { return }
        peekTimer = Timer.scheduledTimer(withTimeInterval: 0.12, repeats: false) { [weak self] _ in
            guard let self else { return }
            self.peekTimer = nil
            let now = self.canvas.convert(self.window?.mouseLocationOutsideOfEventStream ?? .zero, from: nil)
            if now.x <= 10 { self.beginSidebarPeek() }
        }
    }

    func beginSidebarPeek() {
        guard !sidebarVisible, !sidebarPeeking else { return }
        sidebarPeeking = true
        let w = SidebarController.width
        let h = canvas.bounds.height
        let panel = sidebar.panel
        panel.wantsLayer = true
        panel.layer?.masksToBounds = false
        panel.layer?.shadowColor = NSColor.black.cgColor
        panel.layer?.shadowOpacity = 0.22
        panel.layer?.shadowRadius = 18
        panel.layer?.shadowOffset = CGSize(width: 4, height: 0)
        panel.frame = NSRect(x: -w, y: 0, width: w, height: h)
        canvas.addSubview(panel, positioned: .above, relativeTo: nil)
        sidebar.reload()
        sidebar.layout()
        layoutSidebarButton()
        showChrome(autoHide: false)
        let target = NSRect(x: 0, y: 0, width: w, height: h)
        if reduceMotion { panel.frame = target; return }
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.18
            ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
            panel.animator().frame = target
        }
    }

    func endSidebarPeek() {
        guard sidebarPeeking else { return }
        let panel = sidebar.panel
        let w = SidebarController.width
        let finish: () -> Void = { [weak self] in
            guard let self, self.sidebarPeeking else { return }
            self.sidebarPeeking = false
            panel.removeFromSuperview()
            panel.layer?.shadowOpacity = 0
            self.updateToolbarState()
            self.showChrome(autoHide: true)
        }
        if reduceMotion { finish(); return }
        NSAnimationContext.runAnimationGroup({ ctx in
            ctx.duration = 0.16
            ctx.timingFunction = CAMediaTimingFunction(name: .easeIn)
            panel.animator().frame = NSRect(x: -w, y: 0, width: w, height: panel.frame.height)
        }, completionHandler: finish)
    }

    /// Clicking the toggle while the sidebar peeks keeps it open.
    func pinPeekingSidebar() {
        guard sidebarPeeking else { return }
        peekTimer?.invalidate()
        peekTimer = nil
        sidebarPeeking = false
        sidebar.panel.layer?.shadowOpacity = 0
        Preferences.showSidebar = true
        frameChanged()
        updateToolbarState()
    }
}
