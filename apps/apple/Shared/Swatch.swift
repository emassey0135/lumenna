import LumennaApp
import SwiftUI

/// A colour of the core's palette, drawn beside a name and never in place of it: the name, and
/// the colour's word in the row's line, are what VoiceOver says, so the mark is hidden from it.
/// Its light or dark value follows the appearance, and it grows with the text.
struct SwatchMark: View {
    let swatch: LumennaApp.Swatch?
    @Environment(\.colorScheme) private var scheme
    @ScaledMetric(relativeTo: .body) private var size: CGFloat = 10

    var body: some View {
        if let swatch {
            Circle()
                .fill(Color(hex: scheme == .dark ? swatch.dark : swatch.light))
                .frame(width: size, height: size)
                .accessibilityHidden(true)
        }
    }
}

extension Color {
    /// `#RRGGBB`, as the core gives a colour; black for anything else.
    init(hex: String) {
        let digits = UInt32(hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) ?? 0
        self.init(
            red: Double((digits >> 16) & 0xFF) / 255,
            green: Double((digits >> 8) & 0xFF) / 255,
            blue: Double(digits & 0xFF) / 255
        )
    }
}
