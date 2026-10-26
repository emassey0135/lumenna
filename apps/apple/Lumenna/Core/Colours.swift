import UIKit

extension UIColor {
    /// Secondary text that still reads. The system's `secondaryLabel` is about 3.4 to 1
    /// against the background, under the 4.5 to 1 that text this size needs, and the people
    /// most likely to use this app with no screen reader at all are the ones with low vision.
    ///
    /// With Increase Contrast on it is the full text colour, as the system's own greys go
    /// darker then.
    static let quietLabel = UIColor { traits in
        traits.accessibilityContrast == .high ? .label : UIColor.label.resolvedColor(with: traits).withAlphaComponent(0.85)
    }

    /// The app's tint (`Shades.tint`).
    static let lumennaTint = UIColor { traits in
        UIColor(Shades.tint.pick(dark: traits.userInterfaceStyle == .dark, contrast: traits.accessibilityContrast == .high))
    }

    /// Red for overdue (`Shades.warning`), never the only signal.
    static let warningLabel = UIColor { traits in
        UIColor(Shades.warning.pick(dark: traits.userInterfaceStyle == .dark, contrast: traits.accessibilityContrast == .high))
    }

    private convenience init(_ shade: Shades.RGB) {
        self.init(red: shade.red, green: shade.green, blue: shade.blue, alpha: 1)
    }
}
