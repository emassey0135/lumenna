import AppKit
import LumennaApp

/// Pairing this Mac with another of the person's devices: the app's pairing
/// (`ViewModel.devices.pairing`), which runs it and says each step.
///
/// On one network the two find each other: both wait, and DNS-SD does the rest. Anywhere
/// else, one shows a code and the other enters it. Either way both show the same three words,
/// and only if the person says they match on both does anything get paired.
final class PairingSheet: NSViewController {
    private let core: Core
    /// Every sentence and button, the core's.
    private let words = LumennaApp.pairingWords(thisDevice: "this Mac", local: true, capitals: .title)
    private let status = NSTextField(wrappingLabelWithString: "")
    private let code = NSTextField(wrappingLabelWithString: "")
    private let entry = NSTextField()
    private var copyCode: NSButton!
    private var observer: NSObjectProtocol?
    /// The words asked about, so they are asked once.
    private var askedWords: [String]?
    /// Whether a pairing began here, so its end closes the sheet.
    private var began = false

    private init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = words.title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    static func present(on window: NSWindow, core: Core) {
        let screen = PairingSheet(core: core)
        let sheet = NSWindow(contentViewController: screen)
        sheet.title = screen.words.title
        window.beginSheet(sheet)
    }

    override func loadView() {
        status.stringValue = words.intro
        code.font = .monospacedSystemFont(ofSize: NSFont.systemFontSize, weight: .regular)
        code.isSelectable = true
        code.setAccessibilityLabel(words.myCode)
        code.isHidden = true
        // Pasted with Command-V, or typed. An empty field never reads the clipboard: nobody
        // could tell it would, and it sent whatever happened to be copied.
        entry.setAccessibilityLabel(words.theirCode)

        let wait = NSButton(title: words.wait, target: self, action: #selector(waitForOther))
        let enter = NSButton(title: words.join, target: self, action: #selector(pairWithCode))
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
        if core.app.view.devices == nil { core.app.send(.devices(.show)) }
        core.app.send(.devices(.pair(.open(Host.current().localizedName ?? "Mac"))))
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.follow()
        }
    }

    @objc private func waitForOther() {
        began = true
        core.app.send(.devices(.pair(.wait)))
    }

    /// Copies this Mac's code, which only Copy Code does.
    @objc private func copyTheCode() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(code.stringValue, forType: .string)
        Announcer.say(words.copied)
    }

    /// Pairs with the code typed or pasted in. A code entered while this Mac waits means the
    /// person chose the other way: the app gives up the wait and joins with the code once it has
    /// ended.
    @objc private func pairWithCode() {
        let given = entry.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !given.isEmpty else {
            view.window?.showFailure(words.needCode)
            return
        }
        began = true
        core.app.send(.devices(.pair(.code(given))))
        core.app.send(.devices(.pair(.join)))
    }

    /// The app's pairing, onto the sheet: what it is doing, this Mac's code, the words to
    /// compare; closed once it has paired, which the app says.
    private func follow() {
        guard let pairing = core.app.view.devices?.pairing else {
            if began {
                began = false
                endSheet()
            }
            return
        }
        let text: String
        if let problem = pairing.problem {
            text = problem
        } else if pairing.mine != nil {
            text = words.waiting
        } else if pairing.running {
            text = pairing.waiting ? words.opening : words.connecting
        } else {
            text = words.intro
        }
        if status.stringValue != text {
            status.stringValue = text
            if pairing.problem != nil || pairing.mine != nil { Announcer.say(text) }
        }
        // Copied only when asked, by Copy Code: the clipboard is the person's.
        code.stringValue = pairing.mine ?? ""
        code.isHidden = pairing.mine == nil
        copyCode.isHidden = pairing.mine == nil
        if let three = pairing.words, three != askedWords {
            askedWords = three
            ask(three) { [weak self] yes in self?.core.app.send(.devices(.pair(.confirm(yes)))) }
        } else if pairing.words == nil {
            askedWords = nil
        }
    }

    /// Asks whether the words match. The pairing waits for the answer.
    private func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
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
            self?.status.stringValue = yes ? said.finishing : said.refusing
            answer(yes)
        }
    }

    /// Giving up ends the wait for the other device, then the screen.
    @objc private func cancel() {
        began = false
        core.app.send(.devices(.pair(.cancel)))
        core.app.send(.devices(.pair(.cancel)))
        endSheet()
    }

    private func endSheet() {
        guard let sheet = view.window, let parent = sheet.sheetParent else { return }
        parent.endSheet(sheet)
    }
}
