import Foundation

public struct FileWriteError: LocalizedError, Equatable {
    public var path: String
    public var operation: String
    public var code: Int32

    public var errorDescription: String? {
        let reason = String(cString: strerror(code))
        return "Couldn't \(operation) “\((path as NSString).lastPathComponent)”: \(reason)."
    }
}

public enum AtomicFile {
    public static let temporaryMarker = ".fw-tmp-"

    /// Writes `data` to a temporary file beside `url`, flushes it to disk, then renames it over `url`.
    /// Readers see either the previous complete file or the new complete file, never a partial one.
    public static func write(_ data: Data, to url: URL) throws {
        let dir = url.deletingLastPathComponent()
        let tmp = dir.appendingPathComponent(".\(url.lastPathComponent)\(temporaryMarker)\(UUID().uuidString)")
        let fd = open(tmp.path, O_WRONLY | O_CREAT | O_EXCL, 0o644)
        guard fd >= 0 else { throw FileWriteError(path: url.path, operation: "save", code: errno) }
        var failed: Int32 = 0
        data.withUnsafeBytes { (buffer: UnsafeRawBufferPointer) in
            var offset = 0
            while offset < buffer.count {
                let n = Darwin.write(fd, buffer.baseAddress!.advanced(by: offset), buffer.count - offset)
                if n < 0 {
                    if errno == EINTR { continue }
                    failed = errno
                    return
                }
                offset += n
            }
        }
        if failed == 0 {
            if fcntl(fd, F_FULLFSYNC) != 0 && fsync(fd) != 0 { failed = errno }
        }
        close(fd)
        if failed != 0 {
            unlink(tmp.path)
            throw FileWriteError(path: url.path, operation: "save", code: failed)
        }
        if rename(tmp.path, url.path) != 0 {
            let code = errno
            unlink(tmp.path)
            throw FileWriteError(path: url.path, operation: "save", code: code)
        }
        let dfd = open(dir.path, O_RDONLY)
        if dfd >= 0 {
            fsync(dfd)
            close(dfd)
        }
    }

    /// Removes temporary files left behind by an interrupted write. The real file is untouched.
    public static func removeStaleTemporaries(in directory: URL) {
        let fm = FileManager.default
        guard let names = try? fm.contentsOfDirectory(atPath: directory.path) else { return }
        for name in names where name.hasPrefix(".") && name.contains(temporaryMarker) {
            try? fm.removeItem(at: directory.appendingPathComponent(name))
        }
    }
}
