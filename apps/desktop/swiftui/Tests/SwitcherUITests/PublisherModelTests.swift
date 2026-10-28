import Foundation
import Testing
@testable import SwitcherUI

@MainActor
private final class PublisherTransport: DesktopTransport {
    var calls: [(String, String)] = []
    var replies: [String: String] = [:]
    var held: Set<String> = []
    var waiting: [String: CheckedContinuation<String, Error>] = [:]
    func send(command: String, argumentsJSON: String) async throws -> String {
        calls.append((command, argumentsJSON))
        if held.contains(command) { return try await withCheckedThrowingContinuation { waiting[command] = $0 } }
        return replies[command] ?? #"{"ok":true,"value":null}"#
    }
    func release(_ command: String) {
        held.remove(command)
        waiting.removeValue(forKey: command)?.resume(returning: replies[command]!)
    }
}

private let publisherDraft = #"{"schema_version":1,"channel":"preview","content_revision":1,"key_id":"key","issued_at":1,"expires_at":4102444800,"release_id":"release","slots":[{"slot":"residential_broadband","enabled":true,"campaigns":[{"campaign_id":"one","merchant_id":"merchant","merchant_name":"商家","title":"标题","description":"说明","image":{"sha256":"abc","mime":"image/png","bytes":1,"width":1,"height":1},"button_text":"查看","target_url":"https://example.com","starts_at":1,"ends_at":4102444800,"enabled":true}]}]}"#
private let publisherStatus = #"{"source":"local","status":"ready","message":"准备完成","history":[]}"#
private func reply(_ value: String) -> String { #"{"ok":true,"value":\#(value)}"# }

@MainActor
private func publisherSetup() -> (PublisherModel, PublisherTransport) {
    let transport = PublisherTransport()
    transport.replies["ad_draft_read"] = reply(publisherDraft)
    transport.replies["publisher_status"] = reply(publisherStatus)
    transport.replies["ad_image_preview"] = reply(#""data:image/png;base64,AQ==""#)
    transport.replies["publisher_recovery_prepare"] = reply(#"{"digest":"digest","backup_filename":"backup.csbackup","generation":"generation"}"#)
    transport.replies["publisher_recovery_confirm"] = reply(#"{"completed":true,"message":"已恢复","recovery":null}"#)
    return (PublisherModel(bridge: DesktopBridge(edition: .personal, transport: transport)), transport)
}

@Test @MainActor
func draftPreviewKeepsAllPublishedCardFieldsAndReflectsEdits() async throws {
    let (model, _) = publisherSetup()
    await model.load()
    let card = try #require(model.previewAd(slot: "residential_broadband", period: 0))
    guard case .active(let content) = card.state else { Issue.record("missing preview content"); return }
    #expect(content.title == "标题")
    #expect(content.description == "说明")
    #expect(content.merchant == "商家")
    #expect(content.button == "查看")
    #expect(content.imageData == Data([1]))
    model.editCampaign("residential_broadband", period: 0) { $0.merchantName = "新商家"; $0.buttonText = "打开" }
    let updated = try #require(model.previewAd(slot: "residential_broadband", period: 0))
    guard case .active(let changed) = updated.state else { Issue.record("missing updated content"); return }
    #expect(changed.merchant == "新商家" && changed.button == "打开")
    let next = try #require(model.previewAd(slot: "residential_broadband", period: 1))
    guard case .unavailable = next.state else { Issue.record("empty period must not reuse the first"); return }
    #expect(model.previewAd(slot: "unknown", period: 0)?.id == nil)
}

@Test @MainActor
func publisherRecoveryStaysLockedUntilBothFreshReadsSucceed() async {
    let (model, transport) = publisherSetup()
    await model.load()
    await model.prepareRecovery(passphrase: "test-passphrase")
    #expect(model.recoveryHolding && model.recoveryPreview != nil)
    transport.held.insert("publisher_status")
    transport.replies["publisher_status"] = #"{"ok":false,"error":"读取失败"}"#
    let confirming = Task { await model.confirmRecovery() }
    for _ in 0..<10 where transport.waiting["publisher_status"] == nil { await Task.yield() }
    #expect(transport.waiting["publisher_status"] != nil)
    await model.publish("production", confirmed: true)
    #expect(!transport.calls.contains { $0.0 == "publish_ad" })
    transport.release("publisher_status")
    await confirming.value
    #expect(model.recoveryHolding && model.recoveryRefreshPending)
    await model.publish("production", confirmed: true)
    #expect(!transport.calls.contains { $0.0 == "publish_ad" })
    transport.replies["publisher_status"] = reply(publisherStatus)
    await model.retryRecoveryRefresh()
    #expect(!model.recoveryHolding && !model.recoveryRefreshPending)
    #expect(model.error == nil)
}

@Test @MainActor
func publisherRecoveryKeepsOperationFailureAcrossRefreshRetries() async {
    for fromBackup in [false, true] {
        let (model, transport) = publisherSetup()
        await model.load()
        if !fromBackup { await model.prepareRecovery(passphrase: "test-passphrase") }
        let command = fromBackup ? "restore_publisher_backup" : "publisher_recovery_confirm"
        transport.replies[command] = #"{"ok":false,"error":"恢复操作失败"}"#
        transport.replies["publisher_status"] = #"{"ok":false,"error":"状态读取失败"}"#
        if fromBackup { await model.restoreBackup(Data([1]), passphrase: "test-passphrase") }
        else { await model.confirmRecovery() }
        #expect(model.recoveryHolding && model.recoveryRefreshPending)
        #expect(model.error?.contains("恢复操作失败") == true)
        #expect(model.error?.contains("读取最新发布资料失败") == true)

        await model.retryRecoveryRefresh()
        #expect(model.recoveryHolding && model.recoveryRefreshPending)
        #expect(model.error?.contains("恢复操作失败") == true)
        transport.replies["publisher_status"] = reply(publisherStatus)
        await model.retryRecoveryRefresh()
        #expect(!model.recoveryHolding && !model.recoveryRefreshPending)
        #expect(model.error == "恢复操作失败")
        #expect(model.outcome == nil)
    }
}

@Test @MainActor
func publisherRestoreReplacesPreviousFailureAndSuccess() async {
    let (model, transport) = publisherSetup()
    await model.load()
    transport.replies["restore_publisher_backup"] = #"{"ok":false,"error":"备份无效"}"#
    await model.restoreBackup(Data([1]), passphrase: "test-passphrase")
    #expect(model.error == "备份无效")
    #expect(model.outcome == nil)

    transport.replies["restore_publisher_backup"] = reply(#"{"completed":false,"message":"恢复完成，尚未发布","recovery":null}"#)
    await model.restoreBackup(Data([2]), passphrase: "test-passphrase")
    #expect(model.error == nil)
    #expect(model.outcome?.message == "恢复完成，尚未发布")
    #expect(model.canEdit && !model.recoveryRefreshPending)

    transport.replies["restore_publisher_backup"] = #"{"ok":false,"error":"备份损坏"}"#
    await model.restoreBackup(Data([3]), passphrase: "test-passphrase")
    #expect(model.error == "备份损坏")
    #expect(model.outcome == nil)
}

@Test @MainActor
func publisherFileReadFailureReplacesPreviousOutcome() async {
    let (model, _) = publisherSetup()
    for message in ["图片读取失败或超过 1 MiB。", "备份读取失败或超过 64 MiB。", "归档读取失败或超过 64 MiB。"] {
        model.outcome = OperationOutcome(completed: true, message: "先前操作成功", recovery: nil)
        model.reportFileError(message)
        #expect(model.error == message)
        #expect(model.outcome == nil)
    }
}

@Test @MainActor
func publisherOutcomeActionReplacesPreviousSuccessOnFailure() async {
    let (model, transport) = publisherSetup()
    transport.replies["export_publisher_backup"] = reply(#"{"completed":true,"message":"备份已保存","recovery":null}"#)
    await model.exportBackup(passphrase: "test-passphrase")
    #expect(model.outcome?.message == "备份已保存")

    transport.replies["export_publisher_backup"] = #"{"ok":false,"error":"备份失败"}"#
    await model.exportBackup(passphrase: "test-passphrase")
    #expect(model.error == "备份失败")
    #expect(model.outcome == nil)
}

@Test @MainActor
func publisherPauseReplacesPreviousFailureAndSuccess() async {
    let (model, transport) = publisherSetup()
    await model.load()
    transport.replies["pause_ad"] = #"{"ok":false,"error":"暂停失败"}"#
    await model.pause(nil)
    #expect(model.error == "暂停失败")
    transport.replies["pause_ad"] = reply(publisherDraft)
    await model.pause(nil)
    #expect(model.error == nil)
    #expect(model.outcome?.completed == true)
    transport.replies["pause_ad"] = #"{"ok":false,"error":"再次暂停失败"}"#
    await model.pause(nil)
    #expect(model.error == "再次暂停失败")
    #expect(model.outcome == nil)
}

@Test @MainActor
func remoteReadFailureClearsPreviouslyLoadedSettings() async {
    let (model, transport) = publisherSetup()
    let saved = #"{"preview":{"account_id":"account","bucket":"old-bucket","public_origin":"https://old.example.com","channel":"preview"},"production":null,"targets":[],"unavailable_targets":[]}"#
    transport.replies["publisher_remote_read"] = reply(saved)
    await model.readRemote()
    #expect(model.remote?.preview?.bucket == "old-bucket")

    transport.replies["publisher_remote_read"] = #"{"ok":false,"error":"密钥不可用"}"#
    await model.readRemote()
    #expect(model.remote == nil)
    #expect(model.error == "密钥不可用")
    await model.saveRemote(channel: "preview", config: nil, accessKey: "", secretKey: "")
    #expect(!transport.calls.contains { $0.0 == "publisher_remote_save" })

    transport.replies["publisher_remote_read"] = reply(saved)
    await model.readRemote()
    #expect(model.remote?.preview?.bucket == "old-bucket" && model.error == nil)
}

@Test @MainActor
func recoveryClearsPreviouslyLoadedRemoteSettings() async {
    for fromBackup in [false, true] {
        let (model, transport) = publisherSetup()
        transport.replies["publisher_remote_read"] = reply(#"{"preview":null,"production":null,"targets":[],"unavailable_targets":[]}"#)
        transport.replies["restore_publisher_backup"] = reply(#"{"completed":false,"message":"已恢复","recovery":null}"#)
        await model.load()
        await model.readRemote()
        #expect(model.remote != nil)
        if fromBackup {
            await model.restoreBackup(Data([1]), passphrase: "test-passphrase")
        } else {
            await model.prepareRecovery(passphrase: "test-passphrase")
            await model.confirmRecovery()
        }
        #expect(model.remote == nil)
        #expect(!model.recoveryHolding)
    }
}

@Test @MainActor
func publisherReadsClearPreviousErrorAfterRetry() async {
    let (model, transport) = publisherSetup()
    let cleanup = #"{"digest":"approved","retain":128,"history_before":0,"history_after":0,"assets_before":0,"assets_after":0,"candidate_bytes":0,"pending_blocks_commit":false,"release_resolution":false,"replaces_checkpoint":false,"retained_history":[],"removed_history_records":[],"protected_assets":[],"candidate_assets":[],"message":"确认"}"#
    let gc = #"{"batch_id":"batch","digest":"digest","eligible":[],"already_missing":[],"blocked":[],"referenced":[],"eligible_bytes":0,"pending_blocks_commit":false,"blocked_details":[],"message":"确认"}"#
    let cases: [(String, String, @MainActor () async -> Void)] = [
        ("publisher_remote_read", #"{"preview":null,"production":null,"targets":[],"unavailable_targets":[]}"#, { await model.readRemote() }),
        ("publisher_retained_list", "[]", { await model.readRetained() }),
        ("preview_publisher_cleanup", cleanup, { await model.previewCleanup(retain: 128, releaseResolution: false) }),
        ("preview_publisher_gc", gc, { await model.previewGC() })
    ]
    for (command, success, action) in cases {
        transport.replies[command] = #"{"ok":false,"error":"读取失败"}"#
        await action()
        #expect(model.error == "读取失败")
        transport.replies[command] = reply(success)
        await action()
        #expect(model.error == nil, "\(command) retained an old error")
    }
}

@Test @MainActor
func publisherRejectsUnconfirmedDirtyOrInvalidPublication() async {
    let (model, transport) = publisherSetup()
    await model.load()
    await model.publish("production", confirmed: false)
    model.editCampaign("residential_broadband", period: 0) { $0.targetUrl = "http://example.com" }
    await model.publish("production", confirmed: true)
    #expect(model.dirty && !model.validation.isEmpty)
    #expect(!transport.calls.contains { $0.0 == "publish_ad" })
    transport.replies["ad_draft_save"] = reply(publisherDraft)
    await model.save()
    transport.replies["publish_ad"] = reply(#"{"completed":true,"message":"已发布","recovery":null}"#)
    await model.publish("production", confirmed: true)
    #expect(transport.calls.filter { $0.0 == "publish_ad" }.count == 1)
}

@Test @MainActor
func cleanupCannotCommitChangedOrBlockedPreview() async {
    let (model, transport) = publisherSetup()
    let preview = #"{"digest":"approved","retain":128,"history_before":3,"history_after":2,"assets_before":2,"assets_after":1,"candidate_bytes":20,"pending_blocks_commit":false,"release_resolution":false,"replaces_checkpoint":false,"retained_history":[],"removed_history_records":[],"protected_assets":[],"candidate_assets":[],"message":"确认"}"#
    transport.replies["preview_publisher_cleanup"] = reply(preview)
    await model.previewCleanup(retain: 128, releaseResolution: false)
    await model.commitCleanup(retain: 127, releaseResolution: false)
    #expect(!transport.calls.contains { $0.0 == "commit_publisher_cleanup" })
    transport.replies["preview_publisher_cleanup"] = reply(preview.replacingOccurrences(of: "\"pending_blocks_commit\":false", with: "\"pending_blocks_commit\":true"))
    await model.previewCleanup(retain: 128, releaseResolution: false)
    await model.commitCleanup(retain: 128, releaseResolution: false)
    #expect(!transport.calls.contains { $0.0 == "commit_publisher_cleanup" })
}

@Test @MainActor
func pauseOnlyCreatesDraftAndRecoveryCancelFailureKeepsLock() async {
    let (model, transport) = publisherSetup()
    await model.load()
    transport.replies["pause_ad"] = reply(publisherDraft)
    await model.pause(nil)
    #expect(!transport.calls.contains { $0.0 == "publish_ad" })
    await model.prepareRecovery(passphrase: "test-passphrase")
    transport.replies["publisher_recovery_cancel"] = #"{"ok":false,"error":"取消失败"}"#
    await model.cancelRecovery()
    #expect(model.recoveryHolding && model.recoveryPreview != nil)
    await model.publish("preview", confirmed: true)
    #expect(!transport.calls.contains { $0.0 == "publish_ad" })
}

@Test @MainActor
func binaryUploadsUseCompactBase64() async throws {
    let (model, transport) = publisherSetup()
    await model.load()
    let bytes = Data([0, 1, 127, 255])
    transport.replies["import_ad_image"] = #"{"ok":false,"error":"test"}"#
    transport.replies["publisher_archive_import"] = reply(#"{"completed":true,"message":"ok","recovery":null}"#)
    transport.replies["restore_publisher_backup"] = reply(#"{"completed":true,"message":"ok","recovery":null}"#)
    await model.importImage(bytes, slot: "residential_broadband", period: 0)
    await model.importArchive(bytes, passphrase: "test-passphrase")
    await model.restoreBackup(bytes, passphrase: "test-passphrase")
    for command in ["import_ad_image", "publisher_archive_import", "restore_publisher_backup"] {
        let call = try #require(transport.calls.first { $0.0 == command })
        let arguments = try JSONDecoder().decode([String: JSONValue].self, from: Data(call.1.utf8))
        #expect(arguments["bytes"] == .string("AAF//w=="))
    }
}
