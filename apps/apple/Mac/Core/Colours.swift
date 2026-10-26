import AppKit

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

    /// The app's accent, as on the phone (`Shades.tint`).
    static let lumennaTint = NSColor(name: "lumennaTint") { appearance in
        NSColor(Shades.tint.pick(dark: isDark(appearance), contrast: increasedContrast))
    }

    /// Red for overdue (`Shades.warning`), never the only signal: the word is beside it.
    static let warningLabel = NSColor(name: "warningLabel") { appearance in
        NSColor(Shades.warning.pick(dark: isDark(appearance), contrast: increasedContrast))
    }

    private convenience init(_ shade: Shades.RGB) {
        self.init(red: shade.red, green: shade.green, blue: shade.blue, alpha: 1)
    }
}
