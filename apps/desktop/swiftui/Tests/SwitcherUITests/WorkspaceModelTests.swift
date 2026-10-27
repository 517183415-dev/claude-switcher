import Foundation
import Testing
@testable import SwitcherUI

@MainActor
private final class WorkspaceTransport: DesktopTransport {
    var calls: [(String, String)] = []
    var replies: [String: String] = [:]
    var held: Set<String> = []
    var waiting: [String: CheckedContinuation<String, Error>] = [:]

    func send(command: String, argumentsJSON: String) async throws -> String {
        calls.append((command, argumentsJSON))
        if held.contains(command) {
            return try await withCheckedThrowingContinuation { waiting[command] = $0 }
        }
        return replies[command] ?? #"{"ok":true,"value":null}"#
    }

    func release(_ command: String) {
        held.remove(command)
        waiting.removeValue(forKey: command)?.resume(returning: replies[command]!)
    }
}

private let report = #"{"completed_at":10,"checks":[{"id":"guardian","status":"pass","reason":"ok","observed_at":10,"duration_ms":1,"valid_until":40,"evidence":[]}],"summary":{"passed":1,"advisory":0,"blocking":0}}"#
private let snapshot = #"{"runtime":{"version":"0.2.0","edition":"personal","automatic_updates":false},"accounts":{"current":"A","profiles":[],"action_blocker":null},"preferences":{"ads":{"slots_enabled":[false,false,false],"request_purpose_acknowledged":false},"updates_paused_until":null},"check_report":null,"ads_manifest":null,"update":{"paused":false,"message":"idle","next_check_at":null,"candidate_version":null,"can_download":false,"can_install":false},"pending_materials":[]}"#
private let repair = #"{"can_execute":true,"message":"ready","original":{"country":"US","timezone":"UTC","locale":"en_US"},"target":{"country":"US","timezone":"UTC","locale":"en_US"},"network_service":"Wi-Fi","original_ipv6":"off","target_ipv6":"off","user_changes":[],"recovery_required":false}"#
private let manifest = #"{"schema_version":1,"channel":"production","content_revision":1,"key_id":"key","issued_at":1,"expires_at":4102444800,"release_id":"release","slots":[{"slot":"residential_broadband","enabled":true,"campaigns":[{"campaign_id":"one","merchant_id":"m","merchant_name":"店铺","title":"标题","description":"说明","image":{"sha256":"abc","mime":"image/png","bytes":1,"width":1,"height":1},"button_text":"查看","target_url":"https://example.com","starts_at":1,"ends_at":4102444800,"enabled":true}]}]}"#
private func success(_ value: String) -> String { #"{"ok":true,"value":\#(value)}"# }

@MainActor
private func setup() -> (WorkspaceModel, WorkspaceTransport) {
    let transport = WorkspaceTransport()
    transport.replies["workspace_snapshot"] = success(snapshot)
    transport.replies["check_environment"] = success(report)
    transport.replies["cancel_check"] = success(#"{"completed":true,"message":"已取消","recovery":null}"#)
    return (WorkspaceModel(bridge: DesktopBridge(edition: .personal, transport: transport)), transport)
}

@Test @MainActor
func staleCheckCannotRestoreEvidenceAfterCancellation() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    transport.held.insert("check_environment")
    let checking = Task { await model.runChecks() }
    await Task.yield()
    #expect(model.canCancelCheck)
    #expect(transport.waiting["check_environment"] != nil)
    await model.cancelChecks()
    transport.release("check_environment")
    await checking.value
    #expect(model.snapshot?.checkReport == nil)
    #expect(!model.canCancelCheck)
    #expect(model.busy == nil)
    #expect(model.outcome?.message == "已取消")
}

@Test @MainActor
func earlierSnapshotCannotOverwriteNewCheck() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    transport.held.insert("workspace_snapshot")
    let oldSnapshot = Task { try await model.refresh() }
    await Task.yield()
    #expect(transport.waiting["workspace_snapshot"] != nil)
    await model.runChecks()
    transport.release("workspace_snapshot")
    try await oldSnapshot.value
    #expect(model.snapshot?.checkReport?.summary.passed == 1)
    #expect(model.overview.groups.allSatisfy { !$0.good })
}

