import Foundation
import Testing
@testable import SwitcherUI

@MainActor
private final class RecordingTransport: DesktopTransport {
    var calls: [(String, String)] = []
    var response = #"{"ok":true,"value":null}"#
    func send(command: String, argumentsJSON: String) async throws -> String {
        calls.append((command, argumentsJSON))
        return response
    }
}

@Test @MainActor
func publicEditionCannotSendPersonalCommands() async throws {
    let transport = RecordingTransport()
    let bridge = DesktopBridge(edition: .public, transport: transport)
    for command in DesktopCommand.allCases where !command.isAvailable(in: .public) {
        do {
            _ = try await bridge.invoke(command)
            Issue.record("personal command reached public transport: \(command)")
        } catch BridgeError.commandUnavailable {}
    }
    #expect(transport.calls.isEmpty)
    _ = try await bridge.invoke(.workspaceSnapshot)
    #expect(transport.calls.map(\.0) == ["workspace_snapshot"])
}

@Test @MainActor
func transportReceivesJSONArgumentsAndReturnsStructuredResult() async throws {
    let transport = RecordingTransport()
    transport.response = #"{"ok":true,"value":{"completed":true,"message":"完成"}}"#
    let bridge = DesktopBridge(edition: .personal, transport: transport)
    let value = try await bridge.invoke(.publisherRemoteSave, arguments: [
        "secretKey": .string("quote\"\\newline\n"), "enabled": .bool(true),
    ])
    #expect(value == .object(["completed": .bool(true), "message": .string("完成")]))
    let sent = try JSONDecoder().decode([String: JSONValue].self, from: Data(transport.calls[0].1.utf8))
    #expect(sent["secretKey"] == .string("quote\"\\newline\n"))
    #expect(sent["enabled"] == .bool(true))
}

@Test @MainActor
func bridgeRejectsBadResponseAndReturnsBackendError() async throws {
    let transport = RecordingTransport()
    let bridge = DesktopBridge(edition: .personal, transport: transport)
    transport.response = "not JSON"
    await #expect(throws: BridgeError.invalidResponse) { try await bridge.invoke(.runtimeInfo) }
    transport.response = #"{"ok":true}"#
    await #expect(throws: BridgeError.invalidResponse) { try await bridge.invoke(.runtimeInfo) }
    transport.response = #"{"ok":false}"#
    await #expect(throws: BridgeError.invalidResponse) { try await bridge.invoke(.runtimeInfo) }
    transport.response = #"{"ok":false,"error":"拒绝"}"#
    await #expect(throws: BridgeError.remote("拒绝")) { try await bridge.invoke(.runtimeInfo) }
}

@Test @MainActor
func nonFiniteArgumentsFailBeforeTransport() async throws {
    let transport = RecordingTransport()
    let bridge = DesktopBridge(edition: .personal, transport: transport)
    do {
        _ = try await bridge.invoke(.adDraftSave, arguments: ["value": .number(.infinity)])
        Issue.record("non-finite number accepted")
    } catch {}
    #expect(transport.calls.isEmpty)
}

@Test
func commandGateMatchesProductManifest() throws {
    let source = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
        .appendingPathComponent("src-tauri/build.rs")
    let text = try String(contentsOf: source, encoding: .utf8)
    let marker = "else if std::env::var_os(\"CARGO_FEATURE_PERSONAL\").is_some()"
    let start = try #require(text.range(of: marker, options: .backwards)?.upperBound)
    let finish = try #require(text.range(of: "};\n    let manifest", range: start..<text.endIndex)?.lowerBound)
    let sections = text[start..<finish].components(separatedBy: "]\n    } else {")
    #expect(sections.count == 2)
    guard sections.count == 2 else { return }
    let pattern = try NSRegularExpression(pattern: #"\"([a-z][a-z0-9_]*)\""#)
    func commands(_ section: String) -> Set<String> {
        Set(pattern.matches(in: section, range: NSRange(section.startIndex..., in: section)).compactMap {
            Range($0.range(at: 1), in: section).map { String(section[$0]) }
        })
    }
    #expect(commands(sections[0]) == Set(DesktopCommand.allCases.map(\.rawValue)))
    #expect(commands(sections[1]) == Set(DesktopCommand.allCases.filter { $0.isAvailable(in: .public) }.map(\.rawValue)))
}
