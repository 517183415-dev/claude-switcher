import SwiftUI

struct PublisherRecoveryPanel: View {
    @ObservedObject var model: PublisherModel
    @State private var passphrase = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("旧发布资料恢复").font(.headline)
            Text("软件先保存并核验完整加密归档，再等待确认。旧目录、历史、事务和密钥会保留。")
            if model.recoveryRefreshPending {
                Text("恢复结果尚未重新读取。发布操作继续锁定。")
                Button("重新读取恢复结果") { Task { await model.retryRecoveryRefresh() } }.disabled(model.busy != nil)
            } else if let preview = model.recoveryPreview {
                Text("完整备份已写入并读回校验：\(preview.backupFilename)")
                Text("取消会释放锁，并保留备份。确认后请核对新草稿，再明确发布。")
                HStack {
                    Button("取消恢复") { Task { await model.cancelRecovery() } }
                    Button("确认保留旧资料并建立新代次") { Task { await model.confirmRecovery() } }
                }.disabled(model.busy != nil)
            } else if model.recoveryHolding {
                Text("恢复会话仍被锁定。请取消会话后重试。")
                Button("重试取消恢复") { Task { await model.cancelRecovery() } }.disabled(model.busy != nil)
            } else {
                SecureField("完整归档口令，至少 12 个字符", text: $passphrase).textFieldStyle(.roundedBorder).frame(maxWidth: 360)
                HStack {
                    Button("保存完整备份并准备恢复") {
                        let secret = passphrase; passphrase = ""
                        Task { await model.prepareRecovery(passphrase: secret) }
                    }.disabled(!model.canEdit || model.dirty || passphrase.count < 12)
                    Button("导入完整归档到保留目录") {
                        let publisher = model
                        do {
                            try PublisherFilePicker.withArchive(passphrase: &passphrase) { data, secret in
                                Task { await publisher.importArchive(data, passphrase: secret) }
                            }
                        }
                        catch { model.reportFileError("归档读取失败或超过 64 MiB。") }
                    }.disabled(!model.canEdit || model.dirty || passphrase.count < 12)
                    Button("读取保留资料") { Task { await model.readRetained() } }.disabled(!model.canEdit)
                }
                ForEach(model.retained) { item in
                    HStack {
                        Text("\(item.generation)：\(item.stateVerified ? "索引已核对，\(item.history.count) 条历史" : "索引尚未通过核对")\(item.pendingReleaseId == nil ? "" : "；含待定事务")\(item.pendingTargetUnknown ? "；原目标不明" : "")")
                        Spacer()
                        Button("加密导出") {
                            let secret = passphrase; passphrase = ""
                            Task { await model.exportRetained(item.generation, passphrase: secret) }
                        }.disabled(!model.canEdit || !item.stateVerified || passphrase.count < 12)
                    }
                }
            }
        }.font(.system(size: 12)).panel().onDisappear { passphrase = "" }
    }
}
