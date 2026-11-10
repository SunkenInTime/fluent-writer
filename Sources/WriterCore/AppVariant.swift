import Foundation

/// Which copy of the app is running. The dev build keeps its own settings, drafts and support files,
/// so trying a new build never touches the copy you write with every day.
public enum AppVariant: String, Sendable {
    case standard
    case dev

    /// `FWVariant` in Info.plist picks the variant. Unbundled runs (`swift run`) count as dev.
    public static var current: AppVariant {
        if let raw = Bundle.main.object(forInfoDictionaryKey: "FWVariant") as? String, let v = AppVariant(rawValue: raw) { return v }
        return Bundle.main.bundleIdentifier == nil ? .dev : .standard
    }

    public var appName: String { self == .dev ? "Fluent Writer Dev" : "Fluent Writer" }
    var folderName: String { appName }

    public var supportDirectory: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(folderName, isDirectory: true)
    }

    public var draftsDirectory: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(folderName, isDirectory: true)
    }
}
