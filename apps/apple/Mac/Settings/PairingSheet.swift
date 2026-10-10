import AppKit

/// Pairing this Mac with another of the person's devices.
///
/// On one network the two find each other: both wait, and DNS-SD does the rest. Anywhere
/// else, one shows a code and the other enters it. Either way both show the same three words,
/// and only if the person says they match on both does anything get paired.
final class PairingSheet: NSViewController {
    private let core: Core
    /// Every sentence and button, the core's.
    private static let words = pairingWords(thisDevice: "this Mac", local: true)
    private var words: PairingWords { Self.words }
    private let status = NSTextField(wrappingLabelWithString: "")
    private let code = NSTextField(wrappingLabelWithString: "")
    private let entry = NSTextField()
    private var prompt: MacPrompt?
    /// A code entered while waiting, to join with once the wait has ended.
    private var nextCode: String?
    private var wait: NSButton!
    private var enter: NSButton!
    private var copyCode: NSButton!

    private init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = Self.words.title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    static func present(on window: NSWindow, core: Core) {
        let sheet = NSWindow(contentViewController: PairingSheet(core: core))
        sheet.title = words.title
        window.beginSheet(sheet)
    }

    override func loadView() {
        status.stringValue = words.intro
        code.font = .monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
        code.isSelectable = true
        code.setAccessibilityLabel(words.myCode)
        code.isHidden = true
        entry.placeholderString = words.emptyMeans
        entry.setAccessibilityLabel(words.theirCode)
        entry.setAccessibilityHelp(words.emptyMeans)

        wait = NSButton(title: words.wait, target: self, action: #selector(waitForOther))
        enter = NSButton(title: words.join, target: self, action: #selector(pairWithCode))
        copyCode = NSButton(title: words.copyCode, target: self, action: #selector(copyTheCode))
        copyCode.isHidden = true
        let close = NSButton(title: "Cancel", target: self, action: #selector(cancel))
        close.keyEquivalent = "\u{1b}"

        let stack = NSStackView(views: [status, wait, code, copyCode, entry, enter, NSStackView(views: [NSView(), close])])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 12
        stack.edgeInsets = NSEdgeInsets(top: 20, left: 20, bottom: 20, right: 20)
        for view in [status, code, entry] {
            view.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -40).isActive = true
        }
        stack.widthAnchor.constraint(equalToConstant: 480).isActive = true
        view = stack
    }

    @objc private func waitForOther() { start(code: nil) }

    /// Copies this Mac's code again, as when it was shown.
    @objc private func copyTheCode() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(code.stringValue, forType: .string)
        Announcer.say(words.copied)
    }

    /// Pairs with the code typed in — or, if nothing was typed, the one on the clipboard,
    /// which is how a code sent from the other device usually arrives.
    @objc private func pairWithCode() {
        var given = entry.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        // This Mac's own code, copied while it waits, is never the other device's.
        if given.isEmpty, let pasted = NSPasteboard.general.string(forType: .string)?.trimmingCharacters(in: .whitespacesAndNewlines),
           code.isHidden || pasted != code.stringValue {
            given = pasted
            entry.stringValue = pasted
        }
        guard !given.isEmpty else {
            view.window?.showFailure(words.needCode)
            return
        }
        // A code entered while this Mac waits to be found means the person chose the other
        // way: give up the wait, and join with the code once it has ended.
        if let waiting = prompt {
            nextCode = given
            enter.isEnabled = false
            say(words.switching)
            waiting.cancel()
            return
        }
        start(code: given)
    }

    private func start(code given: String?) {
        guard prompt == nil else { return }
        let prompt = MacPrompt(screen: self)
        self.prompt = prompt
        wait.isEnabled = false
        enter.isEnabled = given == nil
        say(given == nil ? words.opening : words.connecting)
        let lumenna = core.lumenna
        let name = Host.current().localizedName ?? "Mac"
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let result = Result {
                try lumenna.pair(code: given, reach: .internet, name: name, platform: "macos", prompt: prompt)
            }
            DispatchQueue.main.async { self?.finished(result) }
        }
    }

    private func say(_ text: String) {
        status.stringValue = text
        Announcer.say(text)
    }

    private func finished(_ result: Result<PairedWith, Error>) {
        prompt = nil
        wait.isEnabled = true
        enter.isEnabled = true
        if let next = nextCode {
            nextCode = nil
            code.isHidden = true
            copyCode.isHidden = true
            start(code: next)
            return
        }
        switch result {
        case let .success(paired):
            NotificationCenter.default.post(name: Core.changed, object: nil)
            endSheet()
            Announcer.say(paired.announcement, notices: paired.notices)
        case let .failure(error):
            code.isHidden = true
            copyCode.isHidden = true
            say(error.sentence)
        }
    }

    /// Shows this Mac's code, to read out, copy or send to the other device.
    fileprivate func show(code text: String) {
        code.stringValue = text
        code.isHidden = false
        copyCode.isHidden = false
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        say(words.waiting)
        Announcer.say(words.copied)
    }

    /// Asks whether the words match. The pairing thread waits for the answer.
    fileprivate func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        guard let window = view.window else {
            answer(false)
            return
        }
        let alert = NSAlert()
        let said = self.words
        alert.messageText = said.matchTitle
        alert.informativeText = "\(said.matchMessage) \(words.joined(separator: ", "))"
        // No is first, so the default: a stray Return never pairs.
        alert.addButton(withTitle: said.matchNo)
        alert.addButton(withTitle: said.matchYes)
        alert.beginSheetModal(for: window) { [weak self] response in
            let yes = response == .alertSecondButtonReturn
            // What happens next is said while the devices finish, in the core's words.
            self?.say(yes ? said.finishing : said.refusing)
            answer(yes)
        }
    }

    @objc private func cancel() {
        // Giving up ends the wait for the other device.
        prompt?.cancel()
        endSheet()
    }

    private func endSheet() {
        guard let sheet = view.window, let parent = sheet.sheetParent else { return }
        parent.endSheet(sheet)
    }
}

/// The pairing's questions, asked on the main thread while the pairing thread waits.
private final class MacPrompt: PairingPrompt, @unchecked Sendable {
    private weak var screen: PairingSheet?
    private let lock = NSLock()
    private var cancelled = false

    init(screen: PairingSheet) {
        self.screen = screen
    }

    func cancel() {
        lock.withLock { cancelled = true }
    }

    func showCode(code: String) {
        DispatchQueue.main.async { [weak self] in self?.screen?.show(code: code) }
    }

    func confirm(words: [String]) -> Bool {
        let answered = DispatchSemaphore(value: 0)
        var matched = false
        DispatchQueue.main.async { [weak self] in
            guard let screen = self?.screen else {
                answered.signal()
                return
            }
            screen.ask(words) { yes in
                matched = yes
                answered.signal()
            }
        }
        answered.wait()
        return matched
    }

    func isCancelled() -> Bool {
        lock.withLock { cancelled }
    }
}
