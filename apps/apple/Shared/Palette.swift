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
