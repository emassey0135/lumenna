import SwiftUI

/// What the app says, shown for a moment at the top of the screen as well as spoken, so someone
/// without VoiceOver sees a refusal or what a change did. VoiceOver already says it, so the
/// banner is hidden from it, and it takes no taps: whatever is under it still answers. Left out
/// under test (`LUMENNA_TEST_PROFILE`), where the audits would judge text on its way out.
struct SaidBanner: ViewModifier {
    @State private var shown: String?
    @State private var shownAt = Date.distantPast
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let wanted = ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] == nil

    func body(content: Content) -> some View {
        if Self.wanted {
            content
                .overlay(alignment: .top) {
                    if let shown {
                        Text(shown)
                            .font(.footnote)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 6)
                            .frame(maxWidth: .infinity)
                            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
                            .accessibilityHidden(true)
                            .allowsHitTesting(false)
                            .transition(.opacity)
                    }
                }
                .onReceive(NotificationCenter.default.publisher(for: Said.posted)) { note in
                    guard let text = Said.text(of: note) else { return }
                    let at = Date()
                    withAnimation(reduceMotion ? nil : .easeOut(duration: 0.2)) { shown = text }
                    shownAt = at
                    DispatchQueue.main.asyncAfter(deadline: .now() + Said.seconds(for: text)) {
                        // A later sentence keeps the banner for its own time.
                        guard shownAt == at else { return }
                        withAnimation(reduceMotion ? nil : .easeIn(duration: 0.3)) { shown = nil }
                    }
                }
        } else {
            content
        }
    }
}

extension View {
    /// Shows what the app says over this screen (`SaidBanner`).
    func showsWhatIsSaid() -> some View { modifier(SaidBanner()) }
}
