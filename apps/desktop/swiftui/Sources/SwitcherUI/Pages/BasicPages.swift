import SwiftUI

struct AccountsPage: View {
    @ObservedObject var model: WorkspaceModel

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            pageHeading("账号档案", "选择本机账号，检查后自动切换并启动。")
            Label("档案与登录资料只在本机处理。登录状态仅按实际证据显示。", systemImage: "lock").notice()
            if let blocker = model.snapshot?.accounts.actionBlocker { Label(blocker, systemImage: "exclamationmark.triangle").notice() }
            Text("选择账号后自动检查、退出 Claude、切换并启动。首次切换会保存账号 A；首次使用 B 时打开登录界面。")
                .font(.system(size: 12)).foregroundStyle(.secondary)
            HStack(alignment: .top, spacing: 16) {
                ForEach(model.snapshot?.accounts.profiles ?? []) { profile in
                    VStack(alignment: .leading, spacing: 12) {
                        Text(profile.id).font(.title.bold()).foregroundStyle(.white)
                            .frame(width: 58, height: 58).background(SwitcherPalette.green.gradient, in: Circle())
                        Text("账号 \(profile.id)").font(.headline)
                        Text(profileLabel(profile.state)).font(.caption).foregroundStyle(SwitcherPalette.green)
                        Text(profile.loginEvidence).font(.caption).foregroundStyle(.secondary)
                        Button(profile.state == "current" ? "启动当前账号" : profile.state == "uninitialized" ? "首次使用此账号" : "切换并启动") {
                            let action = profile.id == "A" ? "select_a" : "select_b"
                            Task { await model.perform(.accountAction, arguments: ["action": .string(action)], label: "正在检查环境并切换启动…") }
                        }.disabled(model.busy != nil || profile.state == "recovery_pending" || !["A", "B"].contains(profile.id))
                    }.panel()
                }
            }
            if model.snapshot?.accounts.profiles.isEmpty != false {
                Text("尚未读取档案状态。连接核心后可查看已有 A/B 档案。").notice()
            }
            VStack(alignment: .leading, spacing: 10) {
                Text("恢复中断操作").font(.headline)
                Text("切换中断时保留账号资料，按记录恢复。").font(.caption).foregroundStyle(.secondary)
                Button("恢复中断事务") {
                    Task { await model.perform(.accountAction, arguments: ["action": .string("recover")], label: "正在恢复中断操作…") }
                }.disabled(model.busy != nil || model.snapshot == nil)
            }.panel()
        }
    }

    private func profileLabel(_ state: String) -> String {
        ["uninitialized": "未建立", "saved": "已保存", "current": "当前使用", "recovery_pending": "恢复待处理"][state] ?? "待确认"
    }
}

struct EnvironmentPage: View {
    @ObservedObject var model: WorkspaceModel
    @State private var expanded: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            pageHeading("环境检查", "查看实际证据，一键修复环境后重新检查。")
            HStack {
                if let summary = model.snapshot?.checkReport?.summary {
                    Text("\(summary.passed) 项通过 · \(summary.advisory) 项提示 · \(summary.blocking) 项待处理")
                } else { Text("14 项稳定检查 · 尚未取得报告") }
                Spacer()
                Button("重新检查") { Task { await model.runChecks() } }.disabled(model.busy != nil || model.snapshot == nil)
            }.font(.system(size: 12))
            if let report = model.snapshot?.checkReport {
                VStack(spacing: 0) {
                    ForEach(report.checks) { check in
                        VStack(alignment: .leading, spacing: 8) {
                            Button {
                                expanded = expanded == check.id ? nil : check.id
                            } label: {
                                HStack(alignment: .top) {
                                    Image(systemName: check.status == "pass" ? "checkmark.circle.fill" : "exclamationmark.circle")
                                        .foregroundStyle(check.status == "pass" ? SwitcherPalette.green : .orange)
                                    VStack(alignment: .leading) {
                                        Text(checkNames[check.id] ?? check.id).fontWeight(.semibold)
                                        Text(check.reason).foregroundStyle(.secondary)
                                    }
                                    Spacer()
                                    Text(WorkspaceModel.statusName(check.status)).foregroundStyle(.secondary)
                                    Image(systemName: expanded == check.id ? "minus" : "plus")
                                }
                            }.buttonStyle(.plain)
                            if expanded == check.id {
                                Text("观察时间：\(dateText(check.observedAt)) · 耗时：\(check.durationMs) ms · 有效期至：\(dateText(check.validUntil))")
                                    .foregroundStyle(.secondary)
                                ForEach(Array(check.evidence.enumerated()), id: \.offset) { _, evidence in Text(evidence) }
                                if check.evidence.isEmpty { Text("未取得更多证据。").foregroundStyle(.secondary) }
                            }
                        }.font(.system(size: 12)).padding(12)
                        Divider().opacity(0.4)
                    }
                }.switcherGlass(cornerRadius: 14)
                Text("必需项失败或缺失时，启动进入等待。401 表示需要认证；403 表示访问受限。")
                    .font(.caption).foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    Text("准备检查环境").font(.headline)
                    Text("检查返回完整 14 项报告。网络与 STUN 超时会单独标记。")
                    Button("开始只读检查") { Task { await model.runChecks() } }.disabled(model.busy != nil || model.snapshot == nil)
                }.panel()
            }
            RegionPanel(model: model, environment: true)
        }
    }
}

