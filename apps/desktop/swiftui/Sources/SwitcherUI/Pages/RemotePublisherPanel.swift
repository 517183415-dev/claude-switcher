import SwiftUI

struct RemotePublisherPanel: View {
    @ObservedObject var model: PublisherModel
    @State private var channel = "preview"
    @State private var remote = false
    @State private var account = ""
    @State private var bucket = ""
    @State private var origin = ""
    @State private var accessKey = ""
    @State private var secretKey = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading) {
                    Text("发布来源").font(.headline)
                    Text("预览和正式渠道分别设置。R2 配置与写入凭据保存在本机钥匙串。")
                }
                Spacer()
                Button("读取已保存配置") { Task { await model.readRemote(); fill() } }.disabled(!model.canEdit)
            }
            if let settings = model.remote {
                HStack {
                    Picker("渠道", selection: $channel) { Text("预览").tag("preview"); Text("正式").tag("production") }.frame(width: 170)
                    Picker("发布来源", selection: $remote) { Text("本机目录").tag(false); Text("Cloudflare R2").tag(true) }.frame(width: 220)
                }.onChange(of: channel) { _ in fill() }
                Text("已保存的来源：\(settings.config(for: channel)?.publicOrigin ?? "本机目录")").foregroundStyle(.secondary)
                if remote {
                    Grid(alignment: .leading, horizontalSpacing: 18, verticalSpacing: 10) {
                        GridRow { TextField("Cloudflare 账号 ID", text: $account); TextField("存储桶名称", text: $bucket) }
                        GridRow { TextField("公开域名", text: $origin); Text("") }
                        GridRow { SecureField("访问密钥 ID", text: $accessKey); SecureField("访问密钥", text: $secretKey) }
                    }.textFieldStyle(.roundedBorder)
                    Text("两个渠道需要不同的存储桶和访问密钥。修改配置时请重新输入密钥。")
                }
                Button("保存该渠道配置") {
                    let selected = channel
                    let config = remote ? RemoteConfig(accountId: account.trimmingCharacters(in: .whitespacesAndNewlines), bucket: bucket.trimmingCharacters(in: .whitespacesAndNewlines), publicOrigin: origin.trimmingCharacters(in: .whitespacesAndNewlines), channel: selected) : nil
                    let access = accessKey, secret = secretKey
                    accessKey = ""; secretKey = ""
                    Task { await model.saveRemote(channel: selected, config: config, accessKey: access, secretKey: secret); fill() }
                }.disabled(!model.canEdit || (remote && (account.isEmpty || bucket.isEmpty || origin.isEmpty || accessKey.isEmpty || secretKey.isEmpty)))
                Text("更换来源或选择本机目录会保留旧目标及其凭据。此处保存配置不会连接远程服务。")
                    .foregroundStyle(.secondary)
                ForEach(settings.targets.filter { $0.config.channel == channel && !$0.selected }) { item in
                    HStack {
                        Text("\(item.config.bucket) · \(item.config.publicOrigin)")
                        Button("选回此目标") { Task { await model.selectRemote(channel: channel, target: item.target); fill() } }.disabled(!model.canEdit)
                    }
                }
                if !settings.unavailableTargets.isEmpty {
                    Label("有 \(settings.unavailableTargets.count) 个旧发布目标缺少访问配置。核对前无法执行多目标清理。", systemImage: "exclamationmark.triangle")
                }
            }
        }.font(.system(size: 12)).panel().disabled(!model.canEdit).onDisappear { clearSecrets() }
    }

    private func fill() {
        let config = model.remote?.config(for: channel)
        remote = config != nil
        account = config?.accountId ?? ""
        bucket = config?.bucket ?? ""
        origin = config?.publicOrigin ?? ""
        clearSecrets()
    }

    private func clearSecrets() { accessKey = ""; secretKey = "" }
}
