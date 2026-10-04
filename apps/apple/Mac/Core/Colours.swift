import AppKit
import SwiftUI

/// Whether Increase Contrast is on, which the app's colours follow as the system's do.
private var increasedContrast: Bool {
    NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
}

private func isDark(_ appearance: NSAppearance) -> Bool {
    appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
}

extension NSColor {
    /// Secondary text that still reads. The system's `secondaryLabelColor` is about half
    /// opacity, under the 4.5 to 1 text needs; with Increase Contrast it is full text colour.
    static let quietLabel = NSColor(name: "quietLabel") { _ in
        increasedContrast ? .labelColor : NSColor.labelColor.withAlphaComponent(0.78)
    }

    /// The app's accent, as on the phone: system blue is about 4 to 1 on white.
    static let lumennaTint = NSColor(name: "lumennaTint") { appearance in
        switch (isDark(appearance), increasedContrast) {
        case (true, true): NSColor(red: 0.62, green: 0.81, blue: 1.0, alpha: 1)
        case (true, false): NSColor(red: 0.45, green: 0.71, blue: 1.0, alpha: 1)
        case (false, true): NSColor(red: 0.0, green: 0.28, blue: 0.62, alpha: 1)
        case (false, false): NSColor(red: 0.0, green: 0.38, blue: 0.80, alpha: 1)
        }
    }

    /// Red for overdue that clears 4.5 to 1, never the only signal: the word is beside it.
    static let warningLabel = NSColor(name: "warningLabel") { appearance in
        switch (isDark(appearance), increasedContrast) {
        case (true, true): NSColor(red: 1.0, green: 0.62, blue: 0.60, alpha: 1)
        case (true, false): NSColor(red: 1.0, green: 0.48, blue: 0.45, alpha: 1)
        case (false, true): NSColor(red: 0.55, green: 0.0, blue: 0.08, alpha: 1)
        case (false, false): NSColor(red: 0.69, green: 0.0, blue: 0.11, alpha: 1)
        }
    }
}

extension Color {
    static let quietLabel = Color(nsColor: .quietLabel)
    static let lumennaTint = Color(nsColor: .lumennaTint)
    static let warningLabel = Color(nsColor: .warningLabel)
}
