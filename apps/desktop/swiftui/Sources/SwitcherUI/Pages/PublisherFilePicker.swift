import AppKit
import Darwin
import Foundation
import UniformTypeIdentifiers

enum PublisherFilePicker {
    /// Read in bounded chunks so a file that grows after selection cannot bypass the limit.
    static func read(limit: Int, image: Bool = false) throws -> Data? {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        if image { panel.allowedContentTypes = [.png, .jpeg] }
        guard panel.runModal() == .OK, let url = panel.url else { return nil }
        return try readFile(at: url, limit: limit)
    }

    static func readFile(at url: URL, limit: Int) throws -> Data {
        guard limit >= 0, limit < Int.max else { throw CocoaError(.fileReadUnknown) }
        // Opening a FIFO in blocking mode can hang the UI before its type is checked.
        let descriptor: Int32 = url.withUnsafeFileSystemRepresentation { path in
            guard let path else { return -1 }
            return Darwin.open(path, O_RDONLY | O_NONBLOCK | O_NOFOLLOW)
        }
        guard descriptor >= 0 else { throw CocoaError(.fileReadUnknown) }
        let handle = FileHandle(fileDescriptor: descriptor, closeOnDealloc: true)
        defer { try? handle.close() }
        var details = stat()
        guard fstat(descriptor, &details) == 0, details.st_mode & S_IFMT == S_IFREG else {
            throw CocoaError(.fileReadUnknown)
        }
        guard details.st_size >= 0, details.st_size <= limit else { throw CocoaError(.fileReadTooLarge) }
        var result = Data()
        while let part = try handle.read(upToCount: min(65_536, limit + 1 - result.count)), !part.isEmpty {
            result.append(part)
            guard result.count <= limit else { throw CocoaError(.fileReadTooLarge) }
        }
        return result
    }
}
