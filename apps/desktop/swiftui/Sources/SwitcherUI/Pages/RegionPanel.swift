import SwiftUI

struct RegionPanel: View {
    @ObservedObject var model: WorkspaceModel
    let environment: Bool
    @State private var selected = "us"

    private let options = [("us", "美国"), ("jp", "日本"), ("tw", "台湾"), ("hk", "香港"), ("sg", "新加坡"),
                           ("gb", "英国"), ("de", "德国"), ("fr", "法国"), ("kr", "韩国"), ("au", "澳大利亚"), ("ca", "加拿大")]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(environment ? "修复计划" : "目标地区").font(.headline)
            if !environment {
                Text("选择明确的目标国家。程序使用固定国家、时区与地区格式组合。")
                    .foregroundStyle(.secondary)
                if let region = model.region {
                    Text("当前锁定国家：\(region.country) · 目标时区：\(region.timezone) · 地区格式：\(region.locale)")
                } else { Text("尚未读取地区锁定，连接核心后重试。").foregroundStyle(.secondary) }
                Picker("新目标地区", selection: $selected) {
                    ForEach(options, id: \.0) { id, label in Text(label).tag(id) }
                }.frame(maxWidth: 250).disabled(model.busy != nil)
                Text("更换国家需要先切换到对应代理节点。系统修复成功后，核心才会保存新的地区配置。")
                    .foregroundStyle(.secondary)
            }
            HStack {
                Button(environment ? "修复环境" : "预览地区修改") {
                    Task { await model.previewRepair(region: environment ? nil : selected, autoExecute: environment) }
                }.disabled(model.busy != nil || model.snapshot == nil)
                Button("重新读取地区") { Task { await model.loadRegion() } }.disabled(model.busy != nil)
            }
            if let preview = model.repairPreview {
                Divider().opacity(0.4)
                Text("地区修复预览").font(.headline)
                Text(preview.message)
                HStack(alignment: .top) {
                    regionValues("系统原值", preview.original, preview.networkService, preview.originalIpv6)
                    regionValues("确认目标", preview.target, preview.networkService, preview.targetIpv6)
                }
                Text("用户配置逐项预览").fontWeight(.semibold)
                if preview.userChanges.isEmpty { Text("当前没有用户配置修改项目。").foregroundStyle(.secondary) }
                ForEach(preview.userChanges) { change in
                    VStack(alignment: .leading, spacing: 5) {
                        HStack { Text(itemNames[change.id] ?? change.id).fontWeight(.semibold); Spacer(); Text(stateLabel(change.state)) }
                        Text("\(change.original) → \(change.target)")
                        Text(change.message).foregroundStyle(.secondary)
                        if change.state != "ready" && change.state != "unchanged" { Text("此项需人工处理，本次不会自动修改。") }
                    }.padding(10).switcherGlass(cornerRadius: 8)
                }
                if preview.recoveryRequired {
                    Text("存在未完成的修复事务。程序先查询系统持久结果，再完成提交或恢复原值。")
                    Button("恢复修复事务") { Task { await model.recoverRepair() } }.disabled(model.busy != nil)
                }
                Text("确认系统原值与目标值后执行。系统权限由已验证的 helper 请求。")
                    .foregroundStyle(.secondary)
                HStack {
                    Button("取消此计划") { Task { await model.cancelRepairPlan() } }.disabled(model.busy != nil)
                    Button("确认执行地区计划") { Task { await model.confirmRepair() } }
                        .disabled(model.busy != nil || !preview.canExecute || preview.recoveryRequired)
                }
            }
        }
        .font(.system(size: 12)).panel()
        .task { await model.loadRegion() }
        .onDisappear { Task { await model.cancelRepairPlan() } }
        .onChange(of: selected) { _ in Task { await model.cancelRepairPlan() } }
    }

    private func regionValues(_ label: String, _ region: RegionView, _ network: String, _ ipv6: String) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(label).foregroundStyle(.secondary)
            Text("\(region.country) · \(region.timezone) · \(region.locale)")
            Text("\(network) · IPv6 \(ipv6)").foregroundStyle(.secondary)
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    private func stateLabel(_ state: String) -> String {
        state == "ready" ? "待修改" : state == "unchanged" ? "无需修改" : "需手动处理"
    }
}

private let itemNames = ["locale": "系统地区格式", "client_language": "Claude 界面语言", "proxy_tun_config": "代理 TUN 配置", "proxy_pinned_routes": "代理域名置顶规则"]
