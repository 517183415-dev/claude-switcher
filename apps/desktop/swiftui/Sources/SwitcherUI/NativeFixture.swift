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
            if ProcessInfo.processInfo.environment["NATIVE_UI_FIXTURE_BACKUP_RECOVERY"] == "1" {
                stage = "backup_recovery"
                try await verifyBackupRecovery(publisher: publisher)
                checks["backup_recovery"] = true
            }
            if ProcessInfo.processInfo.environment["NATIVE_UI_FIXTURE_VALID_LARGE_BACKUP"] == "1" {
                stage = "valid_backup_at_limit"
                print("SWITCHER_NATIVE_STAGE \(stage)")
                fflush(stdout)
                try await verifyValidLargeBackup(publisher: publisher)
                checks["valid_backup_at_limit"] = true
            }
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

/// Exercises the production bridge with only the fixture's isolated memory vault.
@MainActor
private func verifyBackupRecovery(publisher: PublisherModel) async throws {
    let slot = "residential_broadband"
    let passphrase = "synthetic-passphrase"
    guard let root = ProcessInfo.processInfo.environment["IPC_BUSINESS_ROOT"],
          let original = publisher.draft?.slots.first(where: { $0.slot == slot })?.enabled else {
        throw BridgeError.invalidResponse
    }
    await publisher.outcomeAction(.createAdSigningKey, label: "创建测试密钥", refreshStatus: true)
    guard publisher.error == nil, publisher.outcome?.completed == true else { throw BridgeError.invalidResponse }

    publisher.editSlot(slot) { $0.enabled = !original }
    await publisher.save()
    guard publisher.error == nil, !publisher.dirty else { throw BridgeError.invalidResponse }
    await publisher.exportBackup(passphrase: passphrase)
    guard publisher.error == nil, publisher.outcome?.completed == true else { throw BridgeError.invalidResponse }
    let directory = URL(fileURLWithPath: root).appendingPathComponent("v2/personal/publisher/backups")
    let backups = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
        .filter { $0.pathExtension == "csbackup" }
    guard backups.count == 1 else { throw BridgeError.invalidResponse }
    let bytes = try PublisherFilePicker.readFile(at: backups[0], limit: 64 * 1024 * 1024)

    publisher.editSlot(slot) { $0.enabled = original }
    await publisher.save()
    guard publisher.error == nil, !publisher.dirty else { throw BridgeError.invalidResponse }
    await publisher.restoreBackup(Data([1]), passphrase: passphrase)
    guard publisher.error != nil, publisher.outcome == nil,
          publisher.draft?.slots.first(where: { $0.slot == slot })?.enabled == original else {
        throw BridgeError.invalidResponse
    }
    await publisher.restoreBackup(bytes, passphrase: passphrase)
    guard publisher.error == nil, publisher.outcome?.completed == false,
          publisher.outcome?.message.contains("尚未发布") == true,
          publisher.draft?.slots.first(where: { $0.slot == slot })?.enabled == !original,
          publisher.status?.history.isEmpty == true, publisher.canEdit, !publisher.recoveryRefreshPending else {
        throw BridgeError.invalidResponse
    }
    // A second export proves the restored generation can retrieve its signing key.
    await publisher.exportBackup(passphrase: passphrase)
    guard publisher.error == nil, publisher.outcome?.completed == true else { throw BridgeError.invalidResponse }
}

@MainActor
private func verifyValidLargeBackup(publisher: PublisherModel) async throws {
    let slot = "residential_broadband"
    guard let tmp = ProcessInfo.processInfo.environment["TMPDIR"],
          let original = publisher.draft?.slots.first(where: { $0.slot == slot })?.enabled else {
        throw BridgeError.invalidResponse
    }
    let file = URL(fileURLWithPath: tmp).appendingPathComponent("valid-large.csbackup")
    let bytes = try PublisherFilePicker.readFile(at: file, limit: 64 * 1024 * 1024)
    guard bytes.count == 64 * 1024 * 1024 else { throw BridgeError.invalidResponse }
    publisher.editSlot(slot) { $0.enabled = !original }
    await publisher.save()
    guard publisher.error == nil, !publisher.dirty else { throw BridgeError.invalidResponse }
    await publisher.restoreBackup(Data([1]), passphrase: "synthetic-passphrase")
    guard publisher.error != nil, publisher.outcome == nil,
          publisher.draft?.slots.first(where: { $0.slot == slot })?.enabled == !original else {
        throw BridgeError.invalidResponse
    }
    await publisher.restoreBackup(bytes, passphrase: "synthetic-passphrase")
    guard publisher.error == nil, publisher.outcome?.completed == false,
          publisher.outcome?.message.contains("尚未发布") == true,
          publisher.draft?.slots.first(where: { $0.slot == slot })?.enabled == original,
          publisher.status?.history.isEmpty == true, publisher.canEdit, !publisher.recoveryRefreshPending else {
        print("SWITCHER_NATIVE_LARGE_FAILURE \(publisher.error ?? "unexpected recovery state")")
        throw BridgeError.invalidResponse
    }
    await publisher.exportBackup(passphrase: "synthetic-passphrase")
    guard publisher.error == nil, publisher.outcome?.completed == true else { throw BridgeError.invalidResponse }
}
#endif
