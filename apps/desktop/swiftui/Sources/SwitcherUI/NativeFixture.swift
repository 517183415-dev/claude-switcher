#if SWITCHER_NATIVE_FIXTURE
import AppKit
import Foundation
import WebKit

/// Compiled only into the explicitly isolated IPC fixture.
@MainActor
func verifyNativeFixture(webView: WKWebView, host: NSView, bridge: DesktopBridge) async {
    var stage = "workspace"
    do {
        let workspace = WorkspaceModel(bridge: bridge)
        try await workspace.refresh()
        let publisher = PublisherModel(bridge: bridge)
        await publisher.load()
        guard workspace.snapshot?.runtime.edition == bridge.edition, host.superview === webView.superview,
              webView.url?.lastPathComponent == "native-bridge.html" else { throw BridgeError.invalidResponse }
        var checks: [String: Bool] = ["workspace": true, "mounted": true, "bridge_page": true]
        stage = "edition_gate"
        let publicBridge = DesktopBridge(edition: .public, transport: TauriWebViewTransport(webView: webView))
        do { _ = try await publicBridge.invoke(.adDraftRead); throw BridgeError.invalidResponse }
        catch BridgeError.commandUnavailable {}
        checks["swift_gate"] = true
        if bridge.edition == .personal {
            guard publisher.draft?.slots.count == 3, publisher.status != nil else { throw BridgeError.invalidResponse }
            checks["publisher"] = true
            stage = "binary_input"
            do {
                _ = try await bridge.invoke(.importAdImage, arguments: ["bytes": .string("AQID")])
                throw BridgeError.invalidResponse
            } catch BridgeError.remote(let message) {
                guard message.contains("请选择静态 PNG 或 JPEG 图片") else { throw BridgeError.invalidResponse }
            }
            checks["binary_input"] = true
            if ProcessInfo.processInfo.environment["NATIVE_UI_FIXTURE_LARGE_BACKUP"] == "1" {
                stage = "backup_at_limit"
                print("SWITCHER_NATIVE_STAGE \(stage)")
                fflush(stdout)
                let encoder = JSONEncoder()
                encoder.outputFormatting = [.sortedKeys]
                let before = try encoder.encode(publisher.draft)
                await publisher.restoreBackup(Data(count: 64 * 1024 * 1024), passphrase: "synthetic-passphrase")
                guard publisher.error?.contains("backup_header") == true,
                      publisher.canEdit, !publisher.recoveryRefreshPending,
                      try encoder.encode(publisher.draft) == before else {
                    print("SWITCHER_NATIVE_LARGE_FAILURE \(publisher.error ?? "no error")")
                    throw BridgeError.invalidResponse
                }
                checks["backup_at_limit"] = true

                stage = "backup_over_limit"
                print("SWITCHER_NATIVE_STAGE \(stage)")
                fflush(stdout)
                let oversized = Data(count: 64 * 1024 * 1024 + 1).base64EncodedString()
                do {
                    _ = try await bridge.invoke(.restorePublisherBackup, arguments: [
                        "bytes": .string(oversized), "passphrase": .string("synthetic-passphrase")
                    ])
                    throw BridgeError.invalidResponse
                } catch BridgeError.remote(let message) {
                    guard message.contains("备份文件超过 64 MiB 上限") else {
                        print("SWITCHER_NATIVE_LARGE_FAILURE \(message)")
                        throw BridgeError.invalidResponse
                    }
                }
                checks["backup_over_limit"] = true
            }
        } else {
            guard publisher.draft == nil, !SwitcherPage.visible(in: .public).contains(.operations)
            else { throw BridgeError.invalidResponse }
            checks["publisher_hidden"] = true
            stage = "backend_gate"
            let transport = TauriWebViewTransport(webView: webView)
            let response = try await transport.send(command: "ad_draft_read", argumentsJSON: "{}")
            guard let data = response.data(using: .utf8),
                  let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  object["ok"] as? Bool == false,
                  let message = object["error"] as? String, message.lowercased().contains("not allowed")
            else { throw BridgeError.invalidResponse }
            checks["backend_denied"] = true
        }
        stage = "screenshot"
        try await Task.sleep(nanoseconds: 1_000_000_000)
        var screenshot = false
        if let path = ProcessInfo.processInfo.environment["NATIVE_UI_FIXTURE_IMAGE"],
           let image = host.bitmapImageRepForCachingDisplay(in: host.bounds) {
            host.cacheDisplay(in: host.bounds, to: image)
            if let png = image.representation(using: .png, properties: [:]) {
                try png.write(to: URL(fileURLWithPath: path), options: .atomic)
                screenshot = true
            }
        }
        checks["screenshot"] = screenshot
        let data = try JSONSerialization.data(withJSONObject: checks, options: [.sortedKeys])
        print("SWITCHER_NATIVE_RESULT \(String(decoding: data, as: UTF8.self))")
        fflush(stdout)
        exit(screenshot ? 0 : 1)
    } catch {
        print("SWITCHER_NATIVE_RESULT {\"error\":\"native_fixture_failed\",\"stage\":\"\(stage)\"}")
        fflush(stdout)
        exit(1)
    }
}
#endif
