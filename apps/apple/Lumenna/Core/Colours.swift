import UIKit

extension UIColor {
    /// Secondary text that still reads. The system's `secondaryLabel` is about 3.4 to 1
    /// against the background, under the 4.5 to 1 that text this size needs, and the people
    /// most likely to use this app with no screen reader at all are the ones with low vision.
    static let quietLabel = UIColor.label.withAlphaComponent(0.78)

    /// Red for overdue that clears 4.5 to 1 in both appearances, which `systemRed` does not
    /// on white. Never the only signal: the word "overdue" is always beside it (§13).
    static let warningLabel = UIColor { traits in
        traits.userInterfaceStyle == .dark
            ? UIColor(red: 1.0, green: 0.48, blue: 0.45, alpha: 1)
            : UIColor(red: 0.69, green: 0.0, blue: 0.11, alpha: 1)
    }
}