private let checkNames: [String: String] = [
    "proxy_process": "代理客户端", "tun_route": "隧道路由", "egress_region": "出口地区", "ip_continuity": "出口地址变化",
    "timezone": "时区", "locale": "地区格式", "ipv6_policy": "IPv6 策略", "dns_route": "DNS 路径", "stun_path": "STUN 路径",
    "guardian": "后台守护", "service_routes": "服务路由", "client_language": "客户端语言", "asn_info": "网络运营商信息", "service_transport": "服务传输与访问结果",
]

struct ActivityPage: View {
    @ObservedObject var model: WorkspaceModel
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            pageHeading("活动记录", "这里只显示脱敏后的本机操作记录。")
            HStack {
                Text("本机记录 · 与导出使用同一脱敏结果").font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("导出脱敏记录") { Task { await model.exportActivity() } }.disabled(model.busy != nil || model.activity.isEmpty)
            }
            if model.activity.isEmpty {
                Text(model.activityLoaded ? "暂无活动记录" : "正在读取记录").notice()
            } else {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(model.activity.enumerated()), id: \.offset) { _, entry in
                        HStack(alignment: .top, spacing: 14) {
                            Text(dateText(entry.observedAt)).foregroundStyle(.secondary).frame(width: 150, alignment: .leading)
                            Text(entry.message).frame(maxWidth: .infinity, alignment: .leading)
                            Text(entry.level == "error" ? "错误" : entry.level == "warning" ? "提示" : "记录").foregroundStyle(.secondary)
                        }.font(.system(size: 12)).padding(.vertical, 12)
                        Divider().opacity(0.4)
                    }
                }.panel()
            }
        }
    }
}

struct SettingsPage: View {
    @ObservedObject var model: WorkspaceModel
    @Binding var consentIndex: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            pageHeading("工作空间设置", "管理更新、广告和本地时间恢复。")
            RegionPanel(model: model, environment: false)
            VStack(alignment: .leading, spacing: 12) {
                HStack { Text("软件更新").font(.headline); Spacer(); Text(model.snapshot?.runtime.version ?? "—").font(.caption) }
                Text(model.snapshot?.update.message ?? "尚未取得更新状态。")
                if let next = model.snapshot?.update.nextCheckAt { Text("下次检查：\(dateText(next))").foregroundStyle(.secondary) }
                if let version = model.snapshot?.update.candidateVersion { Text("候选版本：\(version)") }
                HStack {
                    updateButton("检查更新", "check", disabled: model.snapshot == nil)
                    updateButton("下载候选", "download", disabled: model.snapshot?.update.canDownload != true)
                    updateButton("确认安装", "install", disabled: model.snapshot?.update.canInstall != true)
                    if model.snapshot?.runtime.automaticUpdates == true,
                       model.snapshot?.update.message.contains("源码构建版本") == false,
                       let paused = model.snapshot?.update.paused {
                        updateButton(paused ? "恢复自动检查" : "暂停自动检查", paused ? "resume" : "pause", disabled: false)
                        updateButton("延后安装", "defer", disabled: false)
                    }
                }
                Text("安装要求：Claude 完全退出、账号事务空闲、组件兼容，且工件通过全部身份验证。")
                    .foregroundStyle(.secondary)
            }.font(.system(size: 12)).panel()
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text("广告展示").font(.headline)
                    Spacer()
                    Button("刷新广告内容") { Task { await model.perform(.refreshAdContent, label: "正在刷新并验证广告内容…") } }
                        .disabled(model.busy != nil || model.snapshot == nil)
                }
                Text("开启后仅向固定内容源读取签名清单与对应素材。所有广告位关闭时，广告请求为零。")
                    .foregroundStyle(.secondary)
                ForEach(Array(WorkspaceModel.slots.enumerated()), id: \.offset) { index, slot in
                    Toggle(slot.name, isOn: Binding(get: {
                        guard let enabled = model.snapshot?.preferences.ads.slotsEnabled, enabled.indices.contains(index) else { return false }
                        return enabled[index]
                    }, set: { enabled in
                        if enabled && model.snapshot?.preferences.ads.requestPurposeAcknowledged != true { consentIndex = index }
                        else { Task { await model.setAdEnabled(index: index, enabled: enabled) } }
                    })).disabled(model.busy != nil || model.snapshot == nil)
                }
            }.font(.system(size: 12)).panel()
            VStack(alignment: .leading, spacing: 12) {
                Text("本地时间恢复").font(.headline)
                Text("发现时间回拨或跳变时，广告收起、软件安装暂停。校正系统时间后，重新建立时间基线。")
                    .foregroundStyle(.secondary)
                HStack {
                    Button("打开系统日期设置") { Task { await model.perform(.openDateSettings, label: "正在打开系统日期设置…") } }
                    Button("重新建立时间基线") { Task { await model.perform(.rebuildTimeBaseline, label: "正在重建时间基线…") } }
                }.disabled(model.busy != nil || model.snapshot == nil)
            }.font(.system(size: 12)).panel()
            if let materials = model.snapshot?.pendingMaterials, !materials.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("等待材料").font(.headline)
                    ForEach(materials, id: \.self) { Text($0).font(.caption) }
                }.panel()
            }
        }
    }

    private func updateButton(_ title: String, _ action: String, disabled: Bool) -> some View {
        Button(title) { Task { await model.perform(.softwareUpdate, arguments: ["action": .string(action)], label: "正在处理软件更新…") } }
            .disabled(model.busy != nil || disabled)
    }
}
