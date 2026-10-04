import UIKit

/// Suggestions for what is being typed, above the keyboard (§6.3).
///
/// The core says what could go at the cursor; this shows it. Nothing is announced as the
/// suggestions change — speech on every keystroke would bury the typing — so the count comes
/// first in the bar, and a VoiceOver user meets it before the candidates, as §6.3 asks.
final class CompletionBar: UIInputView {
    private let core: Core
    private let syntax: Syntax
    private weak var field: (UIView & UITextInput)?
    private let stack = UIStackView()
    private var current: Completions?

    /// `field` is a text field or a text view; whoever owns it calls [`update`] when its text
    /// or selection changes.
    init(core: Core, syntax: Syntax, field: UIView & UITextInput) {
        self.core = core
        self.syntax = syntax
        self.field = field
        super.init(frame: CGRect(x: 0, y: 0, width: 0, height: 52), inputViewStyle: .keyboard)
        allowsSelfSizing = true

        let scroll = UIScrollView()
        scroll.showsHorizontalScrollIndicator = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        stack.axis = .horizontal
        stack.spacing = 8
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(scroll)
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(greaterThanOrEqualToConstant: 52),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            scroll.topAnchor.constraint(equalTo: topAnchor),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            stack.centerYAnchor.constraint(equalTo: scroll.centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    /// Asks the core what could go at the cursor, and shows it.
    @objc func update() {
        guard let field, let text = Self.text(of: field) else { return }
        let cursor = Self.byteOffset(of: field.selectedTextRange?.start, in: field, text: text)
        current = try? core.lumenna.completeText(text: text, cursor: cursor, syntax: syntax)
        redraw()
    }

    private func redraw() {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard let current, !current.candidates.isEmpty else { return }

        let count = UILabel()
        count.text = current.announcement
        count.font = .preferredFont(forTextStyle: .footnote)
        count.adjustsFontForContentSizeCategory = true
        count.textColor = .quietLabel
        stack.addArrangedSubview(count)

        for candidate in current.candidates {
            var configuration = UIButton.Configuration.gray()
            configuration.title = candidate.text
            let button = UIButton(
                configuration: configuration,
                primaryAction: UIAction { [weak self] _ in self?.insert(candidate) }
            )
            // "project Work", not "#Work": the sigil is punctuation VoiceOver may skip.
            button.accessibilityLabel = candidate.label
            stack.addArrangedSubview(button)
        }
    }

    /// Puts a candidate in place of the text it replaces, with a space after it so typing
    /// carries on.
    private func insert(_ candidate: Candidate) {
        guard let field, let text = Self.text(of: field), let current else { return }
        let bytes = Array(text.utf8)
        let start = min(Int(current.start), bytes.count)
        let end = min(max(Int(current.end), start), bytes.count)
        let before = String(decoding: bytes[..<start], as: UTF8.self)
        let after = String(decoding: bytes[end...], as: UTF8.self)
        let inserted = candidate.text + (after.hasPrefix(" ") ? "" : " ")
        let replaced = before + inserted + after
        switch field {
        case let field as UITextField:
            field.text = replaced
        case let view as UITextView:
            view.text = replaced
        default:
            return
        }
        if let position = field.position(
            from: field.beginningOfDocument, offset: (before + inserted).utf16.count
        ) {
            field.selectedTextRange = field.textRange(from: position, to: position)
        }
        // Tell the owner, as typing would.
        switch field {
        case let field as UITextField:
            field.sendActions(for: .editingChanged)
        case let view as UITextView:
            view.delegate?.textViewDidChange?(view)
        default:
            break
        }
    }

    private static func text(of field: UIView & UITextInput) -> String? {
        switch field {
        case let field as UITextField: field.text
        case let view as UITextView: view.text
        default: nil
        }
    }

    /// The core counts in UTF-8 bytes, as Rust strings do; UIKit counts in UTF-16.
    private static func byteOffset(
        of position: UITextPosition?, in field: UIView & UITextInput, text: String
    ) -> UInt32 {
        guard let position else { return UInt32(text.utf8.count) }
        let units = field.offset(from: field.beginningOfDocument, to: position)
        let index = String.Index(utf16Offset: max(0, min(units, text.utf16.count)), in: text)
        return UInt32(text[..<index].utf8.count)
    }
}
