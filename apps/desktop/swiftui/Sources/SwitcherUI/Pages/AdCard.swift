import AppKit
import SwiftUI

extension OverviewAdContent {
    init(campaign: Campaign, imageData: Data?) {
        self.init(title: campaign.title, description: campaign.description,
                  merchant: campaign.merchantName, button: campaign.buttonText, imageData: imageData)
    }
}

/// The same card renders published content and the non-clickable draft preview.
struct AdCard: View {
    let ad: OverviewAd
    let onOpen: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(ad.name)
                Spacer()
                Text("推广")
            }.font(.system(size: 11)).foregroundStyle(.secondary)
            switch ad.state {
            case .disabled, .unavailable:
                Image(systemName: ad.symbol).font(.system(size: 24)).foregroundStyle(SwitcherPalette.green)
                Text(ad.state.isDisabled ? "此广告位已关闭" : "暂无有效内容")
                    .font(.system(size: 15, weight: .semibold))
                Text(ad.state.isDisabled ? "不会读取此广告位的素材。" : "内容通过签名与排期校验后显示。")
                    .font(.system(size: 12)).foregroundStyle(.secondary)
                Text("账号功能继续按环境检查结果工作")
                    .font(.system(size: 10)).foregroundStyle(.secondary)
            case .active(let content):
                if let bytes = content.imageData, let image = NSImage(data: bytes) {
                    Image(nsImage: image).resizable().scaledToFit().frame(maxWidth: .infinity).frame(height: 124)
                } else {
                    Label("素材等待本地校验", systemImage: ad.symbol)
                        .frame(maxWidth: .infinity).frame(height: 104)
                        .background(SwitcherPalette.accent.opacity(0.25), in: RoundedRectangle(cornerRadius: 8))
                }
                Text(content.title).font(.system(size: 15, weight: .semibold))
                Text(content.description).font(.system(size: 12)).foregroundStyle(.secondary)
                Text(content.merchant).font(.system(size: 10)).foregroundStyle(.secondary)
                Button(content.button) { onOpen?() }
                    .buttonStyle(.plain).font(.system(size: 12, weight: .semibold)).foregroundStyle(SwitcherPalette.green)
                    .disabled(onOpen == nil)
            }
        }
        .frame(maxWidth: .infinity, minHeight: 155, alignment: .topLeading)
        .padding(16).switcherGlass(cornerRadius: 15)
    }
}

private extension OverviewAdState {
    var isDisabled: Bool {
        if case .disabled = self { return true }
        return false
    }
}
