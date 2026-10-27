import Combine
import Foundation

/// Coordinates presentation state. Security and persistence stay in the Rust core.
@MainActor
final class WorkspaceModel: ObservableObject {
    @Published private(set) var snapshot: WorkspaceSnapshot?
    @Published private(set) var activity: [ActivityEntry] = []
    @Published private(set) var activityLoaded = false
    @Published private(set) var busy: String?
    @Published private(set) var canCancelCheck = false
    @Published var error: String?
    @Published var outcome: OperationOutcome?
    @Published var adConfirmation: AdConfirmation?
    @Published private(set) var region: RegionView?
    @Published private(set) var repairPreview: RepairPreview?
    @Published private(set) var adImages: [String: Data] = [:]

    struct AdConfirmation: Identifiable {
        let slot: String
        let message: String
        var id: String { slot }
    }

    let bridge: DesktopBridge
    private var checkEpoch = 0
    private var suppressReport = false
    private var repairEpoch = 0
    private var repairRequest: String?
    private var repairRegion: String?
    private var repairOwner: String?
    private var repairExecuting = false
    private var adImagesInFlight: Set<String> = []

    init(bridge: DesktopBridge) { self.bridge = bridge }

    func connect() async {
        guard busy == nil else { return }
        busy = "正在连接核心…"
        do {
            try await refresh()
            busy = nil
            await runChecks()
        } catch {
            busy = nil
            self.error = message(error)
        }
    }

    func refresh() async throws {
        let epoch = checkEpoch
        var value: WorkspaceSnapshot = try await bridge.read(.workspaceSnapshot)
        guard epoch == checkEpoch else { return }
        guard value.runtime.edition == bridge.edition else { throw BridgeError.invalidResponse }
        if suppressReport { value.checkReport = nil }
        snapshot = value
        await loadAdImages()
    }

    private func clearReport() {
        suppressReport = true
        snapshot?.checkReport = nil
    }

    private func beginAction(_ label: String) {
        busy = label
        error = nil
        outcome = nil
    }

    func runChecks() async {
        guard busy == nil, snapshot != nil else { return }
        checkEpoch += 1
        let epoch = checkEpoch
        clearReport()
        beginAction("正在检查 14 项环境…")
        canCancelCheck = true
        defer {
            if epoch == checkEpoch { busy = nil; canCancelCheck = false }
        }
        do {
            let report: CheckReport = try await bridge.read(.checkEnvironment)
            guard epoch == checkEpoch else { return }
            suppressReport = false
            snapshot?.checkReport = report
            outcome = OperationOutcome(completed: true, message: "检查完成：\(report.summary.passed) 项通过 · \(report.summary.advisory) 项提示 · \(report.summary.blocking) 项阻止启动。", recovery: nil)
        } catch {
            guard epoch == checkEpoch else { return }
            self.error = message(error)
            clearReport()
        }
    }

    func cancelChecks() async {
        guard canCancelCheck else { return }
        checkEpoch += 1
        let epoch = checkEpoch
        clearReport()
        canCancelCheck = false
        busy = "正在取消本次检查…"
        defer { if epoch == checkEpoch { busy = nil } }
        do {
            let result: OperationOutcome = try await bridge.read(.cancelCheck)
            guard epoch == checkEpoch else { return }
            outcome = result
            try await refresh()
        } catch {
            guard epoch == checkEpoch else { return }
            self.error = message(error)
        }
    }

    func perform(_ command: DesktopCommand, arguments: [String: JSONValue] = [:], label: String) async {
        guard busy == nil, snapshot != nil else { return }
        let invalidatesReport = command == .launchChecked || command == .accountAction
        if invalidatesReport { checkEpoch += 1; clearReport() }
        beginAction(label)
        defer { busy = nil }
        do {
            let result: OperationOutcome = try await bridge.read(command, arguments: arguments)
            outcome = result
            if invalidatesReport { suppressReport = false }
            try await refresh()
        } catch { self.error = message(error) }
    }

