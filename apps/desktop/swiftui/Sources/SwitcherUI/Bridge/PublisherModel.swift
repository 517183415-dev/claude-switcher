import Combine
import Foundation

/// Presentation state for Personal. Rust validates, signs and persists every operation.
@MainActor
final class PublisherModel: ObservableObject {
    @Published private(set) var draft: AdManifest?
    @Published private(set) var status: PublisherView?
    @Published private(set) var dirty = false
    @Published private(set) var busy: String?
    @Published var error: String?
    private var recoveryFailure: String?
    @Published var outcome: OperationOutcome?
    @Published private(set) var images: [String: Data] = [:]
    @Published private(set) var remote: RemoteSettings?
    @Published private(set) var retained: [RetainedGeneration] = []
    @Published private(set) var recoveryPreview: PublisherRecoveryPreview?
    @Published private(set) var recoveryHolding = false
    @Published private(set) var recoveryRefreshPending = false
    @Published private(set) var cleanup: PublisherCleanupPreview?
    @Published private(set) var gc: PublisherGCPreview?

    let bridge: DesktopBridge
    init(bridge: DesktopBridge) { self.bridge = bridge }

    var canEdit: Bool { bridge.edition == .personal && busy == nil && !recoveryHolding }

    private func beginAction(_ label: String, replacesOutcome: Bool = false) {
        busy = label
        error = nil
        if replacesOutcome { outcome = nil }
    }

    func reportFileError(_ message: String) {
        outcome = nil
        error = message
    }

