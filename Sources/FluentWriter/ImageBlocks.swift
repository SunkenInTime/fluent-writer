import AppKit
import ImageIO
import WriterCore

/// The framed preview drawn under an image line. The Markdown stays in the text; this view only
/// occupies the space the line's paragraph spacing leaves for it.
final class ImageBlockView: NSView {
    enum Content: Equatable {
        case image(NSImage)
        case placeholder(symbol: String, message: String)
    }

    var content: Content = .placeholder(symbol: "photo", message: "") { didSet { needsDisplay = true } }
    var url: URL?
    /// Location of the image's Markdown line, for putting the caret back on it.
    var lineRange = NSRange(location: 0, length: 0)
    weak var editor: EditorController?

    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        let r = bounds.insetBy(dx: 0.5, dy: 0.5)
        let path = NSBezierPath(roundedRect: r, xRadius: 6, yRadius: 6)
        switch content {
        case let .image(img):
            NSGraphicsContext.saveGraphicsState()
            path.addClip()
            img.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.high.rawValue])
            NSGraphicsContext.restoreGraphicsState()
            Theme.hairline.setStroke()
            path.lineWidth = 1
            path.stroke()
        case let .placeholder(symbol, message):
            Theme.codeBackground.setFill()
            path.fill()
            let ink = Theme.text.withAlphaComponent(0.62)
            let attrs: [NSAttributedString.Key: Any] = [.font: Theme.uiFont(size: 11), .foregroundColor: ink]
            let text = message as NSString
            let size = text.size(withAttributes: attrs)
            var x: CGFloat = 12
            if let icon = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?
                .withSymbolConfiguration(.init(pointSize: 12, weight: .regular).applying(.init(paletteColors: [ink]))) {
                let s = icon.size
                icon.draw(in: NSRect(x: x, y: (bounds.height - s.height) / 2, width: s.width, height: s.height), from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
                x += s.width + 8
            }
            text.draw(with: NSRect(x: x, y: (bounds.height - size.height) / 2, width: bounds.width - x - 12, height: size.height),
                      options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine], attributes: attrs)
        }
    }

    override func mouseDown(with event: NSEvent) {
        if event.clickCount >= 2, let url { NSWorkspace.shared.open(url); return }
        guard let editor else { return }
        editor.window?.makeFirstResponder(editor.textView)
        editor.textView.setSelectedRange(NSRange(location: NSMaxRange(lineRange), length: 0))
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        guard let url else { return nil }
        let menu = NSMenu()
        let open = NSMenuItem(title: "Open Image", action: #selector(openImage), keyEquivalent: "")
        open.target = self
        menu.addItem(open)
        if url.isFileURL {
            let reveal = NSMenuItem(title: "Show in Finder", action: #selector(revealImage), keyEquivalent: "")
            reveal.target = self
            menu.addItem(reveal)
        }
        if case .image = content {
            let copy = NSMenuItem(title: "Copy Image", action: #selector(copyImage), keyEquivalent: "")
            copy.target = self
            menu.addItem(copy)
        }
        return menu
    }

    @objc private func openImage() { if let url { NSWorkspace.shared.open(url) } }
    @objc private func revealImage() { if let url { NSWorkspace.shared.activateFileViewerSelecting([url]) } }
    @objc private func copyImage() {
        let pb = NSPasteboard.general
        pb.clearContents()
        if let url, url.isFileURL, let full = NSImage(contentsOf: url) { pb.writeObjects([full]) }
        else if case let .image(img) = content { pb.writeObjects([img]) }
    }

    override func isAccessibilityElement() -> Bool { true }
    override func accessibilityRole() -> NSAccessibility.Role? { .image }
}

/// Finds image lines, reserves room under each with paragraph spacing, and keeps a preview view
/// in that room as the text reflows.
final class ImageBlockController {
    weak var editor: EditorController?

    private struct Block {
        var image: MarkdownImage
        var url: URL?
        var size: NSSize
        var content: ImageBlockView.Content
    }

    private struct Cached {
        var image: NSImage
        var pointSize: NSSize
        var modified: Date?
        var maxPixel: CGFloat
    }

    private var blocks: [Block] = []
    private var views: [ImageBlockView] = []
    private var cache: [URL: Cached] = [:]
    private var failed: [URL: String] = [:]
    private var loading: Set<URL> = []
    private var dimRange: NSRange?

    init(editor: EditorController) {
        self.editor = editor
    }

    private var gap: CGFloat { ((editor?.styler.lineHeight ?? 28) * 0.45).rounded() }

    /// Widest an image may be: the prose column.
    private var columnWidth: CGFloat {
        guard let editor, let tc = editor.textView.textContainer else { return 600 }
        return max(80, tc.containerSize.width - editor.styler.hangWidth)
    }

    private var maxHeight: CGFloat {
        guard let editor else { return 520 }
        return max(160, min(560, editor.scrollView.contentSize.height * 0.7))
    }

    /// Re-reads image lines after the text was restyled, then lays the previews out.
    func update(lines: [MarkdownLine]) {
        guard let editor, let storage = editor.textView.textStorage else { return }
        let text = editor.textView.string as NSString
        let base = editor.draft.url?.deletingLastPathComponent() ?? editor.library.draftsDirectory
        let found = MarkdownImages.blocks(in: text, lines: lines)
        blocks = found.map { img in
            let url = editor.draft.remote == nil ? MarkdownImages.resolve(img.source, relativeTo: base) : nil
            let content = self.content(for: url, source: img.source, isRemoteDraft: editor.draft.remote != nil)
            return Block(image: img, url: url, size: displaySize(for: content), content: content)
        }
        guard !blocks.isEmpty || !views.isEmpty else { return }
        storage.beginEditing()
        for b in blocks {
            let line = b.image.lineRange
            let full = NSRange(location: line.location, length: min(line.length + 1, storage.length - line.location))
            let current = (storage.attribute(.paragraphStyle, at: line.location, effectiveRange: nil) as? NSParagraphStyle) ?? editor.styler.paragraphStyle()
            let p = current.mutableCopy() as! NSMutableParagraphStyle
            p.paragraphSpacing = b.size.height + gap * 2
            storage.addAttribute(.paragraphStyle, value: p, range: full)
            // The Markdown reads as a caption-like label rather than prose.
            storage.addAttribute(.foregroundColor, value: Theme.quiet, range: line)
        }
        storage.endEditing()
        layoutViews()
    }

    private func displaySize(for content: ImageBlockView.Content) -> NSSize {
        switch content {
        case .placeholder:
            return NSSize(width: min(columnWidth, 420), height: 40)
        case let .image(img):
            let natural = img.size.width > 0 && img.size.height > 0 ? img.size : NSSize(width: columnWidth, height: columnWidth * 0.6)
            var w = min(columnWidth, natural.width)
            var h = w * natural.height / natural.width
            if h > maxHeight { h = maxHeight; w = h * natural.width / natural.height }
            return NSSize(width: w.rounded(), height: h.rounded())
        }
    }

    /// Positions one preview per image line, reusing views.
    func layoutViews() {
        guard let editor, let lm = editor.textView.layoutManager, let tc = editor.textView.textContainer else { return }
        while views.count < blocks.count {
            let v = ImageBlockView()
            v.editor = editor
            editor.textView.addSubview(v)
            views.append(v)
        }
        while views.count > blocks.count { views.removeLast().removeFromSuperview() }
        lm.ensureLayout(for: tc)
        let origin = editor.textView.textContainerOrigin
        let storageLength = editor.textView.textStorage?.length ?? 0
        for (b, v) in zip(blocks, views) {
            let lastChar = max(b.image.lineRange.location, NSMaxRange(b.image.lineRange) - 1)
            guard lastChar < storageLength else { v.isHidden = true; continue }
            let glyph = lm.glyphIndexForCharacter(at: lastChar)
            let used = lm.lineFragmentUsedRect(forGlyphAt: glyph, effectiveRange: nil)
            v.frame = NSRect(x: origin.x + editor.styler.hangWidth, y: origin.y + used.maxY + gap, width: b.size.width, height: b.size.height)
            v.isHidden = false
            v.content = b.content
            v.url = b.url
            v.lineRange = b.image.lineRange
            v.toolTip = b.url.map { $0.isFileURL ? ($0.path as NSString).abbreviatingWithTildeInPath : $0.absoluteString }
            v.setAccessibilityLabel(b.image.alt.isEmpty ? "Image" : b.image.alt)
        }
        applyDimming(dimRange)
    }

    /// Fades previews outside the focused sentence or paragraph, like the text around them.
    func applyDimming(_ focus: NSRange?) {
        dimRange = focus
        for (b, v) in zip(blocks, views) {
            let inside = focus.map { NSIntersectionRange($0, b.image.lineRange).length > 0 || NSLocationInRange(b.image.lineRange.location, $0) } ?? true
            v.alphaValue = inside ? 1 : 0.28
        }
    }

    // MARK: Loading

    private func content(for url: URL?, source: String, isRemoteDraft: Bool) -> ImageBlockView.Content {
        if isRemoteDraft { return .placeholder(symbol: "photo", message: "Images aren't shown for files opened over SSH") }
        guard let url else { return .placeholder(symbol: "questionmark.square.dashed", message: "Can't read image address “\(source)”") }
        if url.isFileURL {
            let modified = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date
            guard modified != nil else { return .placeholder(symbol: "photo.badge.exclamationmark", message: "Image not found · \(source)") }
            let wantPixels = columnWidth * (editor?.window?.backingScaleFactor ?? 2)
            if let c = cache[url], c.modified == modified, c.maxPixel >= wantPixels * 0.9 { return .image(c.image) }
            guard let loaded = Self.loadLocal(url, maxPixel: wantPixels) else {
                return .placeholder(symbol: "photo.badge.exclamationmark", message: "Can't open \(url.lastPathComponent)")
            }
            cache[url] = Cached(image: loaded.image, pointSize: loaded.points, modified: modified, maxPixel: wantPixels)
            return .image(loaded.image)
        }
        if let c = cache[url] { return .image(c.image) }
        if let message = failed[url] { return .placeholder(symbol: "wifi.exclamationmark", message: message) }
        fetch(url)
        return .placeholder(symbol: "photo", message: "Loading \(url.host ?? "image")…")
    }

    /// A downsampled image sized in points (screenshots taken at 2× keep their on-screen size).
    static func loadLocal(_ url: URL, maxPixel: CGFloat) -> (image: NSImage, points: NSSize)? {
        if let src = CGImageSourceCreateWithURL(url as CFURL, nil), CGImageSourceGetCount(src) > 0 {
            let props = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any]
            let pw = (props?[kCGImagePropertyPixelWidth] as? CGFloat) ?? 0
            let ph = (props?[kCGImagePropertyPixelHeight] as? CGFloat) ?? 0
            let dpi = (props?[kCGImagePropertyDPIWidth] as? CGFloat).flatMap { $0 > 0 ? $0 : nil } ?? 72
            let opts: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: max(64, min(max(pw, ph), maxPixel)),
            ]
            if pw > 0, ph > 0, let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) {
                // Orientation transforms can swap the axes.
                let swapped = (cg.width > cg.height) != (pw > ph)
                let points = NSSize(width: (swapped ? ph : pw) * 72 / dpi, height: (swapped ? pw : ph) * 72 / dpi)
                return (NSImage(cgImage: cg, size: points), points)
            }
        }
        // SVG and anything else ImageIO can't read.
        guard let img = NSImage(contentsOf: url), img.size.width > 0 else { return nil }
        return (img, img.size)
    }

    private func fetch(_ url: URL) {
        guard !loading.contains(url) else { return }
        loading.insert(url)
        URLSession.shared.dataTask(with: url) { [weak self] data, _, error in
            let image = data.flatMap { NSImage(data: $0) }
            DispatchQueue.main.async {
                guard let self else { return }
                self.loading.remove(url)
                if let image, image.size.width > 0 {
                    self.cache[url] = Cached(image: image, pointSize: image.size, modified: nil, maxPixel: .greatestFiniteMagnitude)
                } else {
                    self.failed[url] = error == nil ? "Not an image · \(url.host ?? "")" : "Couldn't load image from \(url.host ?? "the web")"
                }
                self.editor?.refreshImages()
            }
        }.resume()
    }
}

