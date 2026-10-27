import SwiftUI

public enum SwitcherPalette {
    public static let ink = Color(red: 0.12, green: 0.17, blue: 0.15)
    public static let green = Color(red: 0.12, green: 0.32, blue: 0.24)
    public static let background = Color(red: 0.96, green: 0.96, blue: 0.93)
    public static let accent = Color(red: 0.79, green: 0.86, blue: 0.79)
}

public extension View {
    @ViewBuilder
    func switcherGlass(cornerRadius: CGFloat = 18) -> some View {
        let shape = RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
        if #available(macOS 26.0, *) {
            self.background(.white.opacity(0.24), in: shape)
                .glassEffect(.regular, in: shape)
                .overlay(shape.strokeBorder(.white.opacity(0.7), lineWidth: 1))
                .shadow(color: .black.opacity(0.05), radius: 9, y: 4)
        } else {
            self.background(.ultraThinMaterial, in: shape)
                .overlay(shape.strokeBorder(.white.opacity(0.65), lineWidth: 1))
        }
    }
}
