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

    static var showSidebar: Bool {
        get { d.bool(forKey: "showSidebar") }
        set { d.set(newValue, forKey: "showSidebar") }
    }

    static var sidebarMode: SidebarController.Mode {
        get { SidebarController.Mode(rawValue: d.string(forKey: "sidebarMode") ?? "") ?? .recent }
        set { d.set(newValue.rawValue, forKey: "sidebarMode") }
    }

    static var folderRoot: FolderRoot? {
        get {
            if let path = d.string(forKey: "folderLocal") { return .local(URL(fileURLWithPath: path, isDirectory: true)) }
            if let raw = d.string(forKey: "folderRemote"), let loc = RemoteLocation.parse(raw) { return .remote(loc) }
            return nil
        }
        set {
            d.removeObject(forKey: "folderLocal")
            d.removeObject(forKey: "folderRemote")
            switch newValue {
            case let .local(url): d.set(url.path, forKey: "folderLocal")
            case let .remote(loc): d.set(loc.display, forKey: "folderRemote")
            case nil: break
            }
        }
    }

    static var lastSSHLocation: String {
        get { d.string(forKey: "lastSSHLocation") ?? "" }
        set { d.set(newValue, forKey: "lastSSHLocation") }
    }
}
