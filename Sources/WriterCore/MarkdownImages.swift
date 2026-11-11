import Foundation

/// An image on a line of its own, `![alt](source "title")`, which the editor previews below the line.
public struct MarkdownImage: Equatable, Sendable {
    public var lineRange: NSRange
    public var alt: String
    public var source: String
}

public enum MarkdownImages {
    static let blockRegex = try! NSRegularExpression(
        pattern: #"^[ ]{0,3}!\[([^\]\n]*)\]\(\s*(<[^>\n]+>|[^\s)]+)(?:\s+(?:"[^"\n]*"|'[^'\n]*'))?\s*\)\s*$"#
    )

    public static let imageExtensions: Set<String> = ["png", "jpg", "jpeg", "gif", "heic", "heif", "webp", "tiff", "tif", "bmp", "svg"]

    /// Image lines among `lines`. Code blocks and fences are skipped.
    public static func blocks(in text: NSString, lines: [MarkdownLine]) -> [MarkdownImage] {
        var out: [MarkdownImage] = []
        for line in lines where line.kind == .paragraph {
            guard line.range.length >= 5, NSMaxRange(line.range) <= text.length else { continue }
            let s = text.substring(with: line.range)
            guard s.trimmingCharacters(in: .whitespaces).hasPrefix("![") else { continue }
            let ns = s as NSString
            guard let m = blockRegex.firstMatch(in: s, range: NSRange(location: 0, length: ns.length)) else { continue }
            var source = ns.substring(with: m.range(at: 2))
            if source.hasPrefix("<") && source.hasSuffix(">") { source = String(source.dropFirst().dropLast()) }
            out.append(MarkdownImage(lineRange: line.range, alt: ns.substring(with: m.range(at: 1)), source: source))
        }
        return out
    }

    /// Where `source` points: a web URL, a file URL, or a path relative to `base` (the draft's folder).
    public static func resolve(_ source: String, relativeTo base: URL?) -> URL? {
        if let url = URL(string: source), let scheme = url.scheme?.lowercased() {
            if scheme == "http" || scheme == "https" || scheme == "file" { return url }
            if scheme.count > 1 { return nil }
        }
        let decoded = source.removingPercentEncoding ?? source
        if decoded.hasPrefix("/") { return URL(fileURLWithPath: decoded).standardizedFileURL }
        if decoded.hasPrefix("~") { return URL(fileURLWithPath: (decoded as NSString).expandingTildeInPath).standardizedFileURL }
        guard let base else { return nil }
        return base.appendingPathComponent(decoded).standardizedFileURL
    }

    /// `file` relative to `directory` when it's inside it, else nil.
    public static func relativePath(of file: URL, inside directory: URL) -> String? {
        let dir = directory.standardizedFileURL.resolvingSymlinksInPath().path
        let path = file.standardizedFileURL.resolvingSymlinksInPath().path
        guard path.hasPrefix(dir + "/") else { return nil }
        return String(path.dropFirst(dir.count + 1))
    }

    /// The Markdown for an image, with the path escaped so spaces and parentheses survive.
    public static func markdown(alt: String, path: String) -> String {
        var allowed = CharacterSet.urlPathAllowed
        allowed.remove(charactersIn: "()")
        let encoded = path.addingPercentEncoding(withAllowedCharacters: allowed) ?? path
        let cleanAlt = alt.replacingOccurrences(of: "[", with: "").replacingOccurrences(of: "]", with: "").replacingOccurrences(of: "\n", with: " ")
        return "![\(cleanAlt)](\(encoded))"
    }

    /// A readable alt text from a file name: `team-offsite_2026.png` → `team offsite 2026`.
    public static func altText(forFileName name: String) -> String {
        let stem = (name as NSString).deletingPathExtension
        return stem.replacingOccurrences(of: "[-_]+", with: " ", options: .regularExpression).trimmingCharacters(in: .whitespaces)
    }

    /// A name in `directory` that doesn't exist yet: `photo.png`, `photo 2.png`, …
    public static func uniqueFileURL(named name: String, in directory: URL) -> URL {
        let ext = (name as NSString).pathExtension
        let stem = (name as NSString).deletingPathExtension
        var candidate = directory.appendingPathComponent(name)
        var n = 2
        while FileManager.default.fileExists(atPath: candidate.path) {
            candidate = directory.appendingPathComponent(ext.isEmpty ? "\(stem) \(n)" : "\(stem) \(n).\(ext)")
            n += 1
        }
        return candidate
    }

    /// The text to insert at `location` so the image sits on a line of its own.
    public static func blockInsertion(_ markdown: String, at location: Int, in text: NSString) -> String {
        let before = location > 0 ? text.character(at: location - 1) : 0x0A
        let after = location < text.length ? text.character(at: location) : 0x0A
        let lead = before == 0x0A ? "" : "\n\n"
        let trail = after == 0x0A ? "" : "\n\n"
        return lead + markdown + trail
    }
}