@Test @MainActor
func adEnableRequiresConsentAndChangesOnlySelectedSlot() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    await model.setAdEnabled(index: 1, enabled: true)
    #expect(transport.calls.map(\.0) == ["workspace_snapshot"])
    transport.replies["save_preferences"] = success(#"{"ads":{"slots_enabled":[false,true,false],"request_purpose_acknowledged":true},"updates_paused_until":null}"#)
    await model.setAdEnabled(index: 1, enabled: true, acknowledged: true)
    let arguments = try JSONDecoder().decode([String: JSONValue].self, from: Data(transport.calls.last!.1.utf8))
    guard case .object(let preferences) = arguments["preferences"],
          case .object(let ads) = preferences["ads"] else { Issue.record("missing preferences"); return }
    #expect(ads["slots_enabled"] == .array([.bool(false), .bool(true), .bool(false)]))
    #expect(ads["request_purpose_acknowledged"] == .bool(true))
    #expect(model.snapshot?.preferences.ads.slotsEnabled == [false, true, false])
}

@Test @MainActor
func failedSaveDoesNotChangePreferencesAndWrongEditionIsRejected() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    transport.replies["save_preferences"] = #"{"ok":false,"error":"保存失败"}"#
    await model.setAdEnabled(index: 0, enabled: true, acknowledged: true)
    #expect(model.snapshot?.preferences.ads.slotsEnabled == [false, false, false])
    #expect(model.error == "保存失败")
    let publicModel = WorkspaceModel(bridge: DesktopBridge(edition: .public, transport: transport))
    await #expect(throws: BridgeError.invalidResponse) { try await publicModel.refresh() }
    #expect(publicModel.snapshot == nil)
}

@Test @MainActor
func adPreferenceRetryReplacesPreviousFeedback() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    model.outcome = OperationOutcome(completed: true, message: "旧操作成功", recovery: nil)
    transport.replies["save_preferences"] = #"{"ok":false,"error":"保存失败"}"#
    await model.setAdEnabled(index: 0, enabled: true, acknowledged: true)
    #expect(model.error == "保存失败")
    #expect(model.outcome == nil)
    #expect(model.snapshot?.preferences.ads.slotsEnabled == [false, false, false])

    transport.replies["save_preferences"] = success(#"{"ads":{"slots_enabled":[true,false,false],"request_purpose_acknowledged":true},"updates_paused_until":null}"#)
    await model.setAdEnabled(index: 0, enabled: true, acknowledged: true)
    #expect(model.error == nil)
    #expect(model.snapshot?.preferences.ads.slotsEnabled == [true, false, false])
}

@Test @MainActor
func activityExportRetryReplacesPreviousFeedback() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    let entries = #"[{"observed_at":1,"level":"info","message":"合成记录"}]"#
    transport.replies["activity_log"] = success(entries)
    await model.loadActivity()
    transport.replies["export_activity"] = success(#"["/synthetic/export.json",\#(entries)]"#)
    await model.exportActivity()
    #expect(model.outcome?.completed == true)
    transport.replies["export_activity"] = #"{"ok":false,"error":"导出失败"}"#
    await model.exportActivity()
    #expect(model.error == "导出失败")
    #expect(model.outcome == nil)
    #expect(model.activity.count == 1)

    transport.replies["export_activity"] = success(#"["/synthetic/export.json",\#(entries)]"#)
    await model.exportActivity()
    #expect(model.error == nil)
    #expect(model.outcome?.completed == true)
}

@Test @MainActor
func adLinkRetryReplacesPreviousFeedback() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    model.outcome = OperationOutcome(completed: true, message: "旧操作成功", recovery: nil)
    transport.replies["open_ad_link"] = #"{"ok":false,"error":"核验失败"}"#
    await model.openAd(slot: "account_service", confirmed: false)
    #expect(model.error == "核验失败")
    #expect(model.outcome == nil)

    transport.replies["open_ad_link"] = success(#"{"completed":false,"message":"确认商家","recovery":"confirm_ad_link"}"#)
    await model.openAd(slot: "account_service", confirmed: false)
    #expect(model.error == nil)
    #expect(model.adConfirmation?.message == "确认商家")
}

@Test @MainActor
func cancelledRepairPreviewCannotExecuteAfterLateResponse() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    transport.replies["preview_repair"] = success(repair)
    transport.held.insert("preview_repair")
    let preview = Task { await model.previewRepair(autoExecute: true) }
    await Task.yield()
    #expect(transport.waiting["preview_repair"] != nil)
    await model.cancelRepairPlan()
    transport.release("preview_repair")
    await preview.value
    #expect(!transport.calls.contains { $0.0 == "execute_repair" })
    #expect(model.repairPreview == nil)
    #expect(model.busy == nil)
}

