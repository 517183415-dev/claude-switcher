import Foundation

// The Rust core owns validation and security decisions. These types only decode
// the fields needed to present its existing IPC responses.
struct WorkspaceSnapshot: Decodable {
    let runtime: RuntimeInfo
    let accounts: AccountOverview
    var preferences: Preferences
    var checkReport: CheckReport?
    let adsManifest: AdManifest?
    let update: UpdateView
    let pendingMaterials: [String]
}

struct RuntimeInfo: Decodable {
    let version: String
    let edition: DesktopEdition
    let automaticUpdates: Bool
}

struct AccountOverview: Decodable {
    let current: String?
    let profiles: [AccountProfile]
    let actionBlocker: String?
}

struct AccountProfile: Decodable, Identifiable {
    let id: String
    let state: String
    let loginEvidence: String
}

struct Preferences: Codable {
    var ads: AdPreferences
    let updatesPausedUntil: Int64?
}

struct AdPreferences: Codable {
    var slotsEnabled: [Bool]
    var requestPurposeAcknowledged: Bool
}

struct CheckReport: Decodable {
    let completedAt: Int64
    let checks: [CheckResult]
    let summary: CheckSummary
}

struct CheckSummary: Decodable {
    let passed, advisory, blocking: Int
}

struct CheckResult: Decodable, Identifiable {
    let id, status, reason: String
    let observedAt, durationMs, validUntil: Int64
    let evidence: [String]
}

struct AdManifest: Codable {
    var schemaVersion: Int
    var channel: String
    var contentRevision: Int64
    var keyId: String
    var issuedAt, expiresAt: Int64
    var releaseId: String
    var slots: [SlotContent]
}

struct SlotContent: Codable {
    var slot: String
    var enabled: Bool
    var campaigns: [Campaign]
}

struct Campaign: Codable {
    var campaignId, merchantId, merchantName, title, description: String
    var image: ImageAsset
    var buttonText, targetUrl: String
    var startsAt, endsAt: Int64
    var enabled: Bool
}

struct ImageAsset: Codable {
    var sha256, mime: String
    var bytes, width, height: Int
}

struct UpdateView: Decodable {
    let paused: Bool?
    let message: String
    let nextCheckAt: Int64?
    let candidateVersion: String?
    let canDownload, canInstall: Bool
}

struct ActivityEntry: Decodable, Identifiable {
    let observedAt: Int64
    let level, message: String
    var id: String { "\(observedAt):\(level):\(message)" }
}

struct OperationOutcome: Decodable {
    let completed: Bool
    let message: String
    let recovery: String?
}

struct RegionView: Decodable {
    let country, timezone, locale: String
}

struct RepairPreview: Decodable {
    let canExecute: Bool
    let message: String
    let original, target: RegionView
    let networkService, originalIpv6, targetIpv6: String
    let userChanges: [RepairChange]
    let recoveryRequired: Bool
}

struct RepairChange: Decodable, Identifiable {
    let id, state, original, target, message: String
}

extension DesktopBridge {
    func read<T: Decodable>(_ command: DesktopCommand, arguments: [String: JSONValue] = [:]) async throws -> T {
        let value = try await invoke(command, arguments: arguments)
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(T.self, from: JSONEncoder().encode(value))
    }

    func argument<T: Encodable>(_ value: T) throws -> JSONValue {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        return try JSONDecoder().decode(JSONValue.self, from: encoder.encode(value))
    }
}

func desktopErrorMessage(_ error: Error) -> String {
    if case BridgeError.remote(let message) = error,
       message.unicodeScalars.contains(where: { (0x3400...0x9fff).contains($0.value) }) {
        return message
    }
    return "操作未完成。请检查桌面核心连接，或在活动记录中查看具体原因。"
}

func desktopRecoveryMessage(_ recovery: String?) -> String? {
    guard let recovery, !recovery.isEmpty else { return nil }
    switch recovery {
    case "wait_release_materials": return "请等待正式更新材料就绪，然后重新检查更新。"
    case "update_recovery_required": return "请核验已安装组件及兼容回退工件，再恢复更新事务。"
    case "clock_recovery_required": return "请校正系统时间，重新建立时间基线后重试。"
    case "lifecycle_busy": return "请等待当前任务完成后重试。"
    case "update_in_progress": return "请等待当前更新任务完成后重试。"
    default:
        return recovery.unicodeScalars.contains(where: { (0x3400...0x9fff).contains($0.value) })
            ? recovery : "请查看活动记录中的原因，完成处理后重试。"
    }
}
