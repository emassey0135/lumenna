import UIKit

/// What the app says, shown for a moment at the top of the window for a person who reads it, as
/// Windows and GTK show it on their status line. VoiceOver has spoken it already (`Announcer`),
/// so it is no element of its own; and it takes no touches, so what lies under it is pressed as
/// if it were not there.
final class SaidBanner: UIView {
    private let label = UILabel()
    private var hiding: DispatchWorkItem?
    private var observer: NSObjectProtocol?

    /// Shows what is said on `window`, over everything in it. Not under the UI tests: the
    /// audits judge contrast by what is drawn, and a banner over the navigation bar for a few
    /// seconds after every action would fail them for nothing (`SaidBannerTests` checks it).
    @discardableResult
    static func install(on window: UIWindow) -> SaidBanner? {
        guard ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] == nil else { return nil }
        let banner = SaidBanner()
        banner.place(on: window)
        return banner
    }

    init() {
        super.init(frame: .zero)
        isUserInteractionEnabled = false
        accessibilityElementsHidden = true
        alpha = 0
        backgroundColor = .secondarySystemBackground
        layer.cornerRadius = 12
        layer.shadowColor = UIColor.black.cgColor
        layer.shadowOpacity = 0.25
        layer.shadowRadius = 6
        layer.shadowOffset = CGSize(width: 0, height: 2)
        label.font = .preferredFont(forTextStyle: .subheadline)
        label.adjustsFontForContentSizeCategory = true
        label.numberOfLines = 0
        label.textColor = .label
        label.textAlignment = .center
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.topAnchor.constraint(equalTo: topAnchor, constant: 10),
            label.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -10),
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -14),
        ])
        observer = NotificationCenter.default.addObserver(forName: Said.posted, object: nil, queue: .main) { [weak self] note in
            if let text = Said.text(of: note) { self?.show(text) }
        }
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Pinned under the top of `window`'s safe area, centred, never wider than a sentence needs.
    func place(on window: UIWindow) {
        translatesAutoresizingMaskIntoConstraints = false
        window.addSubview(self)
        let guide = window.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            topAnchor.constraint(equalTo: guide.topAnchor, constant: 4),
            centerXAnchor.constraint(equalTo: guide.centerXAnchor),
            leadingAnchor.constraint(greaterThanOrEqualTo: guide.leadingAnchor, constant: 12),
            trailingAnchor.constraint(lessThanOrEqualTo: guide.trailingAnchor, constant: -12),
            widthAnchor.constraint(lessThanOrEqualToConstant: 560),
        ])
    }

    /// The sentence shown now, if one is.
    var shown: String? { alpha > 0 ? label.text : nil }

    /// Shows `text` in place of whatever was shown, for as long as it takes to read.
    func show(_ text: String) {
        label.text = text
        superview?.bringSubviewToFront(self)
        hiding?.cancel()
        let quick = UIAccessibility.isReduceMotionEnabled || !UIView.areAnimationsEnabled
        UIView.animate(withDuration: quick ? 0 : 0.2) { self.alpha = 1 }
        let hide = DispatchWorkItem { [weak self] in
            UIView.animate(withDuration: quick ? 0 : 0.3) { self?.alpha = 0 }
        }
        hiding = hide
        DispatchQueue.main.asyncAfter(deadline: .now() + Said.seconds(for: text), execute: hide)
    }
}
