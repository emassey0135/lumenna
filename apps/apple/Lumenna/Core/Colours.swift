import UIKit

extension UIColor {
    /// Secondary text that still reads. The system's `secondaryLabel` is about 3.4 to 1
    /// against the background, under the 4.5 to 1 that text this size needs, and the people
    /// most likely to use this app with no screen reader at all are the ones with low vision.
    ///
    /// With Increase Contrast on it is the full text colour, as the system's own greys go
    /// darker then.
    static let quietLabel = UIColor { traits in
        traits.accessibilityContrast == .high ? .label : UIColor.label.resolvedColor(with: traits).withAlphaComponent(0.78)
    }

    /// The app's tint. The system blue is about 4 to 1 on white, under the 4.5 to 1 that text
    /// in buttons and links needs; this one clears it in both appearances.
    /// Further still with Increase Contrast, as the system's blue does.
    static let lumennaTint = UIColor { traits in
        switch (traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high) {
        case (true, true): UIColor(red: 0.62, green: 0.81, blue: 1.0, alpha: 1)
        case (true, false): UIColor(red: 0.45, green: 0.71, blue: 1.0, alpha: 1)
        case (false, true): UIColor(red: 0.0, green: 0.28, blue: 0.62, alpha: 1)
        case (false, false): UIColor(red: 0.0, green: 0.38, blue: 0.80, alpha: 1)
        }
    }

    /// Red for overdue that clears 4.5 to 1 in both appearances, which `systemRed` does not
    /// on white. Never the only signal: the word "overdue" is always beside it.
    static let warningLabel = UIColor { traits in
        switch (traits.userInterfaceStyle == .dark, traits.accessibilityContrast == .high) {
        case (true, true): UIColor(red: 1.0, green: 0.62, blue: 0.60, alpha: 1)
        case (true, false): UIColor(red: 1.0, green: 0.48, blue: 0.45, alpha: 1)
        case (false, true): UIColor(red: 0.55, green: 0.0, blue: 0.08, alpha: 1)
        case (false, false): UIColor(red: 0.69, green: 0.0, blue: 0.11, alpha: 1)
        }
    }
}