// MARK: Adding images

extension EditorController {
    func refreshImages() {
        images.update(lines: lines)
    }

    /// Handles pasted or dropped images. Returns false when the pasteboard holds no images, so the
    /// normal text paste or drop runs instead.
    func insertImages(from pb: NSPasteboard, at location: Int?) -> Bool {
        let fileURLs = (pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? [])
            .filter { MarkdownImages.imageExtensions.contains($0.pathExtension.lowercased()) }
        var data: Data?
        if fileURLs.isEmpty {
            // Text wins when there is any, so pasting from a web page still pastes its words.
            if let s = pb.string(forType: .string), !s.isEmpty { return false }
            if let png = pb.data(forType: .png) { data = png }
            else if let tiff = pb.data(forType: .tiff), let rep = NSBitmapImageRep(data: tiff) { data = rep.representation(using: .png, properties: [:]) }
        }
        guard !fileURLs.isEmpty || data != nil else { return false }
        if preview.isActive { NSSound.beep(); return true }
        guard draft.remote == nil else {
            flashMessage("Images can't be added to files opened over SSH yet")
            return true
        }

        let base = draft.url?.deletingLastPathComponent() ?? library.draftsDirectory
        let folder = base.appendingPathComponent("images", isDirectory: true)
        var snippets: [String] = []
        var copied = 0
        do {
            for url in fileURLs {
                if let rel = MarkdownImages.relativePath(of: url, inside: base) {
                    snippets.append(MarkdownImages.markdown(alt: MarkdownImages.altText(forFileName: url.lastPathComponent), path: rel))
                    continue
                }
                try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                let dest = MarkdownImages.uniqueFileURL(named: url.lastPathComponent, in: folder)
                try FileManager.default.copyItem(at: url, to: dest)
                copied += 1
                snippets.append(MarkdownImages.markdown(alt: MarkdownImages.altText(forFileName: url.lastPathComponent), path: "images/" + dest.lastPathComponent))
            }
            if let data {
                try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                let f = DateFormatter()
                f.dateFormat = "yyyy-MM-dd 'at' HH.mm.ss"
                let dest = MarkdownImages.uniqueFileURL(named: "Pasted image \(f.string(from: Date())).png", in: folder)
                try data.write(to: dest, options: .atomic)
                copied += 1
                snippets.append(MarkdownImages.markdown(alt: "", path: "images/" + dest.lastPathComponent))
            }
        } catch {
            flashMessage("Couldn't save the image: \(error.localizedDescription)")
            if snippets.isEmpty { return true }
        }

        let ns = textView.string as NSString
        let range = location.map { NSRange(location: min($0, ns.length), length: 0) } ?? textView.selectedRange()
        let before = range.location > 0 ? ns.character(at: range.location - 1) : 0x0A
        let after = NSMaxRange(range) < ns.length ? ns.character(at: NSMaxRange(range)) : 0x0A
        let insertion = (before == 0x0A ? "" : "\n\n") + snippets.joined(separator: "\n\n") + (after == 0x0A ? "" : "\n\n")
        let caret = range.location + (insertion as NSString).length
        apply(TextEdit(range: range, replacement: insertion, selectionAfter: NSRange(location: caret, length: 0)),
              actionName: snippets.count == 1 ? "Insert Image" : "Insert Images")
        window?.makeFirstResponder(textView)
        let what = snippets.count == 1 ? "Image added" : "\(snippets.count) images added"
        flashMessage(copied > 0 ? "\(what) · copied to the images folder beside this draft" : "\(what) · linked where it already is")
        return true
    }

    @objc func insertImageFromFile(_ sender: Any?) {
        guard let window else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = MarkdownImages.imageExtensions.compactMap { .init(filenameExtension: $0) }
        panel.allowsMultipleSelection = true
        panel.message = "Images are copied into an images folder next to this draft."
        panel.beginSheetModal(for: window) { [weak self] response in
            guard response == .OK, let self else { return }
            let pb = NSPasteboard(name: .init("fluent-writer-insert-image"))
            pb.clearContents()
            pb.writeObjects(panel.urls as [NSURL])
            _ = self.insertImages(from: pb, at: nil)
        }
    }
}
