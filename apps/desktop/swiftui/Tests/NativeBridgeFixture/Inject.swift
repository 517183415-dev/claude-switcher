import AppKit
import Foundation
import WebKit

@MainActor
private final class CountingTransport: DesktopTransport {
    let inner: TauriWebViewTransport
    var calls = 0

    init(webView: WKWebView) { inner = TauriWebViewTransport(webView: webView) }
    func send(command: String, argumentsJSON: String) async throws -> String {
        calls += 1
        return try await inner.send(command: command, argumentsJSON: argumentsJSON)
    }
}

@MainActor
private func findWebView(_ view: NSView) -> WKWebView? {
    if let webView = view as? WKWebView { return webView }
    return view.subviews.lazy.compactMap { findWebView($0) }.first
}

@MainActor
private func runFixture() async {
    for _ in 0..<80 {
        if let webView = NSApp.windows.compactMap(\.contentView).compactMap(findWebView).first {
            let transport = CountingTransport(webView: webView)
            let personal = DesktopBridge(edition: .personal, transport: transport)
            if let result = try? await personal.invoke(.runtimeInfo),
               case .object(let fields) = result,
               fields["edition"] == .string("personal") {
                do {
                    let draft = try await personal.invoke(.adDraftRead)
                    guard case .object(let draftFields) = draft,
                          case .array(let slots) = draftFields["slots"], slots.count == 3 else {
                        throw BridgeError.invalidResponse
                    }
                    let publicBridge = DesktopBridge(edition: .public, transport: transport)
                    let before = transport.calls
                    do {
                        _ = try await publicBridge.invoke(.adDraftRead)
                        throw BridgeError.invalidResponse
                    } catch BridgeError.commandUnavailable {}
                    guard transport.calls == before else { throw BridgeError.invalidResponse }

                    do {
                        _ = try await personal.invoke(.adImagePreview, arguments: ["sha256": .string("bad\"\\\n")])
                        throw BridgeError.invalidResponse
                    } catch BridgeError.remote(let message) {
                        guard message == "发布数据无效：asset_missing" else { throw BridgeError.invalidResponse }
                    }

                    let denied = try await transport.inner.send(command: "unregistered_command", argumentsJSON: "{}")
                    guard let data = denied.data(using: .utf8),
                          let value = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                          value["ok"] as? Bool == false else { throw BridgeError.invalidResponse }
                    print("SWIFTUI_BRIDGE_RESULT {\"runtime_info\":true,\"draft_read\":true,\"public_gate\":true,\"backend_error\":true,\"unknown_command_denied\":true}")
                    fflush(stdout)
                    exit(0)
                } catch {
                    print("SWIFTUI_BRIDGE_RESULT {\"error\":\"assertion_failed\"}")
                    fflush(stdout)
                    exit(1)
                }
            }
        }
        try? await Task.sleep(nanoseconds: 250_000_000)
    }
    print("SWIFTUI_BRIDGE_RESULT {\"error\":\"webview_or_ipc_unavailable\"}")
    fflush(stdout)
    exit(1)
}

@_cdecl("switcher_swiftui_bridge_fixture_start")
func switcherSwiftUIBridgeFixtureStart() {
    guard ProcessInfo.processInfo.processName == "switcher-desktop" else { return }
    Task { @MainActor in await runFixture() }
}
