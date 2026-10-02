import Foundation

public enum TitleDeriver {
    public static let untitled = "Untitled"

    /// A local title derived from the first nonempty line of the body.
    public static func derivedTitle(from body: String, maxLength: Int = 60) -> String {
        for raw in body.split(separator: "\n", omittingEmptySubsequences: true) {
            let line = MarkdownParser.parseLine(String(raw) as NSString)
            if line.kind == .codeFence || line.kind == .horizontalRule || line.kind == .blank { continue }
            let rendered = PlainTextExporter.renderInlineFragment((String(raw) as NSString).substring(with: line.contentRange))
            let collapsed = rendered.split(whereSeparator: { $0.isWhitespace }).joined(separator: " ")
            if collapsed.isEmpty { continue }
            return truncate(collapsed, maxLength: maxLength)
        }
        return untitled
    }

    static func truncate(_ s: String, maxLength: Int) -> String {
        guard s.count > maxLength else { return s }
        let cut = s.prefix(maxLength)
        if let space = cut.lastIndex(of: " "), cut.distance(from: cut.startIndex, to: space) > maxLength / 2 {
            return String(cut[..<space]).trimmingCharacters(in: CharacterSet(charactersIn: " ,;:-"))
        }
        return String(cut)
    }

    /// A string safe to use as a file name on macOS.
    public static func fileNameStem(for title: String) -> String {
        var s = title.replacingOccurrences(of: "/", with: "-").replacingOccurrences(of: ":", with: "-")
        s = s.components(separatedBy: CharacterSet.controlCharacters).joined()
        s = s.trimmingCharacters(in: CharacterSet.whitespaces.union(CharacterSet(charactersIn: ".")))
        if s.isEmpty { s = untitled }
        if s.utf8.count > 200 { s = String(s.prefix(120)) }
        return s
    }
}
