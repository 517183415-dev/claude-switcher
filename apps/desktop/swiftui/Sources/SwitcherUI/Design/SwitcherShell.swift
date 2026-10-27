import SwiftUI

public enum SwitcherPage: String, CaseIterable, Identifiable {
    case overview, accounts, environment, activity, operations, settings
    public var id: Self { self }

    public var title: String {
        switch self {
        case .overview: return "总览"
        case .accounts: return "账号"
        case .environment: return "环境检查"
        case .activity: return "活动记录"
        case .operations: return "广告运营"
        case .settings: return "设置"
        }
    }

    var symbol: String {
        switch self {
        case .overview: return "house"
        case .accounts: return "person"
        case .environment: return "externaldrive"
        case .activity: return "list.clipboard"
        case .operations: return "photo"
        case .settings: return "gearshape"
        }
    }

    public static func visible(in edition: DesktopEdition) -> [Self] {
        allCases.filter { edition == .personal || $0 != .operations }
    }
}

/// Navigation and footer remain in the same places as the current product.
public struct SwitcherShell<Content: View>: View {
    @Binding private var selection: SwitcherPage
    private let edition: DesktopEdition
    private let version: String
    private let lastCheck: String
    private let onRefresh: () -> Void
    private let content: (SwitcherPage) -> Content

    public init(selection: Binding<SwitcherPage>, edition: DesktopEdition, version: String,
                lastCheck: String, onRefresh: @escaping () -> Void,
                @ViewBuilder content: @escaping (SwitcherPage) -> Content) {
        _selection = selection
        self.edition = edition
        self.version = version
        self.lastCheck = lastCheck
        self.onRefresh = onRefresh
        self.content = content
    }

    public var body: some View {
        ZStack {
            SwitcherBackground()
            HStack(spacing: 0) {
                sidebar
                VStack(spacing: 0) {
                    ScrollView { content(selection).frame(maxWidth: .infinity, alignment: .leading).padding(30) }
                    footer
                }
            }
        }
        .foregroundStyle(SwitcherPalette.ink)
        .preferredColorScheme(.light)
        .frame(minWidth: 960, minHeight: 680)
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            Label("工作空间", systemImage: "square.grid.2x2")
                .font(.system(size: 17, weight: .bold))
                .padding(.horizontal, 18).padding(.top, 30).padding(.bottom, 25)
            ForEach(SwitcherPage.visible(in: edition).filter { $0 != .settings }) { page in
                navigationButton(page)
            }
            Spacer()
            navigationButton(.settings)
            Text("\(edition == .personal ? "Personal" : "Public") · \(version)")
                .font(.system(size: 10)).foregroundStyle(.secondary)
                .padding(.horizontal, 18).padding(.top, 16).padding(.bottom, 18)
        }
        .padding(.horizontal, 10)
        .frame(width: 183)
        .background(.ultraThinMaterial)
        .overlay(alignment: .trailing) { Rectangle().fill(.white.opacity(0.45)).frame(width: 1) }
    }

    private func navigationButton(_ page: SwitcherPage) -> some View {
        Button { selection = page } label: {
            Label(page.title, systemImage: page.symbol)
                .font(.system(size: 14, weight: selection == page ? .semibold : .regular))
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 15).frame(height: 48)
                .background(selection == page ? SwitcherPalette.accent.opacity(0.38) : .clear,
                            in: RoundedRectangle(cornerRadius: 11))
                .overlay {
                    RoundedRectangle(cornerRadius: 11)
                        .strokeBorder(.white.opacity(selection == page ? 0.8 : 0), lineWidth: 1)
                }
        }
        .buttonStyle(.plain)
        .padding(.bottom, 6)
    }

    private var footer: some View {
        HStack(spacing: 10) {
            Text("最近检查")
            Text(lastCheck)
            Rectangle().fill(.secondary.opacity(0.25)).frame(width: 1, height: 14)
            Button(action: onRefresh) { Label("重新检查", systemImage: "arrow.clockwise") }
                .buttonStyle(.plain).foregroundStyle(SwitcherPalette.green)
            Spacer()
            Label("资料保留在本机", systemImage: "lock")
        }
        .font(.system(size: 11)).foregroundStyle(.secondary)
        .padding(.horizontal, 30).frame(height: 40)
        .background(.ultraThinMaterial)
        .overlay(alignment: .top) { Rectangle().fill(.white.opacity(0.6)).frame(height: 1) }
    }
}

private struct SwitcherBackground: View {
    var body: some View {
        ZStack {
            LinearGradient(colors: [Color(red: 0.96, green: 0.95, blue: 0.92),
                                    Color(red: 0.87, green: 0.89, blue: 0.84)],
                           startPoint: .topLeading, endPoint: .bottomTrailing)
            Ellipse().fill(.white.opacity(0.6)).frame(width: 700, height: 360)
                .rotationEffect(.degrees(-24)).offset(x: 250, y: -200).blur(radius: 65)
            Ellipse().fill(SwitcherPalette.accent.opacity(0.45)).frame(width: 560, height: 250)
                .rotationEffect(.degrees(-32)).offset(x: -330, y: 260).blur(radius: 70)
        }
        .ignoresSafeArea()
    }
}
