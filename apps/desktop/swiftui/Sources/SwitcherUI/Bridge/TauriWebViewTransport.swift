import Foundation
import WebKit

/// Uses the existing WebView and its existing Tauri ACL; it never interpolates arguments into JavaScript.
@MainActor
public final class TauriWebViewTransport: DesktopTransport {
    private weak var webView: WKWebView?

    public init(webView: WKWebView) { self.webView = webView }

    public func send(command: String, argumentsJSON: String) async throws -> String {
        guard let webView else { throw BridgeError.unavailable }
        let source = """
        try {
          const value = await window.__TAURI_INTERNALS__.invoke(command, JSON.parse(argumentsJSON));
          return JSON.stringify({ok: true, value: value === undefined ? null : value});
        } catch (error) {
          return JSON.stringify({ok: false, error: String(error)});
        }
        """
        let result = try await webView.callAsyncJavaScript(
            source,
            arguments: ["command": command, "argumentsJSON": argumentsJSON],
            in: nil,
            contentWorld: .page
        )
        guard let result = result as? String else { throw BridgeError.invalidResponse }
        return result
    }
}
