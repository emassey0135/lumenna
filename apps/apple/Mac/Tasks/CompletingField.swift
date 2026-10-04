import AppKit

/// A text field that offers what could be typed next (§6.3), through the system's own
/// completion: Option-Escape or F5 as in any Mac text field, and by itself after a `#` or `@`,
/// where a name is the only thing that can follow.
///
/// The core says what goes at the cursor and which span it replaces, in UTF-8 bytes; AppKit
/// completes the word it thinks is partial, in UTF-16. The two are reconciled here.
final class CompletingField: NSTextField, NSTextFieldDelegate {
    private let core: Core
    private let syntax: Syntax
    /// Called after every change to the text, typed or completed.
    var changed: (() -> Void)?
    /// Called on Return.
    var submitted: (() -> Void)?
    private var completing = false

    init(core: Core, syntax: Syntax, name: String) {
        self.core = core
        self.syntax = syntax
        super.init(frame: .zero)
        delegate = self
        setAccessibilityLabel(name)
        setAccessibilityHelp("Option-Escape offers what could come next.")
        lineBreakMode = .byWordWrapping
        usesSingleLineMode = false
        cell?.wraps = true
        cell?.isScrollable = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    func controlTextDidChange(_ notification: Notification) {
        changed?()
        guard !completing, let editor = currentEditor() as? NSTextView else { return }
        // Straight after a sigil only a name can follow, so offer them unasked.
        let text = editor.string as NSString
        let cursor = editor.selectedRange().location
        if cursor > 0, ["#", "@"].contains(text.substring(with: NSRange(location: cursor - 1, length: 1))) {
            completing = true
            editor.complete(nil)
            completing = false
        }
    }

    func control(
        _ control: NSControl, textView: NSTextView, completions words: [String],
        forPartialWordRange range: NSRange, indexOfSelectedItem index: UnsafeMutablePointer<Int>
    ) -> [String] {
        let text = textView.string
        let cursor = TextOffsets.bytes(textView.selectedRange().location, in: text)
        guard let found = try? core.lumenna.completeText(text: text, cursor: cursor, syntax: syntax),
              !found.candidates.isEmpty else { return [] }
        // What AppKit will replace, against what the core would: drop from each candidate
        // whatever lies before AppKit's range, such as the `#` it does not count as the word.
        let start = TextOffsets.utf16(found.start, in: text)
        let skip = max(0, range.location - start)
        Announcer.say(found.announcement)
        return found.candidates.map { candidate in
            let units = Array(candidate.text.utf16)
            return skip < units.count ? String(decoding: units[skip...], as: UTF16.self) : candidate.text
        }
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        if selector == #selector(NSResponder.insertNewline(_:)) {
            submitted?()
            return true
        }
        return false
    }
}
