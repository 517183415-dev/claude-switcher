import SwiftUI

public struct OverviewGroup: Identifiable {
    public let title: String
    public let detail: String
    public let symbol: String
    public let status: String
    public let reason: String
    public let good: Bool
    public var id: String { title }

    public init(title: String, detail: String, symbol: String, status: String, reason: String, good: Bool) {
        self.title = title
        self.detail = detail
        self.symbol = symbol
        self.status = status
        self.reason = reason
        self.good = good
    }
}

public struct OverviewAdContent {
    public let title, description, merchant, button: String
    public let imageData: Data?

    public init(title: String, description: String, merchant: String, button: String, imageData: Data?) {
        self.title = title
        self.description = description
        self.merchant = merchant
        self.button = button
        self.imageData = imageData
    }
}

public enum OverviewAdState {
    case disabled, unavailable, active(OverviewAdContent)
}

public struct OverviewAd: Identifiable {
    public let id, name, symbol: String
    public let state: OverviewAdState

    public init(id: String, name: String, symbol: String, state: OverviewAdState) {
        self.id = id
        self.name = name
        self.symbol = symbol
        self.state = state
    }
}

public struct OverviewData {
    public let account: String
    public let accountDetail: String
    public let passed: Int?
    public let total: Int
    public let summary: String
    public let notice: String?
    public let groups: [OverviewGroup]
    public let ads: [OverviewAd]
    public let launchEnabled: Bool

    public init(account: String, accountDetail: String, passed: Int?, total: Int,
                summary: String, notice: String?, groups: [OverviewGroup], ads: [OverviewAd], launchEnabled: Bool) {
        self.account = account
        self.accountDetail = accountDetail
        self.passed = passed
        self.total = total
        self.summary = summary
        self.notice = notice
        self.groups = groups
        self.ads = ads
        self.launchEnabled = launchEnabled
    }
}

/// Presentation only: the caller supplies current data and owns all business actions.
public struct OverviewView: View {
    private let data: OverviewData
    private let onAccounts: () -> Void
    private let onLaunch: () -> Void
    private let onChecks: () -> Void
    private let onRefreshAds: () -> Void
    private let onAdSettings: () -> Void
    private let onAd: (String) -> Void