    func loadActivity() async {
        do {
            let entries: [ActivityEntry] = try await bridge.read(.activityLog)
            activity = entries
            activityLoaded = true
        } catch { self.error = message(error) }
    }

    func exportActivity() async {
        guard busy == nil, !activity.isEmpty else { return }
        beginAction("正在导出本机脱敏记录…")
        defer { busy = nil }
        do {
            let result = try await bridge.invoke(.exportActivity)
            guard case .array(let values) = result, values.count == 2,
                  case .string(let path) = values[0] else { throw BridgeError.invalidResponse }
            let decoder = JSONDecoder()
            decoder.keyDecodingStrategy = .convertFromSnakeCase
            activity = try decoder.decode([ActivityEntry].self, from: JSONEncoder().encode(values[1]))
            outcome = OperationOutcome(completed: true, message: "脱敏记录已保存到本机：\(path)", recovery: nil)
        } catch { self.error = message(error) }
    }

    func setAdEnabled(index: Int, enabled: Bool, acknowledged: Bool = false) async {
        guard busy == nil, var preferences = snapshot?.preferences,
              preferences.ads.slotsEnabled.count == Self.slots.count,
              preferences.ads.slotsEnabled.indices.contains(index) else { return }
        guard !enabled || preferences.ads.requestPurposeAcknowledged || acknowledged else { return }
        preferences.ads.slotsEnabled[index] = enabled
        if acknowledged { preferences.ads.requestPurposeAcknowledged = true }
        beginAction("正在保存本机设置…")
        defer { busy = nil }
        do {
            let saved: Preferences = try await bridge.read(.savePreferences, arguments: ["preferences": try bridge.argument(preferences)])
            snapshot?.preferences = saved
            await loadAdImages()
        } catch { self.error = message(error) }
    }

    func openAd(slot: String, confirmed: Bool) async {
        guard busy == nil, Self.slots.contains(where: { $0.id == slot }) else { return }
        beginAction("正在核验商家链接…")
        defer { busy = nil }
        do {
            let result: OperationOutcome = try await bridge.read(.openAdLink, arguments: ["slot": .string(slot), "confirmed": .bool(confirmed)])
            if result.recovery == "confirm_ad_link", !confirmed {
                adConfirmation = AdConfirmation(slot: slot, message: result.message)
            } else { adConfirmation = nil; outcome = result }
        } catch { self.error = message(error) }
    }

    func loadRegion() async {
        let epoch = repairEpoch
        do {
            let result: RegionView = try await bridge.read(.regionView)
            if epoch == repairEpoch { region = result }
        } catch { if epoch == repairEpoch { self.error = message(error) } }
    }

    func previewRepair(region: String? = nil, autoExecute: Bool = false) async {
        guard busy == nil else { return }
        await cancelRepairPlan()
        guard busy == nil else { return }
        repairEpoch += 1
        let epoch = repairEpoch
        let request = UUID().uuidString
        repairRequest = request
        repairRegion = region
        repairOwner = request
        busy = "正在读取系统原值并预览修改…"
        error = nil
        defer { finishRepair(request) }
        do {
            var args: [String: JSONValue] = ["requestId": .string(request)]
            if let region { args["region"] = .string(region) }
            let preview: RepairPreview = try await bridge.read(region == nil ? .previewRepair : .previewRegionChange, arguments: args)
            guard epoch == repairEpoch else {
                _ = try? await bridge.invoke(.cancelRepair, arguments: ["requestId": .string(request)])
                return
            }
            repairPreview = preview
            if autoExecute && preview.canExecute && !preview.recoveryRequired {
                await executeRepair(request: request, epoch: epoch)
            }
        } catch {
            if epoch == repairEpoch { self.error = message(error); repairRequest = nil }
            _ = try? await bridge.invoke(.cancelRepair, arguments: ["requestId": .string(request)])
        }
    }

