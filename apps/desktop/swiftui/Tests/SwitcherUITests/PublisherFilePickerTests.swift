import Darwin
import Foundation
import Testing
@testable import SwitcherUI

@Test
func publisherFileReaderAcceptsOnlyBoundedRegularFiles() throws {
    let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }

    let file = directory.appendingPathComponent("backup")
    try Data([1, 2, 3]).write(to: file)
    #expect(try PublisherFilePicker.readFile(at: file, limit: 3) == Data([1, 2, 3]))
    #expect(throws: CocoaError.self) { try PublisherFilePicker.readFile(at: file, limit: 2) }
    #expect(throws: CocoaError.self) { try PublisherFilePicker.readFile(at: file, limit: -1) }

    let link = directory.appendingPathComponent("link")
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: file)
    #expect(throws: CocoaError.self) { try PublisherFilePicker.readFile(at: link, limit: 3) }

    let fifo = directory.appendingPathComponent("fifo")
    #expect(mkfifo(fifo.path, 0o600) == 0)
    #expect(throws: CocoaError.self) { try PublisherFilePicker.readFile(at: fifo, limit: 3) }
}
