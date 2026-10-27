import SwiftUI

struct PublisherPage: View {
    @ObservedObject var model: PublisherModel
    @State private var selected = "residential_broadband"
    @State private var period = 0
    @State private var previewPeriod = 0
    @State private var previewWidth = 1180
    @State private var useUTC = false
    @State private var pendingChannel: String?
    @State private var passphrase = ""

    private var slot: SlotContent? { model.draft?.slots.first { $0.slot == selected } }
    private var campaign: Campaign? {
        guard let campaigns = slot?.campaigns, campaigns.indices.contains(period) else { return nil }
        return campaigns[period]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            pageHeading("广告运营", "三个固定广告位，独立编辑、排期和发布。")
            if let busy = model.busy { HStack { ProgressView().controlSize(.small); Text(busy) }.notice() }
            if let error = model.error {
                HStack { Label(error, systemImage: "exclamationmark.triangle"); Spacer(); Button("关闭") { model.error = nil } }.notice()
            }
            if let outcome = model.outcome {
                HStack {
                    VStack(alignment: .leading, spacing: 5) {
                        Text(outcome.message)
                        if let recovery = desktopRecoveryMessage(outcome.recovery) { Text(recovery) }
                    }
                    Spacer()
                    Button("关闭") { model.outcome = nil }
                }.notice()
            }
            VStack(alignment: .leading, spacing: 8) {
                Label(model.status?.source ?? "本机广告运营", systemImage: "lock").font(.headline)
                Text(model.status?.message ?? "广告签名私钥和远程写入凭据保存在本机钥匙串。")
                if model.status?.status == "wait_signing_key" { Text("尚未初始化签名密钥。") }
                Button("创建本机广告签名密钥") {
                    Task { await model.outcomeAction(.createAdSigningKey, label: "正在本机创建广告签名密钥…", refreshStatus: true) }
                }.disabled(!model.canEdit)
            }.font(.system(size: 12)).panel()
            PublisherRecoveryPanel(model: model)
            RemotePublisherPanel(model: model)
            if model.draft != nil {
                editor.disabled(!model.canEdit)
                previews
                publication
            }
            history
            PublisherMaintenancePanel(model: model)
            backup
        }
        .task { if model.draft == nil { await model.load() } }
        .alert("确认发布", isPresented: Binding(get: { pendingChannel != nil }, set: { if !$0 { pendingChannel = nil } })) {
            Button("取消", role: .cancel) { pendingChannel = nil }
            Button("确认发布") {
                let channel = pendingChannel
                pendingChannel = nil
                if let channel { Task { await model.publish(channel, confirmed: true) } }
            }
        } message: {
            Text("软件会按已保存的渠道来源签名、提交并回读。请先核对草稿与发布目标。")
        }
    }

    private var editor: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                VStack(alignment: .leading) {
                    Text("本地草稿 · 序号 \(model.draft?.contentRevision ?? 0)").font(.headline)
                    Text(model.dirty ? "有未保存修改" : "本地内容已读取").foregroundStyle(.secondary)
                }
                Spacer()
                Button("保存草稿") { Task { await model.save() } }
            }
            Picker("广告位", selection: $selected) {
                ForEach(WorkspaceModel.slots, id: \.id) { Text($0.name).tag($0.id) }
            }.pickerStyle(.segmented).onChange(of: selected) { _ in period = 0 }
            if let slot {
                HStack {
                    Toggle("允许此广告位展示", isOn: Binding(get: { self.slot?.enabled ?? false }, set: { value in model.editSlot(selected) { $0.enabled = value } }))
                    Spacer()
                    Button("暂停此位") { Task { await model.pause(selected) } }.disabled(model.dirty)
                    Button("全部暂停") { Task { await model.pause(nil) } }.disabled(model.dirty)
                }
                HStack {
                    ForEach(Array(slot.campaigns.enumerated()), id: \.offset) { index, _ in
                        Button(index == 0 ? "当前一期" : "下一期") { period = index }
                            .fontWeight(period == index ? .bold : .regular)
                    }
                    Button("＋ 添加一期") { model.addCampaign(selected); period = max(0, (self.slot?.campaigns.count ?? 1) - 1) }
                        .disabled(slot.campaigns.count >= 2)
                    Spacer()
                    Picker("编辑时区", selection: $useUTC) {
                        Text(TimeZone.current.identifier).tag(false)
                        Text("UTC").tag(true)
                    }.frame(width: 220)
                }
                if let campaign {
                    Grid(alignment: .leading, horizontalSpacing: 18, verticalSpacing: 12) {
                        GridRow { textField("商家名称", \.merchantName); textField("商家标识", \.merchantId) }
                        GridRow { textField("标题", \.title); textField("按钮文字", \.buttonText) }
                        GridRow { textField("说明", \.description); textField("商家 HTTPS 地址", \.targetUrl) }
                        GridRow {
                            DatePicker("开始时间", selection: dateField(\.startsAt), displayedComponents: [.date, .hourAndMinute])
                            DatePicker("结束时间", selection: dateField(\.endsAt), displayedComponents: [.date, .hourAndMinute])
                        }.environment(\.timeZone, useUTC ? TimeZone(secondsFromGMT: 0)! : .current)
                    }
                    HStack {
                        VStack(alignment: .leading) {
                            Text("静态 PNG / JPEG").fontWeight(.semibold)
                            Text(campaign.image.bytes > 0 ? "\(campaign.image.width) × \(campaign.image.height) · \((campaign.image.bytes + 1023) / 1024) KiB" : "请选择不超过 1 MiB 的图片。")
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button("导入图片") {
                            do {
                                if let bytes = try PublisherFilePicker.read(limit: 1_048_576, image: true) {
                                    let currentSlot = selected, currentPeriod = period
                                    Task { await model.importImage(bytes, slot: currentSlot, period: currentPeriod) }
                                }
                            } catch { model.reportFileError("图片读取失败或超过 1 MiB。") }
                        }
                    }
                    HStack {
                        Toggle("启用此期", isOn: boolField(\.enabled))
                        Spacer()
                        Button("移除此期草稿") { model.removeCampaign(selected, period: period); period = 0 }
                    }
                } else {
                    Text("此广告位还没有排期。每位可保存当前一期和下一期。")
                        .foregroundStyle(.secondary)
                }
            }
        }.font(.system(size: 12)).panel()
    }

    private func textField(_ title: String, _ key: WritableKeyPath<Campaign, String>) -> some View {
        VStack(alignment: .leading) {
            Text(title).fontWeight(.semibold)
            TextField(title, text: Binding(get: { campaign?[keyPath: key] ?? "" }, set: { value in
                model.editCampaign(selected, period: period) { $0[keyPath: key] = value }
            })).textFieldStyle(.roundedBorder)
        }
    }

    private func boolField(_ key: WritableKeyPath<Campaign, Bool>) -> Binding<Bool> {
        Binding(get: { campaign?[keyPath: key] ?? false }, set: { value in model.editCampaign(selected, period: period) { $0[keyPath: key] = value } })
    }

    private func dateField(_ key: WritableKeyPath<Campaign, Int64>) -> Binding<Date> {
        Binding(get: { Date(timeIntervalSince1970: TimeInterval(campaign?[keyPath: key] ?? 0)) }, set: { value in
            model.editCampaign(selected, period: period) { $0[keyPath: key] = Int64(value.timeIntervalSince1970) }
        })
    }

    private var previews: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading) { Text("本机草稿预览").font(.headline); Text("展示草稿，不打开链接，也不代表已发布。") }
                Spacer()
                Picker("期次", selection: $previewPeriod) { Text("当前一期").tag(0); Text("下一期").tag(1) }.frame(width: 130)
                Picker("窗口", selection: $previewWidth) { Text("1180 × 800").tag(1180); Text("960 × 680").tag(960) }.frame(width: 150)
            }
            HStack(alignment: .top, spacing: 12) {
                ForEach(WorkspaceModel.slots, id: \.id) { item in
                    let content = model.draft?.slots.first { $0.slot == item.id }
                    let campaigns = content?.campaigns ?? []
                    let current = campaigns.indices.contains(previewPeriod) ? campaigns[previewPeriod] : nil
                    VStack(alignment: .leading, spacing: 8) {
                        if let card = model.previewAd(slot: item.id, period: previewPeriod) {
                            AdCard(ad: card, onOpen: nil)
                        }
                        if let current {
                            Text(content?.enabled == true && current.enabled ? "草稿已启用" : "草稿已暂停").foregroundStyle(.secondary)
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
            }.frame(maxWidth: previewWidth == 960 ? 704 : 844)
        }.font(.system(size: 12)).panel()
    }

    private var publication: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("签名发布与回读").font(.headline)
            Text("软件按渠道配置自动上传、核对版本并验证回读。")
            ForEach(model.validation, id: \.self) { Label($0, systemImage: "exclamationmark.circle").foregroundStyle(.orange) }
            HStack {
                Button("发布到预览渠道") { pendingChannel = "preview" }
                Button("发布到正式渠道") { pendingChannel = "production" }
            }.disabled(!model.canEdit || model.dirty || !model.validation.isEmpty)
        }.font(.system(size: 12)).panel()
    }

    private var history: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("发布历史与恢复").font(.headline)
            Text("恢复历史会建立新草稿。确认发布后才会改变线上内容。")
            if model.status?.history.isEmpty != false { Text("暂无发布历史").foregroundStyle(.secondary) }
            ForEach(model.status?.history ?? []) { record in
                HStack {
                    VStack(alignment: .leading) {
                        Text("\(record.channel == "preview" ? "预览" : "正式") · 序号 \(record.revision) · \(record.verified ? "已核验" : "待核验")")
                        Text("\(record.releaseId) · \(dateText(record.publishedAt))").foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("恢复为草稿") { Task { await model.restoreHistory(record.releaseId) } }
                        .disabled(!model.canEdit || model.dirty)
                }
            }
        }.font(.system(size: 12)).panel()
    }

    private var backup: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("加密备份与恢复").font(.headline)
            Text("备份包含草稿、历史、素材索引、发布配置和加密密钥。恢复前须先结束本机回收批次。")
            SecureField("备份口令，至少 12 个字符", text: $passphrase).textFieldStyle(.roundedBorder).frame(maxWidth: 360)
            HStack {
                Button("导出加密备份") {
                    let secret = passphrase; passphrase = ""
                    Task { await model.exportBackup(passphrase: secret) }
                }.disabled(!model.canEdit || passphrase.count < 12)
                Button("打开备份目录") { Task { await model.outcomeAction(.openPublisherBackupFolder, label: "正在打开备份目录…") } }.disabled(!model.canEdit)
                Button("恢复加密备份") {
                    let secret = passphrase; passphrase = ""
                    do { if let data = try PublisherFilePicker.read(limit: 67_108_864) { Task { await model.restoreBackup(data, passphrase: secret) } } }
                    catch { model.reportFileError("备份读取失败或超过 64 MiB。") }
                }.disabled(!model.canEdit || model.dirty || passphrase.count < 12)
            }
        }.font(.system(size: 12)).panel().onDisappear { passphrase = "" }
    }
}
