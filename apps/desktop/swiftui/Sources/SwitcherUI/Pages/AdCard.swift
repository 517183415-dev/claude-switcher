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
        HStack(spacing: 10) {
            artwork
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 4) {
                    Text(title).font(.system(size: 12, weight: .semibold)).lineLimit(1)
                    Spacer(minLength: 0)
                    Text("推广").font(.system(size: 9, weight: .medium))
                        .foregroundStyle(.orange)
                        .padding(.horizontal, 5).padding(.vertical, 3)
                        .background(Color.orange.opacity(0.12), in: RoundedRectangle(cornerRadius: 6))
                }
                switch ad.state {
                case .disabled, .unavailable:
                    Text(ad.state.isDisabled ? "此广告位已关闭" : "暂无有效内容")
                        .font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1)
                case .active(let content):
                    Text(content.imageData == nil ? "\(content.merchant) · 素材待校验" : content.merchant)
                        .font(.system(size: 10)).foregroundStyle(.secondary).lineLimit(1)
                    Button(content.button) { onOpen?() }
                        .buttonStyle(.plain).font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(SwitcherPalette.green).lineLimit(1)
                        .disabled(onOpen == nil)
                }
            }
        }
        .frame(maxWidth: .infinity, minHeight: 54, alignment: .leading)
        .padding(10).switcherGlass(cornerRadius: 12)
        .help(details)
    }

    private var artwork: some View {
        Group {
            if case .active(let content) = ad.state,
               let bytes = content.imageData, let image = NSImage(data: bytes) {
                Image(nsImage: image).resizable().scaledToFit()
            } else {
                Image(systemName: ad.symbol).font(.system(size: 21)).foregroundStyle(SwitcherPalette.green)
            }
        }
        .frame(width: 44, height: 44)
        .background(SwitcherPalette.accent.opacity(0.45), in: RoundedRectangle(cornerRadius: 8))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    private var title: String {
        if case .active(let content) = ad.state { return content.title }
        return ad.name
    }

    private var details: String {
        switch ad.state {
        case .disabled: return "\(ad.name)：此广告位已关闭，不会读取素材。"
        case .unavailable: return "\(ad.name)：暂无有效内容。"
        case .active(let content): return "\(content.merchant) · \(content.title)\n\(content.description)"
        }
    }
}

private extension OverviewAdState {
    var isDisabled: Bool {
        if case .disabled = self { return true }
        return false
    }
}