    func confirmRepair() async {
        guard busy == nil, let request = repairRequest, let preview = repairPreview,
              preview.canExecute, !preview.recoveryRequired else { return }
        repairOwner = request
        busy = "正在核验并执行已确认的计划…"
        await executeRepair(request: request, epoch: repairEpoch)
        finishRepair(request)
    }

    private func executeRepair(request: String, epoch: Int) async {
        repairRequest = nil
        repairPreview = nil
        repairExecuting = true
        checkEpoch += 1
        clearReport()
        busy = "正在核验并执行已确认的计划…"
        var completed = false
        do {
            let result: OperationOutcome = try await bridge.read(.executeRepair, arguments: ["requestId": .string(request)])
            guard epoch == repairEpoch else { repairExecuting = false; return }
            outcome = result
            completed = result.completed
            await loadRegion()
        } catch { if epoch == repairEpoch { self.error = message(error) } }
        repairExecuting = false
        finishRepair(request)
        if completed && epoch == repairEpoch { await runChecks() }
    }

    func cancelRepairPlan() async {
        repairEpoch += 1
        let request = repairRequest
        repairRequest = nil
        repairRegion = nil
        repairPreview = nil
        if let request {
            _ = try? await bridge.invoke(.cancelRepair, arguments: ["requestId": .string(request)])
            if !repairExecuting { finishRepair(request) }
        }
    }

    func recoverRepair() async {
        guard busy == nil, repairPreview?.recoveryRequired == true else { return }
        let selectedRegion = repairRegion
        let owner = UUID().uuidString
        repairOwner = owner
        repairExecuting = true
        busy = "正在恢复修复事务…"
        checkEpoch += 1
        clearReport()
        let epoch = repairEpoch
        var recovered = false
        do {
            let result: OperationOutcome = try await bridge.read(.recoverRepair)
            if epoch == repairEpoch {
                outcome = result
                if result.completed { recovered = true; await loadRegion() }
            }
        } catch { if epoch == repairEpoch { self.error = message(error) } }
        repairExecuting = false
        finishRepair(owner)
        if recovered && epoch == repairEpoch {
            await cancelRepairPlan()
            await previewRepair(region: selectedRegion)
        }
    }

    private var eligibleAdImages: [(slot: String, sha256: String)] {
        guard let snapshot, snapshot.preferences.ads.requestPurposeAcknowledged,
              let manifest = snapshot.adsManifest else { return [] }
        let now = Int64(Date().timeIntervalSince1970)
        guard manifest.expiresAt > now else { return [] }
        return Self.slots.enumerated().compactMap { index, slot in
            guard snapshot.preferences.ads.slotsEnabled.indices.contains(index),
                  snapshot.preferences.ads.slotsEnabled[index],
                  let content = manifest.slots.first(where: { $0.slot == slot.id && $0.enabled }),
                  let campaign = content.campaigns.first(where: { $0.enabled && $0.startsAt <= now && $0.endsAt > now }),
                  !campaign.image.sha256.isEmpty else { return nil }
            return (slot.id, campaign.image.sha256)
        }
    }

    private func loadAdImages() async {
        let eligible = eligibleAdImages
        let keys = Set(eligible.map { "\($0.slot):\($0.sha256)" })
        adImages = adImages.filter { keys.contains($0.key) }
        for item in eligible {
            let key = "\(item.slot):\(item.sha256)"
            guard adImages[key] == nil, !adImagesInFlight.contains(key) else { continue }
            adImagesInFlight.insert(key)
            defer { adImagesInFlight.remove(key) }
            guard let url: String = try? await bridge.read(.adCachedImage, arguments: ["slot": .string(item.slot), "sha256": .string(item.sha256)]),
                  eligibleAdImages.contains(where: { $0.slot == item.slot && $0.sha256 == item.sha256 }),
                  let data = Self.imageData(url) else { continue }
            adImages[key] = data
        }
    }

