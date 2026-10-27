import SwiftUI

struct PublisherMaintenancePanel: View {
    @ObservedObject var model: PublisherModel
    @State private var retain = 128
    @State private var releaseResolution = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("本机历史与素材整理").font(.headline)
            Text("先核对索引整理，再单独预览和确认文件回收。")
            Stepper("保留历史条数：\(retain)", value: $retain, in: 0...256)
                .onChange(of: retain) { _ in model.clearMaintenance() }
            Toggle("释放上次未验证发布的处理记录槽", isOn: $releaseResolution)
                .onChange(of: releaseResolution) { _ in model.clearMaintenance() }
            HStack {
                Button("预览索引整理") { Task { await model.previewCleanup(retain: retain, releaseResolution: releaseResolution) } }
                Button("预览本批文件回收") { Task { await model.previewGC() } }
            }
            if let preview = model.cleanup {
                VStack(alignment: .leading, spacing: 7) {
                    Text(preview.message)
                    Text("历史：\(preview.historyBefore) → \(preview.historyAfter)；素材索引：\(preview.assetsBefore) → \(preview.assetsAfter)；候选 \(preview.candidateBytes) 字节。")
                    if preview.replacesCheckpoint { Text("本次将替换已有整理检查点。") }
                    if preview.pendingBlocksCommit { Text("存在待处理事务，本次不能提交。") }
                    entries("保留历史", preview.retainedHistory)
                    entries("移除历史记录", preview.removedHistoryRecords)
                    entries("受保护素材", preview.protectedAssets)
                    entries("候选素材", preview.candidateAssets)
                    Button("确认整理索引") { Task { await model.commitCleanup(retain: retain, releaseResolution: releaseResolution) } }
                        .disabled(preview.pendingBlocksCommit || preview.retain != retain || preview.releaseResolution != releaseResolution)
                }.notice()
            }
            if let preview = model.gc {
                VStack(alignment: .leading, spacing: 7) {
                    Text(preview.message)
                    Text("批次：\(preview.batchId)；可回收 \(preview.eligibleBytes) 字节。")
                    if preview.pendingBlocksCommit { Text("存在待处理事务，本次不能回收。") }
                    entries("可回收", preview.eligible)
                    entries("已不存在", preview.alreadyMissing)
                    entries("校验保留", preview.blockedDetails)
                    entries("引用保留", preview.referenced)
                    Button("确认回收本批文件") { Task { await model.commitGC() } }.disabled(preview.pendingBlocksCommit)
                }.notice()
            }
        }.font(.system(size: 12)).panel().disabled(!model.canEdit)
    }

    @ViewBuilder private func entries(_ label: String, _ items: [String]) -> some View {
        if !items.isEmpty {
            DisclosureGroup("\(label)（\(items.count)）") {
                ForEach(Array(items.enumerated()), id: \.offset) { _, item in Text(item).textSelection(.enabled) }
            }
        }
    }
}
