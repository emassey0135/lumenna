import AppKit
import LumennaApp

/// A text field that offers what could be typed next, through the system's own
/// completion: Down, or Option-Escape or F5 as in any Mac text field. Never by itself, not even
/// after a `#` or `@`: a list opening interrupts someone typing a name straight through, so it
/// opens only when asked, as on every app.
///
/// The app says what goes at the cursor and which span it replaces, in UTF-8 bytes; AppKit
/// completes the word it thinks is partial, in UTF-16. The two are reconciled here.
final class CompletingField: NSTextField, NSTextFieldDelegate {
    private let core: Core
    private let syntax: LumennaApp.Syntax
    /// Called after every change to the text, typed or completed.
    var changed: (() -> Void)?
    /// Called on Return.
    var submitted: (() -> Void)?
    private var completing = false

    init(core: Core, syntax: LumennaApp.Syntax, name: String) {
        self.core = core
        self.syntax = syntax
        super.init(frame: .zero)
        delegate = self
        setAccessibilityLabel(name)
        setAccessibilityHelp(QuickAddText.completes)
        lineBreakMode = .byWordWrapping
        usesSingleLineMode = false
        cell?.wraps = true
        cell?.isScrollable = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    func controlTextDidChange(_ notification: Notification) {
        changed?()
    }

    func control(
        _ control: NSControl, textView: NSTextView, completions words: [String],
        forPartialWordRange range: NSRange, indexOfSelectedItem index: UnsafeMutablePointer<Int>
    ) -> [String] {
        let text = textView.string
        let cursor = TextOffsets.bytes(textView.selectedRange().location, in: text)
        guard let found = try? core.app.complete(text, cursor: cursor, syntax: syntax),
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
        // Down asks, as on every keyboard; the field has no line below to move to.
        if selector == #selector(NSResponder.moveDown(_:)), !completing {
            completing = true
            textView.complete(nil)
            completing = false
            return true
        }
        return false
    }
}
