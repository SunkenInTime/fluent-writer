import AppKit
import WriterCore

enum Preferences {
    private static let d = UserDefaults.standard

    static var focus: FocusUnit {
        get { FocusUnit(rawValue: d.string(forKey: "focus") ?? "") ?? .off }
        set { d.set(newValue.rawValue, forKey: "focus") }
    }

    /// The dimming mode ⌘D returns to when toggling focus back on.
    static var lastDimmingFocus: FocusUnit {
        get { FocusUnit(rawValue: d.string(forKey: "lastDimmingFocus") ?? "") ?? .sentence }
        set { d.set(newValue.rawValue, forKey: "lastDimmingFocus") }
    }

    static var textSize: CGFloat {
        get { let v = d.double(forKey: "textSize"); return v > 0 ? CGFloat(v) : Theme.defaultTextSize }
        set { d.set(Double(newValue), forKey: "textSize") }
    }

    static var appearance: Appearance {
        get { Appearance(rawValue: d.string(forKey: "appearance") ?? "") ?? .system }
        set { d.set(newValue.rawValue, forKey: "appearance") }
    }

    static var showCounts: Bool {
        get { d.object(forKey: "showCounts") as? Bool ?? true }
        set { d.set(newValue, forKey: "showCounts") }
    }

    static var spellCheck: Bool {
        get { d.object(forKey: "spellCheck") as? Bool ?? true }
        set { d.set(newValue, forKey: "spellCheck") }
    }
}
