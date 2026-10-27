import Foundation

struct PublisherView: Decodable {
    let source: String?
    let status, message: String
    let history: [PublishRecord]
}

struct PublishRecord: Decodable, Identifiable {
    let releaseId: String
    let revision: Int64
    let channel: String
    let publishedAt: Int64
    let payloadHash: String
    let verified: Bool
    var id: String { "\(channel):\(releaseId):\(revision)" }
}

struct ImportedImage: Decodable {
    let asset: ImageAsset
    let previewDataUrl: String
}

struct PublisherRecoveryPreview: Decodable {
    let digest, backupFilename, generation: String
}

struct RetainedGeneration: Decodable, Identifiable {
    let generation: String
    let stateVerified: Bool
    let history: [JSONValue]
    let pendingReleaseId: String?
    let pendingTargetUnknown: Bool
    var id: String { generation }
}

struct RemoteConfig: Codable {
    var accountId, bucket, publicOrigin, channel: String
}

struct RemoteTarget: Decodable, Identifiable {
    let target: String
    let config: RemoteConfig
    let selected: Bool
    var id: String { target }
}

struct RemoteSettings: Decodable {
    let preview, production: RemoteConfig?
    let targets: [RemoteTarget]
    let unavailableTargets: [String]

    func config(for channel: String) -> RemoteConfig? {
        channel == "preview" ? preview : production
    }
}

struct PublisherCleanupPreview: Decodable {
    let digest: String
    let retain, historyBefore, historyAfter, assetsBefore, assetsAfter: Int
    let candidateBytes: Int64
    let pendingBlocksCommit, releaseResolution, replacesCheckpoint: Bool
    let retainedHistory, removedHistoryRecords, protectedAssets, candidateAssets: [String]
    let message: String
}

struct PublisherGCPreview: Decodable {
    let batchId, digest: String
    let eligible, alreadyMissing, blocked, referenced: [String]
    let eligibleBytes: Int64
    let pendingBlocksCommit: Bool
    let blockedDetails: [String]
    let message: String
}
