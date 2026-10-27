import AppKit
import SwiftUI
import WebKit

/// Called by Tauri with its own WKWebView. The view remains alive for the IPC bridge.
@_cdecl("switcher_native_attach")
public func switcherNativeAttach(_ pointer: UnsafeMutableRawPointer?, _ publicEdition: Int32) {
    guard let pointer else { return }
    let webView = Unmanaged<WKWebView>.fromOpaque(pointer).takeUnretainedValue()
    Task { @MainActor [weak webView] in
        guard let webView else { return }
        for _ in 0..<80 {
            if webView.superview != nil,
               let ready = try? await webView.callAsyncJavaScript(
                   "return typeof window.__TAURI_INTERNALS__?.invoke === 'function'",
                   arguments: [:], in: nil, contentWorld: .page) as? Bool, ready { break }
            try? await Task.sleep(nanoseconds: 250_000_000)
        }
        guard let container = webView.superview else { return }
        let edition: DesktopEdition = publicEdition == 0 ? .personal : .public
        let bridge = DesktopBridge(edition: edition, transport: TauriWebViewTransport(webView: webView))
        let host = NSHostingView(rootView: SwitcherWorkspaceView(bridge: bridge))
        host.frame = webView.frame
        host.autoresizingMask = [.width, .height]
        container.addSubview(host, positioned: .above, relativeTo: webView)
        #if SWITCHER_NATIVE_FIXTURE
        if ProcessInfo.processInfo.environment["IPC_ORDINARY_PUBLISHER_OPT_IN"] == "synthetic-memory-vault-only" {
            Task { await verifyNativeFixture(webView: webView, host: host, bridge: bridge) }
        }
        #endif
    }
}
