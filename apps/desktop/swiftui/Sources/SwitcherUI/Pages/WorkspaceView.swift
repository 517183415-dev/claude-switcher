import SwiftUI

/// Native workspace. The caller supplies the already-authorized Tauri transport.
@MainActor
public struct SwitcherWorkspaceView: View {
    @StateObject private var model: WorkspaceModel
    @StateObject private var publisher: PublisherModel
    @State private var page: SwitcherPage = .overview
    @State private var consentIndex: Int?

    public init(bridge: DesktopBridge) {
        _model = StateObject(wrappedValue: WorkspaceModel(bridge: bridge))
        _publisher = StateObject(wrappedValue: PublisherModel(bridge: bridge))
    }

    public var body: some View {
        SwitcherShell(selection: $page, edition: model.bridge.edition,
                      version: model.snapshot?.runtime.version ?? "连接中",
                      lastCheck: dateText(model.snapshot?.checkReport?.completedAt),
                      onRefresh: { Task { await model.runChecks() } }) { selected in
            VStack(alignment: .leading, spacing: 18) {
                notices
                switch selected {
                case .overview:
                    OverviewView(data: model.overview,
                                 onAccounts: { page = .accounts },
                                 onLaunch: { Task { await model.perform(.launchChecked, label: "正在重新检查并准备启动…") } },
                                 onChecks: { page = .environment },
                                 onRefreshAds: { Task { await model.perform(.refreshAdContent, label: "正在刷新并验证广告内容…") } },
                                 onAdSettings: { page = .settings },
                                 onAd: { slot in Task { await model.openAd(slot: slot, confirmed: false) } })
                case .accounts: AccountsPage(model: model)
                case .environment: EnvironmentPage(model: model)
                case .activity: ActivityPage(model: model)
                case .settings: SettingsPage(model: model, consentIndex: $consentIndex)
                case .operations: PublisherPage(model: publisher)
                }
            }
        }
        .task {
            await model.connect()
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 30_000_000_000)
                if Task.isCancelled { break }
                if model.busy == nil { try? await model.refresh() }
            }
        }
        .onChange(of: page) { selected in
            if selected == .activity { Task { await model.loadActivity() } }
        }
        .alert("开启广告展示", isPresented: Binding(get: { consentIndex != nil }, set: { if !$0 { consentIndex = nil } })) {
            Button("暂不开启", role: .cancel) { consentIndex = nil }
            Button("确认开启") {
                if let index = consentIndex { Task { await model.setAdEnabled(index: index, enabled: true, acknowledged: true) } }
                consentIndex = nil
            }
        } message: {
            Text("应用将向固定内容源读取签名清单与素材。内容源会看到连接来源，请求不会附加账号、设备或代理资料。")
        }
        .alert(item: $model.adConfirmation) { confirmation in
            Alert(title: Text("打开商家网站"), message: Text(confirmation.message),
                  primaryButton: .default(Text("确认打开")) { Task { await model.openAd(slot: confirmation.slot, confirmed: true) } },
                  secondaryButton: .cancel())
        }
    }

    @ViewBuilder private var notices: some View {
        if let busy = model.busy {
            HStack {
                ProgressView().controlSize(.small)
                Text(busy)
                if model.canCancelCheck { Button("取消本次检查") { Task { await model.cancelChecks() } } }
            }.notice()
        }
        if let error = model.error {
            HStack {
                Label(error, systemImage: "exclamationmark.triangle")
                Spacer()
                Button("重新连接") { Task { await model.connect() } }
                Button("关闭") { model.error = nil }
            }.notice()
        }
        if let outcome = model.outcome {
            HStack {
                VStack(alignment: .leading, spacing: 5) {
                    Label(outcome.message, systemImage: outcome.completed ? "checkmark.circle" : "exclamationmark.circle")
                    if let recovery = desktopRecoveryMessage(outcome.recovery) { Text(recovery) }
                }
                Spacer()
                Button("关闭") { model.outcome = nil }
            }.notice()
        }
    }
}

func dateText(_ timestamp: Int64?) -> String {
    guard let timestamp, timestamp > 0 else { return "尚无记录" }
    return Date(timeIntervalSince1970: TimeInterval(timestamp)).formatted(date: .abbreviated, time: .standard)
}

func pageHeading(_ title: String, _ detail: String) -> some View {
    VStack(alignment: .leading, spacing: 8) {
        Text(title).font(.system(size: 30, weight: .bold))
        Text(detail).font(.system(size: 13)).foregroundStyle(.secondary)
    }.padding(.bottom, 8)
}

extension View {
    func panel() -> some View {
        self.frame(maxWidth: .infinity, alignment: .leading).padding(20).switcherGlass(cornerRadius: 16)
    }
    func notice() -> some View {
        self.font(.system(size: 12)).frame(maxWidth: .infinity, alignment: .leading).padding(14).switcherGlass(cornerRadius: 10)
    }
}
