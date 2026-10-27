import AppKit
import SwiftUI
import SwitcherUI

@main
struct SwitcherPreviewApp: App {
    var body: some Scene {
        WindowGroup("SwiftUI 效果预览") {
            PreviewRoot()
                .task {
                    try? await Task.sleep(nanoseconds: 90_000_000_000)
                    NSApplication.shared.terminate(nil)
                }
        }
        .defaultSize(width: 1280, height: 800)
    }
}

private struct PreviewRoot: View {
    @State private var page: SwitcherPage = .overview
    private let example = OverviewData(
        account: "账号 A", accountDetail: "已选择 · 本地档案", passed: 13, total: 14,
        summary: "13 项通过 · 1 项提示", notice: "网络运营商信息暂未获取",
        groups: [OverviewGroup(title: "网络连接", detail: "代理 · TUN · DNS · STUN", symbol: "globe", status: "通过", reason: "本次证据已取得", good: true),
                 OverviewGroup(title: "地区设置", detail: "出口地区 · 时区 · 地区格式", symbol: "mappin.circle", status: "通过", reason: "本次证据已取得", good: true),
                 OverviewGroup(title: "后台服务", detail: "守护进程 · 服务路由", symbol: "gearshape", status: "通过", reason: "本次证据已取得", good: true)],
        ads: [OverviewAd(id: "residential_broadband", name: "美国家庭宽带", symbol: "globe", state: .disabled),
              OverviewAd(id: "account_service", name: "注册好的账号", symbol: "person", state: .disabled),
              OverviewAd(id: "kyc_service", name: "KYC 服务", symbol: "lock", state: .disabled)],
        launchEnabled: true
    )

    var body: some View {
        SwitcherShell(selection: $page, edition: .personal, version: "0.2.0",
                      lastCheck: "示例时间", onRefresh: {}) { selected in
            VStack(alignment: .leading, spacing: 18) {
                Text("界面预览 · 合成数据 · 不执行系统或账号操作")
                    .font(.system(size: 12)).foregroundStyle(.secondary)
                    .padding(.horizontal, 12).padding(.vertical, 9)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.white.opacity(0.4), in: RoundedRectangle(cornerRadius: 9))
                if selected == .overview {
                    OverviewView(data: example, onAccounts: { page = .accounts },
                                 onLaunch: {}, onChecks: { page = .environment },
                                 onRefreshAds: {}, onAdSettings: { page = .settings }, onAd: { _ in })
                } else {
                    Text(selected.title).font(.system(size: 30, weight: .bold))
                    Text("此页面的原生迁移尚未完成。")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}
