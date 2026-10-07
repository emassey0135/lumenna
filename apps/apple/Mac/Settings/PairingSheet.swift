import AppKit

/// Pairing this Mac with another of the person's devices.
///
/// On one network the two find each other: both wait, and DNS-SD does the rest. Anywhere
/// else, one shows a code and the other enters it. Either way both show the same three words,
/// and only if the person says they match on both does anything get paired.
final class PairingSheet: NSViewController {
    private let core: Core
    private let status = NSTextField(wrappingLabelWithString: "")
    private let code = NSTextField(wrappingLabelWithString: "")
    private let entry = NSTextField()
    private var prompt: MacPrompt?
    /// A code entered while waiting, to join with once the wait has ended.
    private var nextCode: String?
    private var wait: NSButton!
    private var enter: NSButton!

    private init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Pair a Device"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    static func present(on window: NSWindow, core: Core) {
        let sheet = NSWindow(contentViewController: PairingSheet(core: core))
        sheet.title = "Pair a Device"
        window.beginSheet(sheet)
    }

    override func loadView() {
        status.stringValue = "On the same network, start pairing on both devices and they find each other: choose Wait for the Other Device here, and pair on the other one too. On different networks, one shows a code and the other enters it."
        code.font = .monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
        code.isSelectable = true
        code.setAccessibilityLabel("Pairing code")
        code.isHidden = true
        entry.placeholderString = "the code the other device shows, or empty for the one on the clipboard"
        entry.setAccessibilityLabel("Code from the other device")
        entry.setAccessibilityHelp("Left empty, the code on the clipboard is used.")

        wait = NSButton(title: "Wait for the Other Device", target: self, action: #selector(waitForOther))
        enter = NSButton(title: "Pair With This Code", target: self, action: #selector(pairWithCode))
        let close = NSButton(title: "Cancel", target: self, action: #selector(cancel))
        close.keyEquivalent = "\u{1b}"

        let stack = NSStackView(views: [status, wait, code, entry, enter, NSStackView(views: [NSView(), close])])
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
            view.window?.showFailure("Type or paste the code the other device shows.")
            return
        }
        // A code entered while this Mac waits to be found means the person chose the other
        // way: give up the wait, and join with the code once it has ended.
        if let waiting = prompt {
            nextCode = given
            enter.isEnabled = false
            say("Stopping the wait, then connecting with this code.")
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
        say(given == nil ? "Opening a pairing session." : "Connecting to the other device.")
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
            say(error.sentence)
        }
    }

    /// Shows this Mac's code, to read out, copy or send to the other device.
    fileprivate func show(code text: String) {
        code.stringValue = text
        code.isHidden = false
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        say("Waiting for the other device. On this network it finds this Mac by itself. On another network, enter this code there, or run lum pair followed by it. The code is copied. Waiting up to ten minutes.")
    }

    /// Asks whether the words match. The pairing thread waits for the answer.
    fileprivate func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        guard let window = view.window else {
            answer(false)
            return
        }
        let alert = NSAlert()
        alert.messageText = "Do these words match?"
        alert.informativeText = "\(words.joined(separator: ", ")). Say yes only if the other device shows the same three words."
        alert.addButton(withTitle: "No")
        alert.addButton(withTitle: "Yes, They Match")
        alert.beginSheetModal(for: window) { response in answer(response == .alertSecondButtonReturn) }
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