    static func imageData(_ url: String) -> Data? {
        let prefixes = ["data:image/png;base64,", "data:image/jpeg;base64,"]
        guard let prefix = prefixes.first(where: { url.hasPrefix($0) }),
              url.utf8.count <= 1_500_000,
              let data = Data(base64Encoded: String(url.dropFirst(prefix.count))),
              !data.isEmpty, data.count <= 1_048_576 else { return nil }
        return data
    }

    private func finishRepair(_ owner: String) {
        guard repairOwner == owner else { return }
        repairOwner = nil
        busy = nil
    }

    private func message(_ error: Error) -> String {
        desktopErrorMessage(error)
    }

    static let slots = [
        (id: "residential_broadband", name: "美国家庭宽带", symbol: "globe"),
        (id: "account_service", name: "注册好的账号", symbol: "person"),
        (id: "kyc_service", name: "KYC 服务", symbol: "lock"),
    ]

    var overview: OverviewData {
        let report = snapshot?.checkReport
        let issue = report?.checks.first { $0.status != "pass" }
        let groups: [(String, String, String, [String])] = [
            ("网络连接", "代理 · TUN · DNS · STUN", "globe", ["proxy_process", "tun_route", "dns_route", "stun_path", "service_transport"]),
            ("地区设置", "出口地区 · 时区 · 地区格式", "mappin.circle", ["egress_region", "timezone", "locale", "client_language"]),
            ("后台服务", "守护进程 · 服务路由", "gearshape", ["guardian", "service_routes"]),
        ]
        let summaries = groups.map { title, detail, symbol, ids in
            let checks = report?.checks.filter { ids.contains($0.id) } ?? []
            let bad = checks.first { $0.status != "pass" }
            let good = report != nil && Set(checks.map(\.id)) == Set(ids) && bad == nil
            return OverviewGroup(title: title, detail: detail, symbol: symbol,
                                 status: good ? "通过" : (bad.map { Self.statusName($0.status) } ?? "尚未检查"),
                                 reason: good ? "本次证据已取得，启动前将重新核验" : (bad?.reason ?? "取得实际证据后显示结果"), good: good)
        }
        let now = Int64(Date().timeIntervalSince1970)
        let ads = Self.slots.enumerated().map { index, slot in
            let enabled = snapshot?.preferences.ads.slotsEnabled.indices.contains(index) == true && snapshot?.preferences.ads.slotsEnabled[index] == true
            let content = snapshot?.preferences.ads.requestPurposeAcknowledged == true ? snapshot?.adsManifest.flatMap { manifest in
                manifest.expiresAt > now ? manifest.slots.first { $0.slot == slot.id && $0.enabled } : nil
            } : nil
            let campaign = content?.campaigns.first { $0.enabled && $0.startsAt <= now && $0.endsAt > now }
            let state: OverviewAdState
            if !enabled { state = .disabled }
            else if let campaign {
                state = .active(OverviewAdContent(campaign: campaign, imageData: adImages["\(slot.id):\(campaign.image.sha256)"]))
            } else { state = .unavailable }
            return OverviewAd(id: slot.id, name: slot.name, symbol: slot.symbol, state: state)
        }
        let current = snapshot?.accounts.current
        return OverviewData(account: current.map { "账号 \($0)" } ?? "尚未关联",
                            accountDetail: current == nil ? "首次切换时自动保存账号 A" : "已选择 · 本地档案",
                            passed: report?.summary.passed, total: 14,
                            summary: report.map { "\($0.summary.passed) 项通过 · \($0.summary.advisory) 项提示 · \($0.summary.blocking) 项待处理" } ?? "尚未检查 · 等待实际证据",
                            notice: issue?.reason, groups: summaries, ads: ads,
                            launchEnabled: snapshot != nil && busy == nil)
    }

    static func statusName(_ status: String) -> String {
        ["pass": "通过", "warning": "提示", "fail": "未通过", "unknown": "待确认", "timeout": "超时"][status] ?? "待确认"
    }
}