    public init(data: OverviewData, onAccounts: @escaping () -> Void,
                onLaunch: @escaping () -> Void, onChecks: @escaping () -> Void,
                onRefreshAds: @escaping () -> Void, onAdSettings: @escaping () -> Void,
                onAd: @escaping (String) -> Void) {
        self.data = data
        self.onAccounts = onAccounts
        self.onLaunch = onLaunch
        self.onChecks = onChecks
        self.onRefreshAds = onRefreshAds
        self.onAdSettings = onAdSettings
        self.onAd = onAd
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            VStack(alignment: .leading, spacing: 8) {
                Text("准备好，开始工作。")
                    .font(.system(size: 34, weight: .bold))
                Text("自动检查当前环境，确认准备就绪后启动 Claude。")
                    .font(.system(size: 14)).foregroundStyle(.secondary)
            }
            .padding(.bottom, 8)

            HStack(alignment: .top, spacing: 18) {
                accountCard.frame(maxWidth: .infinity)
                statusCard.frame(width: 330)
            }
            HStack {
                Label("资料保留在本机", systemImage: "lock")
                    .font(.system(size: 12)).foregroundStyle(.secondary)
                Spacer()
                Button("查看账号", action: onAccounts).buttonStyle(.plain)
                    .font(.system(size: 12, weight: .semibold)).foregroundStyle(SwitcherPalette.green)
            }
            HStack(alignment: .top, spacing: 14) {
                ForEach(data.groups) { group in
                    VStack(alignment: .leading, spacing: 12) {
                        Label(group.title, systemImage: group.symbol).font(.system(size: 15, weight: .semibold))
                        Text(group.detail).font(.system(size: 12)).foregroundStyle(.secondary)
                        Label(group.status, systemImage: group.good ? "checkmark.circle.fill" : "exclamationmark.circle")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(group.good ? SwitcherPalette.green : .orange)
                        Text(group.reason).font(.system(size: 11)).foregroundStyle(.secondary)
                        Button("查看详情", action: onChecks).buttonStyle(.plain)
                            .font(.system(size: 12, weight: .semibold)).foregroundStyle(SwitcherPalette.green)
                    }
                    .frame(maxWidth: .infinity, minHeight: 80, alignment: .topLeading)
                    .padding(16).switcherGlass(cornerRadius: 15)
                }
            }
            HStack(alignment: .bottom) {
                VStack(alignment: .leading, spacing: 5) {
                    Text("精选服务").font(.system(size: 18, weight: .bold))
                    Text("三个广告位独立开关，商业内容不参与启动判断。")
                        .font(.system(size: 12)).foregroundStyle(.secondary)
                }
                Spacer()
                Button("刷新广告内容", action: onRefreshAds)
                Button("广告设置", action: onAdSettings)
            }
            .buttonStyle(.plain).font(.system(size: 12, weight: .semibold)).foregroundStyle(SwitcherPalette.green)
            HStack(alignment: .top, spacing: 14) {
                ForEach(data.ads) { ad in AdCard(ad: ad, onOpen: { onAd(ad.id) }).frame(maxWidth: .infinity) }
            }
        }
    }

    private var accountCard: some View {
        VStack(alignment: .leading, spacing: 22) {
            Text("当前账号").font(.system(size: 17, weight: .bold))
            HStack(spacing: 18) {
                Text(String(data.account.suffix(1)))
                    .font(.system(size: 25, weight: .semibold)).foregroundStyle(.white)
                    .frame(width: 65, height: 65)
                    .background(SwitcherPalette.green.gradient, in: Circle())
                    .overlay(Circle().strokeBorder(.white.opacity(0.8), lineWidth: 3))
                    .shadow(color: SwitcherPalette.green.opacity(0.2), radius: 7, y: 3)
                VStack(alignment: .leading, spacing: 6) {
                    Text(data.account).font(.system(size: 26, weight: .bold))
                    Text(data.accountDetail).font(.system(size: 12)).foregroundStyle(.secondary)
                }
                Spacer()
                Button(action: onAccounts) { Label("选择账号", systemImage: "arrow.left.arrow.right") }
                    .buttonStyle(.plain).font(.system(size: 12, weight: .semibold))
                    .padding(.horizontal, 14).padding(.vertical, 9).switcherGlass(cornerRadius: 10)
            }
            HStack(spacing: 0) {
                pathNode("本机", "设备与系统环境", "laptopcomputer")
                Image(systemName: "arrow.right").foregroundStyle(SwitcherPalette.green)
                pathNode("代理隧道", "加密连接 · 网络出口", "server.rack")
                Image(systemName: "arrow.right").foregroundStyle(SwitcherPalette.green)
                pathNode("Claude", "服务与访问验证", "sparkle")
            }
            .padding(.top, 3)
            VStack(spacing: 9) {
                Button(action: onLaunch) {
                    HStack {
                        Spacer()
                        Text("检查并启动 Claude")
                        Image(systemName: "arrow.right").padding(.leading, 12)
                        Spacer()
                    }
                    .font(.system(size: 15, weight: .semibold)).foregroundStyle(.white)
                    .frame(height: 48)
                    .background(SwitcherPalette.green.gradient, in: RoundedRectangle(cornerRadius: 11))
                    .overlay(RoundedRectangle(cornerRadius: 11).strokeBorder(.white.opacity(0.65), lineWidth: 1))
                    .shadow(color: SwitcherPalette.green.opacity(0.2), radius: 9, y: 4)
                }
                .buttonStyle(.plain).disabled(!data.launchEnabled)
                Text("启动前自动检查 14 项环境")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 46)
        }
        .padding(20).switcherGlass(cornerRadius: 18)
    }

    private func pathNode(_ title: String, _ detail: String, _ symbol: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: symbol).font(.system(size: 20))
                .frame(width: 58, height: 58)
                .background(.white.opacity(0.38), in: Circle())
                .overlay(Circle().strokeBorder(.white.opacity(0.75), lineWidth: 1))
            Text(title).font(.system(size: 13, weight: .semibold))
            Text(detail).font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1)
        }
        .frame(maxWidth: .infinity)
    }

    private var statusCard: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("环境概况").font(.system(size: 17, weight: .bold))
            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text(data.passed.map(String.init) ?? "—").font(.system(size: 66, weight: .bold)).foregroundStyle(SwitcherPalette.green)
                Text("/ \(data.total)").font(.system(size: 42, weight: .medium)).foregroundStyle(.secondary)
            }
            Text(data.summary).font(.system(size: 13, weight: .semibold))
            if let notice = data.notice {
                Text(notice).font(.system(size: 12))
                    .frame(maxWidth: .infinity, alignment: .leading).padding(12)
                    .background(Color.orange.opacity(0.1), in: RoundedRectangle(cornerRadius: 10))
                    .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Color.orange.opacity(0.35), lineWidth: 1))
            }
            Button(action: onChecks) { Label("查看检查详情", systemImage: "arrow.right") }
                .buttonStyle(.plain).font(.system(size: 12, weight: .semibold)).foregroundStyle(SwitcherPalette.green)
        }
        .frame(maxWidth: .infinity, minHeight: 312, alignment: .topLeading)
        .padding(20).switcherGlass(cornerRadius: 18)
    }
}
