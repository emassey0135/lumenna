import SwiftUI

/// The colours text and controls are drawn in that clear contrast where the system's do not, in
/// SwiftUI, for every Apple app's forms and its widgets: each platform's own (`Colours.swift`,
/// with Increase Contrast variants), and the watch's, which is always dark.
extension Color {
    #if os(iOS)
    static let quietLabel = Color(uiColor: .quietLabel)
    static let lumennaTint = Color(uiColor: .lumennaTint)
    static let warningLabel = Color(uiColor: .warningLabel)
    #elseif os(watchOS)
    // A watch is always dark: a light grey, blue and red, each well over contrast on black.
    static let quietLabel = Color(white: 0.75)
    static let lumennaTint = Color(red: 0.45, green: 0.72, blue: 1.0)
    static let warningLabel = Color(red: 1.0, green: 0.5, blue: 0.5)
    #else
    static let quietLabel = Color(nsColor: .quietLabel)
    static let lumennaTint = Color(nsColor: .lumennaTint)
    static let warningLabel = Color(nsColor: .warningLabel)
    #endif
}

/// The app's own colours, as red, green and blue in each appearance, with Increase Contrast and
/// without: one table, which UIKit's and AppKit's colours (`Colours.swift`) both read, so the
/// phone and the Mac cannot drift apart.
enum Shades {
    typealias RGB = (red: Double, green: Double, blue: Double)

    /// A colour in each of the four appearances.
    struct Four {
        let light: RGB
        let lightContrast: RGB
        let dark: RGB
        let darkContrast: RGB

        func pick(dark isDark: Bool, contrast: Bool) -> RGB {
            switch (isDark, contrast) {
            case (true, true): darkContrast
            case (true, false): dark
            case (false, true): lightContrast
            case (false, false): light
            }
        }
    }

    /// The app's tint. The system blue is about 4 to 1 on white, under the 4.5 to 1 that text in
    /// buttons and links needs; this one clears it in both appearances, and further still with
    /// Increase Contrast, as the system's blue does.
    static let tint = Four(
        light: (0.0, 0.38, 0.80), lightContrast: (0.0, 0.28, 0.62),
        dark: (0.45, 0.71, 1.0), darkContrast: (0.62, 0.81, 1.0)
    )

    /// Red for overdue that clears 4.5 to 1 in both appearances, which the system's red does not
    /// on white. Never the only signal: the word "overdue" is always beside it.
    static let warning = Four(
        light: (0.69, 0.0, 0.11), lightContrast: (0.55, 0.0, 0.08),
        dark: (1.0, 0.48, 0.45), darkContrast: (1.0, 0.62, 0.60)
    )
}
