import UIKit

/// A one-line entry that wraps instead of scrolling sideways — quick add and the filter.
///
/// A text field shows a single line, so at the largest text sizes it shows a few words of what
/// was typed, and the person most likely to have text that large is the one who cannot afford
/// to lose the rest. This wraps, grows to fit, and treats Return as "done" rather than a new
/// line, since what it holds is one line however it is drawn.
final class LineEntry: UITextView, UITextViewDelegate {
    /// Called as the text changes.
    var changed: () -> Void = {}
    /// Called as the cursor moves, which is when completions change too.
    var selectionChanged: () -> Void = {}
    /// Called on Return.
    var submitted: () -> Void = {}

    private let placeholderLabel = UILabel()

    // Set in code — a project's list opens with its query in the filter — the placeholder
    // must go too, or it is drawn under the text.
    override var text: String! {
        didSet { placeholderLabel.isHidden = !text.isEmpty }
    }

    /// What shows while it is empty. VoiceOver hears it as the hint instead, since a label
    /// drawn over the view is not part of it.
    var placeholder: String? {
        get { placeholderLabel.text }
        set {
            placeholderLabel.text = newValue
            accessibilityHint = newValue.map { "Such as: \($0)" }
        }
    }

    init(name: String) {
        super.init(frame: .zero, textContainer: nil)
        accessibilityLabel = name
        font = .preferredFont(forTextStyle: .body)
        adjustsFontForContentSizeCategory = true
        isScrollEnabled = false
        backgroundColor = .secondarySystemBackground
        layer.cornerRadius = 8
        textContainerInset = UIEdgeInsets(top: 8, left: 4, bottom: 8, right: 4)
        returnKeyType = .done
        delegate = self

        placeholderLabel.font = .preferredFont(forTextStyle: .body)
        placeholderLabel.adjustsFontForContentSizeCategory = true
        placeholderLabel.textColor = .quietLabel
        placeholderLabel.numberOfLines = 0
        placeholderLabel.isAccessibilityElement = false
        placeholderLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(placeholderLabel)
        NSLayoutConstraint.activate([
            placeholderLabel.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            placeholderLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 9),
            placeholderLabel.widthAnchor.constraint(equalTo: widthAnchor, constant: -18),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    func textView(
        _ view: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String
    ) -> Bool {
        guard text.contains("\n") else { return true }
        submitted()
        return false
    }

    func textViewDidChange(_ view: UITextView) {
        placeholderLabel.isHidden = !text.isEmpty
        changed()
    }

    func textViewDidChangeSelection(_ view: UITextView) {
        selectionChanged()
    }
}