@Test @MainActor
func completedRepairTriggersFreshEnvironmentCheck() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    transport.replies["preview_repair"] = success(repair)
    transport.replies["execute_repair"] = success(#"{"completed":true,"message":"已修复","recovery":null}"#)
    transport.replies["region_view"] = success(#"{"country":"US","timezone":"UTC","locale":"en_US"}"#)
    await model.previewRepair(autoExecute: true)
    #expect(transport.calls.map(\.0) == ["workspace_snapshot", "preview_repair", "execute_repair", "region_view", "check_environment"])
    #expect(model.snapshot?.checkReport?.summary.passed == 1)
    #expect(model.busy == nil)
}

@Test @MainActor
func recoveredRepairCancelsOldPlanBeforeRepreview() async throws {
    let (model, transport) = setup()
    try await model.refresh()
    transport.replies["preview_region_change"] = success(repair.replacingOccurrences(of: "\"recovery_required\":false", with: "\"recovery_required\":true"))
    transport.replies["recover_repair"] = success(#"{"completed":true,"message":"已恢复","recovery":null}"#)
    transport.replies["region_view"] = success(#"{"country":"US","timezone":"UTC","locale":"en_US"}"#)
    await model.previewRepair(region: "US")
    await model.recoverRepair()
    let calls = transport.calls.map(\.0)
    #expect(calls == ["workspace_snapshot", "preview_region_change", "recover_repair", "region_view", "cancel_repair", "preview_region_change"])
    #expect(!calls.contains("execute_repair"))
    #expect(model.busy == nil)
}

@Test @MainActor
func disabledAdsDoNotReadCachedImagesAndLateImageIsDiscarded() async throws {
    let (model, transport) = setup()
    transport.replies["workspace_snapshot"] = success(snapshot.replacingOccurrences(of: "\"ads_manifest\":null", with: "\"ads_manifest\":\(manifest)"))
    try await model.refresh()
    #expect(!transport.calls.contains { $0.0 == "ad_cached_image" })
    let enabled = snapshot.replacingOccurrences(of: "\"ads_manifest\":null", with: "\"ads_manifest\":\(manifest)")
        .replacingOccurrences(of: "\"slots_enabled\":[false,false,false]", with: "\"slots_enabled\":[true,false,false]")
        .replacingOccurrences(of: "\"request_purpose_acknowledged\":false", with: "\"request_purpose_acknowledged\":true")
    transport.replies["workspace_snapshot"] = success(enabled)
    transport.replies["ad_cached_image"] = success(#""data:image/png;base64,AQ==""#)
    transport.replies["save_preferences"] = success(#"{"ads":{"slots_enabled":[false,false,false],"request_purpose_acknowledged":true},"updates_paused_until":null}"#)
    transport.held.insert("ad_cached_image")
    let refresh = Task { try await model.refresh() }
    await Task.yield()
    #expect(transport.waiting["ad_cached_image"] != nil)
    await model.setAdEnabled(index: 0, enabled: false)
    transport.release("ad_cached_image")
    try await refresh.value
    #expect(model.adImages.isEmpty)
    if case .disabled = model.overview.ads[0].state {} else { Issue.record("disabled slot must remain disabled") }
}

@Test @MainActor
func enabledSlotWithoutConsentDoesNotDisplayCampaign() async throws {
    let (model, transport) = setup()
    let noConsent = snapshot.replacingOccurrences(of: "\"ads_manifest\":null", with: "\"ads_manifest\":\(manifest)")
        .replacingOccurrences(of: "\"slots_enabled\":[false,false,false]", with: "\"slots_enabled\":[true,false,false]")
    transport.replies["workspace_snapshot"] = success(noConsent)
    try await model.refresh()
    #expect(!transport.calls.contains { $0.0 == "ad_cached_image" })
    if case .unavailable = model.overview.ads[0].state {} else { Issue.record("campaign requires acknowledged consent") }
}

@Test
func recoveryGuidanceRemainsVisibleForKnownAndUnknownCodes() {
    #expect(desktopRecoveryMessage("clock_recovery_required") == "请校正系统时间，重新建立时间基线后重试。")
    #expect(desktopRecoveryMessage("请先恢复备份") == "请先恢复备份")
    #expect(desktopRecoveryMessage("unknown_code") == "请查看活动记录中的原因，完成处理后重试。")
    #expect(desktopRecoveryMessage(nil) == nil)
}
