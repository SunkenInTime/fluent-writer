import AppKit
import CoreText

enum Appearance: String, CaseIterable {
    case system, light, dark

    var nsAppearance: NSAppearance? {
        switch self {
        case .system: return nil
        case .light: return NSAppearance(named: .aqua)
        case .dark: return NSAppearance(named: .darkAqua)
        }
    }

    var label: String {
        switch self {
        case .system: return "Match System"
        case .light: return "Light"
        case .dark: return "Dark"
        }
    }
}

enum Theme {
    static func dynamic(light: NSColor, dark: NSColor) -> NSColor {
        NSColor(name: nil) { appearance in
            appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
        }
    }

    static func rgb(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1) -> NSColor {
        NSColor(srgbRed: r / 255, green: g / 255, blue: b / 255, alpha: a)
    }

    static let canvas = dynamic(light: rgb(247, 247, 247), dark: rgb(26, 26, 26))
    static let text = dynamic(light: rgb(25, 25, 25), dark: rgb(218, 218, 216))
    static let dimmed = dynamic(light: rgb(199, 196, 194), dark: rgb(78, 78, 76))
    static let markup = dynamic(light: rgb(172, 170, 168), dark: rgb(112, 112, 110))
    static let quiet = dynamic(light: rgb(150, 148, 146), dark: rgb(120, 120, 118))
    static let caret = dynamic(light: rgb(0, 190, 255), dark: rgb(0, 175, 245))
    static let selection = dynamic(light: rgb(178, 228, 250), dark: rgb(22, 78, 104))
    static let hairline = dynamic(light: rgb(228, 228, 226), dark: rgb(44, 44, 44))
    static let codeBackground = dynamic(light: rgb(236, 236, 234), dark: rgb(38, 38, 38))
    static let warning = dynamic(light: rgb(196, 54, 36), dark: rgb(240, 110, 90))
    static let inserted = dynamic(light: rgb(30, 130, 70), dark: rgb(110, 200, 140))
    static let insertedBackground = dynamic(light: rgb(30, 160, 80, 0.13), dark: rgb(90, 200, 120, 0.18))
    static let deleted = dynamic(light: rgb(180, 50, 40), dark: rgb(235, 120, 105))
    static let deletedBackground = dynamic(light: rgb(210, 60, 40, 0.11), dark: rgb(230, 90, 70, 0.17))

    static let defaultTextSize: CGFloat = 19
    static let textSizes: [CGFloat] = [14, 15, 16, 17, 18, 19, 20, 22, 24, 26, 28, 32]
    static let lineHeightMultiple: CGFloat = 1.55
    static let columnCharacters: CGFloat = 68
    static let minimumSidePadding: CGFloat = 36

    private static var registered = false

    static func registerFonts() {
        guard !registered else { return }
        registered = true
        guard let dir = fontsDirectory(),
              let urls = try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
        else { return }
        for url in urls where url.pathExtension == "otf" {
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        }
    }

    private static func fontsDirectory() -> URL? {
        let name = "FluentWriter_FluentWriter.bundle"
        var candidates: [URL] = []
        if let res = Bundle.main.resourceURL { candidates.append(res.appendingPathComponent(name)) }
        candidates.append(Bundle.main.bundleURL.appendingPathComponent(name))
        if let exe = Bundle.main.executableURL { candidates.append(exe.deletingLastPathComponent().appendingPathComponent(name)) }
        for c in candidates {
            for sub in ["Fonts", "Contents/Resources/Fonts"] {
                let dir = c.appendingPathComponent(sub)
                if FileManager.default.fileExists(atPath: dir.path) { return dir }
            }
        }
        return nil
    }

    static func font(size: CGFloat, bold: Bool = false, italic: Bool = false) -> NSFont {
        registerFonts()
        let name: String
        switch (bold, italic) {
        case (true, true): name = "IBMPlexMono-BoldItalic"
        case (true, false): name = "IBMPlexMono-Bold"
        case (false, true): name = "IBMPlexMono-Italic"
        case (false, false): name = "IBMPlexMono"
        }
        if let f = NSFont(name: name, size: size) { return f }
        var f = NSFont.monospacedSystemFont(ofSize: size, weight: bold ? .bold : .regular)
        if italic { f = NSFontManager.shared.convert(f, toHaveTrait: .italicFontMask) }
        return f
    }

    static func uiFont(size: CGFloat = 12, weight: NSFont.Weight = .regular) -> NSFont {
        registerFonts()
        let name = weight == .medium ? "IBMPlexMono-Medium" : "IBMPlexMono"
        return NSFont(name: name, size: size) ?? NSFont.monospacedSystemFont(ofSize: size, weight: weight)
    }

    static func characterWidth(for font: NSFont) -> CGFloat {
        ("0" as NSString).size(withAttributes: [.font: font]).width
    }
}