    func load() async {
        guard canEdit, !dirty else { return }
        beginAction("正在读取本地草稿与发布历史…")
        do { try await reload() } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    private func reload() async throws {
        let freshDraft: AdManifest = try await bridge.read(.adDraftRead)
        let freshStatus: PublisherView = try await bridge.read(.publisherStatus)
        draft = freshDraft
        status = freshStatus
        dirty = false
        await loadPreviews()
    }

    private func loadPreviews() async {
        guard let draft else { return }
        let hashes = Set(draft.slots.flatMap(\.campaigns).map(\.image.sha256).filter { !$0.isEmpty })
        images = images.filter { hashes.contains($0.key) }
        for hash in hashes where images[hash] == nil {
            if let url: String = try? await bridge.read(.adImagePreview, arguments: ["sha256": .string(hash)]),
               let bytes = WorkspaceModel.imageData(url) { images[hash] = bytes }
        }
    }

    func previewAd(slot: String, period: Int) -> OverviewAd? {
        guard let item = WorkspaceModel.slots.first(where: { $0.id == slot }) else { return nil }
        let campaigns = draft?.slots.first(where: { $0.slot == slot })?.campaigns ?? []
        let state: OverviewAdState
        if campaigns.indices.contains(period) {
            let campaign = campaigns[period]
            state = .active(OverviewAdContent(campaign: campaign, imageData: images[campaign.image.sha256]))
        } else { state = .unavailable }
        return OverviewAd(id: item.id, name: item.name, symbol: item.symbol, state: state)
    }

    func editSlot(_ slot: String, _ change: (inout SlotContent) -> Void) {
        guard canEdit, var next = draft, let index = next.slots.firstIndex(where: { $0.slot == slot }) else { return }
        change(&next.slots[index])
        draft = next
        dirty = true
    }

    func editCampaign(_ slot: String, period: Int, _ change: (inout Campaign) -> Void) {
        editSlot(slot) { value in
            guard value.campaigns.indices.contains(period) else { return }
            change(&value.campaigns[period])
        }
    }

    func addCampaign(_ slot: String) {
        editSlot(slot) { value in
            guard value.campaigns.count < 2 else { return }
            let start = max(Int64(Date().timeIntervalSince1970), value.campaigns.first?.endsAt ?? 0)
            value.campaigns.append(Campaign(campaignId: UUID().uuidString.lowercased(), merchantId: "", merchantName: "", title: "", description: "",
                image: ImageAsset(sha256: "", mime: "image/png", bytes: 0, width: 0, height: 0),
                buttonText: "了解详情", targetUrl: "", startsAt: start, endsAt: start + 604_800, enabled: true))
        }
    }

    func removeCampaign(_ slot: String, period: Int) {
        editSlot(slot) { if $0.campaigns.indices.contains(period) { $0.campaigns.remove(at: period) } }
    }

    var validation: [String] {
        guard let draft else { return ["草稿尚未读取"] }
        var issues: [String] = []
        for slot in draft.slots {
            let name = WorkspaceModel.slots.first { $0.id == slot.slot }?.name ?? slot.slot
            for campaign in slot.campaigns {
                if [campaign.merchantId, campaign.merchantName, campaign.title, campaign.description, campaign.buttonText]
                    .contains(where: { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) {
                    issues.append("\(name)：请填写商家、标题、说明和按钮")
                }
                let url = URLComponents(string: campaign.targetUrl)
                if url?.scheme?.lowercased() != "https" || url?.host?.isEmpty != false || url?.user != nil || url?.password != nil || (url?.port != nil && url?.port != 443) {
                    issues.append("\(name)：需要有效的 HTTPS 商家地址")
                }
                if campaign.image.sha256.isEmpty || campaign.image.bytes <= 0 { issues.append("\(name)：请导入静态图片") }
                if campaign.endsAt <= campaign.startsAt { issues.append("\(name)：结束应晚于开始") }
            }
            if slot.campaigns.count > 2 { issues.append("\(name)：最多保存两期") }
            if slot.campaigns.count >= 2 {
                let a = slot.campaigns[0], b = slot.campaigns[1]
                if a.enabled && b.enabled && a.startsAt < b.endsAt && b.startsAt < a.endsAt {
                    issues.append("\(name)：两期时间重叠")
                }
            }
        }
        return Array(Set(issues)).sorted()
    }

    func save() async {
        guard canEdit, let draft else { return }
        beginAction("正在保存本地草稿…", replacesOutcome: true)
        do {
            self.draft = try await bridge.read(.adDraftSave, arguments: ["manifest": try bridge.argument(draft)])
            dirty = false
            outcome = OperationOutcome(completed: true, message: "草稿已保存到本机，尚未发布。", recovery: nil)
        } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func importImage(_ bytes: Data, slot: String, period: Int) async {
        guard canEdit, bytes.count <= 1_048_576, !bytes.isEmpty,
              draft?.slots.first(where: { $0.slot == slot })?.campaigns.indices.contains(period) == true else { return }
        beginAction("正在解码并核验素材…")
        do {
            let result: ImportedImage = try await bridge.read(.importAdImage, arguments: ["bytes": byteArguments(bytes)])
            if let data = WorkspaceModel.imageData(result.previewDataUrl) { images[result.asset.sha256] = data }
            busy = nil
            editCampaign(slot, period: period) { $0.image = result.asset }
        } catch { self.error = desktopErrorMessage(error); busy = nil }
    }

    func publish(_ channel: String, confirmed: Bool) async {
        guard canEdit, confirmed, !dirty, draft != nil, validation.isEmpty, ["preview", "production"].contains(channel) else { return }
        await outcomeAction(.publishAd, arguments: ["channel": .string(channel)], label: "正在签名、提交并回读发布内容…", refreshStatus: true)
    }

    func pause(_ slot: String?) async {
        guard canEdit, !dirty, slot == nil || WorkspaceModel.slots.contains(where: { $0.id == slot }) else { return }
        beginAction("正在建立暂停草稿…", replacesOutcome: true)
        do {
            draft = try await bridge.read(.pauseAd, arguments: ["slot": slot.map(JSONValue.string) ?? .null])
            dirty = false
            outcome = OperationOutcome(completed: true, message: "暂停草稿已创建，请核对后发布。", recovery: nil)
        } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func restoreHistory(_ releaseId: String) async {
        guard canEdit, !dirty, status?.history.contains(where: { $0.releaseId == releaseId }) == true else { return }
        beginAction("正在从历史建立新草稿…", replacesOutcome: true)
        do {
            draft = try await bridge.read(.restoreAdHistory, arguments: ["releaseId": .string(releaseId)])
            dirty = false
            await loadPreviews()
            outcome = OperationOutcome(completed: true, message: "已建立新草稿，尚未发布。", recovery: nil)
        } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func outcomeAction(_ command: DesktopCommand, arguments: [String: JSONValue] = [:], label: String, refreshStatus: Bool = false) async {
        guard canEdit else { return }
        beginAction(label, replacesOutcome: true)
        do {
            outcome = try await bridge.read(command, arguments: arguments)
            if refreshStatus { status = try await bridge.read(.publisherStatus) }
        } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func readRemote() async {
        guard canEdit else { return }
        beginAction("正在读取发布配置…")
        do { remote = try await bridge.read(.publisherRemoteRead) }
        catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func saveRemote(channel: String, config: RemoteConfig?, accessKey: String, secretKey: String) async {
        guard canEdit, remote != nil, ["preview", "production"].contains(channel),
              config == nil || (config?.channel == channel && !accessKey.isEmpty && !secretKey.isEmpty) else { return }
        beginAction("正在保存发布配置…", replacesOutcome: true)
        do {
            remote = try await bridge.read(.publisherRemoteSave, arguments: ["channel": .string(channel), "config": try config.map { try bridge.argument($0) } ?? .null,
                "accessKey": .string(accessKey), "secretKey": .string(secretKey)])
            outcome = OperationOutcome(completed: true, message: "配置已保存；此操作没有连接远程服务。", recovery: nil)
        } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func selectRemote(channel: String, target: String) async {
        guard canEdit, ["preview", "production"].contains(channel),
              remote?.targets.contains(where: { $0.target == target && $0.config.channel == channel && !$0.selected }) == true else { return }
        beginAction("正在切换已保留的目标…", replacesOutcome: true)
        do {
            remote = try await bridge.read(.publisherRemoteSelect, arguments: ["channel": .string(channel), "target": .string(target)])
            outcome = OperationOutcome(completed: true, message: "已选回发布目标；此操作没有连接远程服务。", recovery: nil)
        } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func prepareRecovery(passphrase: String) async {
        guard canEdit, !dirty, passphrase.count >= 12 else { return }
        recoveryHolding = true
        beginAction("正在保存并核验完整备份…")
        do { recoveryPreview = try await bridge.read(.publisherRecoveryPrepare, arguments: ["passphrase": .string(passphrase)]) }
        catch {
            self.error = desktopErrorMessage(error)
            do { _ = try await bridge.invoke(.publisherRecoveryCancel); recoveryHolding = false }
            catch { self.error = "恢复会话取消失败，请重试取消。" }
        }
        busy = nil
    }

    func cancelRecovery() async {
        guard recoveryHolding, busy == nil, !recoveryRefreshPending else { return }
        beginAction("正在取消恢复…", replacesOutcome: true)
        do {
            _ = try await bridge.invoke(.publisherRecoveryCancel)
            recoveryPreview = nil
            recoveryHolding = false
            outcome = OperationOutcome(completed: true, message: "恢复已取消，已生成的备份继续保留。", recovery: nil)
        } catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func confirmRecovery() async {
        guard recoveryHolding, busy == nil, let preview = recoveryPreview else { return }
        beginAction("正在确认恢复并重新读取资料…", replacesOutcome: true)
        recoveryFailure = nil
        recoveryPreview = nil
        do { outcome = try await bridge.read(.publisherRecoveryConfirm, arguments: ["digest": .string(preview.digest)]) }
        catch { recoveryFailure = desktopErrorMessage(error); self.error = recoveryFailure }
        await refreshAfterRecovery()
        busy = nil
    }

    func retryRecoveryRefresh() async {
        guard recoveryHolding, recoveryRefreshPending, busy == nil else { return }
        beginAction("正在重新读取恢复结果…")
        await refreshAfterRecovery()
        busy = nil
    }

    private func refreshAfterRecovery() async {
        do {
            try await reload()
            recoveryRefreshPending = false
            recoveryHolding = false
            error = recoveryFailure
            recoveryFailure = nil
        } catch {
            recoveryRefreshPending = true
            let refreshError = "读取最新发布资料失败，请重试。"
            self.error = recoveryFailure.map { "\($0)\n\(refreshError)" } ?? refreshError
        }
    }

    func readRetained() async {
        guard canEdit else { return }
        beginAction("正在读取保留资料…")
        do { retained = try await bridge.read(.publisherRetainedList) }
        catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func exportRetained(_ generation: String, passphrase: String) async {
        guard passphrase.count >= 12, retained.contains(where: { $0.generation == generation && $0.stateVerified }) else { return }
        await outcomeAction(.publisherRetainedExport, arguments: ["generation": .string(generation), "passphrase": .string(passphrase)], label: "正在加密导出保留资料…")
    }

    func importArchive(_ bytes: Data, passphrase: String) async {
        guard canEdit, !dirty, passphrase.count >= 12, !bytes.isEmpty, bytes.count <= 67_108_864 else { return }
        await outcomeAction(.publisherArchiveImport, arguments: ["bytes": byteArguments(bytes), "passphrase": .string(passphrase)], label: "正在导入加密归档…")
    }

    func exportBackup(passphrase: String) async {
        guard passphrase.count >= 12 else { return }
        await outcomeAction(.exportPublisherBackup, arguments: ["passphrase": .string(passphrase)], label: "正在生成加密运营备份…")
    }

    func restoreBackup(_ bytes: Data, passphrase: String) async {
        guard canEdit, !dirty, passphrase.count >= 12, !bytes.isEmpty, bytes.count <= 67_108_864 else { return }
        beginAction("正在恢复加密备份…", replacesOutcome: true)
        recoveryFailure = nil
        recoveryHolding = true
        do { outcome = try await bridge.read(.restorePublisherBackup, arguments: ["bytes": byteArguments(bytes), "passphrase": .string(passphrase)]) }
        catch { recoveryFailure = desktopErrorMessage(error); self.error = recoveryFailure }
        await refreshAfterRecovery()
        busy = nil
    }

    func clearMaintenance() { cleanup = nil; gc = nil }

    func previewCleanup(retain: Int, releaseResolution: Bool) async {
        guard canEdit, (0...256).contains(retain) else { return }
        clearMaintenance()
        beginAction("正在核对本机历史与素材引用…")
        do { cleanup = try await bridge.read(.previewPublisherCleanup, arguments: ["retain": .number(Double(retain)), "releaseResolution": .bool(releaseResolution)]) }
        catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func commitCleanup(retain: Int, releaseResolution: Bool) async {
        guard canEdit, let plan = cleanup, !plan.pendingBlocksCommit,
              plan.retain == retain, plan.releaseResolution == releaseResolution else { return }
        clearMaintenance()
        await outcomeAction(.commitPublisherCleanup, arguments: ["digest": .string(plan.digest), "retain": .number(Double(retain)), "releaseResolution": .bool(releaseResolution)], label: "正在提交已确认的索引整理…", refreshStatus: true)
    }

    func previewGC() async {
        guard canEdit else { return }
        clearMaintenance()
        beginAction("正在核对本批可回收文件…")
        do { gc = try await bridge.read(.previewPublisherGC) }
        catch { self.error = desktopErrorMessage(error) }
        busy = nil
    }

    func commitGC() async {
        guard canEdit, let plan = gc, !plan.pendingBlocksCommit else { return }
        clearMaintenance()
        await outcomeAction(.commitPublisherGC, arguments: ["digest": .string(plan.digest)], label: "正在回收已确认的文件…", refreshStatus: true)
    }

    private func byteArguments(_ data: Data) -> JSONValue {
        .string(data.base64EncodedString())
    }
}
