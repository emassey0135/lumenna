import UIKit

/// Pairing this phone with another of the person's devices.
///
/// On one network the two find each other: both wait, and Bonjour does the rest. Anywhere
/// else, one shows a code and the other enters it. Either way both show the same three words,
/// and only if the person says they match on both does anything get paired.
final class PairingViewController: UIViewController {
    private let core: Core
    /// Every sentence and button, the core's. Another device finds this one on the network by
    /// itself: the system's responder advertises it.
    private let words = pairingWords(thisDevice: "this \(UIDevice.current.model)", local: true)
    private let status = UILabel()
    private let code = UITextView()
    private var copyCode: UIButton!
    // Wraps rather than scrolling sideways, as every entry here does: the code is long.
    private lazy var entry = LineEntry(name: words.theirCode)
    private let stack = UIStackView()
    private var prompt: Prompt?
    /// A code entered while waiting, to join with once the wait has ended.
    private var nextCode: String?

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = words.title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        status.font = .preferredFont(forTextStyle: .body)
        status.adjustsFontForContentSizeCategory = true
        status.numberOfLines = 0
        status.text = words.intro

        code.isEditable = false
        code.isScrollEnabled = false
        code.font = .monospacedSystemFont(ofSize: UIFont.preferredFont(forTextStyle: .body).pointSize, weight: .regular)
        code.adjustsFontForContentSizeCategory = true
        code.backgroundColor = .secondarySystemBackground
        code.layer.cornerRadius = 8
        code.accessibilityLabel = words.myCode
        code.isHidden = true

        // Also the VoiceOver hint: left empty, the clipboard's code is used.
        entry.placeholder = words.emptyMeans
        entry.autocapitalizationType = .none
        entry.autocorrectionType = .no
        entry.spellCheckingType = .no

        let show = button(words.wait) { [weak self] in self?.start(code: nil) }
        let enter = button(words.join) { [weak self] in self?.pairWithEnteredCode() }
        copyCode = button(words.copyCode) { [weak self] in self?.copyTheCode() }
        copyCode.isHidden = true
        entry.submitted = { [weak self] in self?.pairWithEnteredCode() }

        stack.axis = .vertical
        stack.spacing = 16
        stack.translatesAutoresizingMaskIntoConstraints = false
        [status, show, code, copyCode, entry, enter].forEach(stack.addArrangedSubview)
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll)
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 16),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -16),
            stack.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: view.layoutMarginsGuide.trailingAnchor),
        ])
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        // Leaving the screen gives up, which ends the wait for the other device.
        if isMovingFromParent {
            prompt?.cancel()
        }
    }

    /// Pairs with the code typed in — or, if nothing was typed, the one on the clipboard,
    /// which is how a code sent from the other device usually arrives.
    private func pairWithEnteredCode() {
        var code = entry.text.trimmingCharacters(in: .whitespacesAndNewlines)
        // This device's own code, copied while it waits, is never the other device's.
        if code.isEmpty, let pasted = UIPasteboard.general.string?.trimmingCharacters(in: .whitespacesAndNewlines),
           self.code.isHidden || pasted != self.code.text {
            code = pasted
            entry.text = pasted
        }
        guard !code.isEmpty else {
            showFailure(words.needCode)
            return
        }
        entry.resignFirstResponder()
        // A code entered while this device waits to be found means the person chose the
        // other way: give up the wait, and join with the code once it has ended.
        if let waiting = prompt {
            nextCode = code
            status.text = words.switching
            Announcer.say(status.text ?? "")
            waiting.cancel()
            return
        }
        start(code: code)
    }

    /// Copies this device's code again, as when it was shown.
    private func copyTheCode() {
        UIPasteboard.general.string = code.text
        Announcer.say(words.copied)
    }

    private func button(_ title: String, action: @escaping () -> Void) -> UIButton {
        var configuration = UIButton.Configuration.filled()
        configuration.title = title
        return UIButton(configuration: configuration, primaryAction: UIAction { _ in action() })
    }

    private func start(code given: String?) {
        guard prompt == nil else { return }
        let prompt = Prompt(screen: self)
        self.prompt = prompt
        status.text = given == nil ? words.opening : words.connecting
        Announcer.say(status.text ?? "")
        let lumenna = core.lumenna
        let name = UIDevice.current.name
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let result = Result {
                try lumenna.pair(code: given, reach: .internet, name: name, platform: "ios", prompt: prompt)
            }
            DispatchQueue.main.async { self?.finished(result) }
        }
    }

    private func finished(_ result: Result<PairedWith, Error>) {
        prompt = nil
        if let next = nextCode {
            nextCode = nil
            code.isHidden = true
            copyCode.isHidden = true
            start(code: next)
            return
        }
        switch result {
        case let .success(paired):
            Announcer.say(paired.announcement, notices: paired.notices)
            NotificationCenter.default.post(name: Core.changed, object: nil)
            navigationController?.popViewController(animated: true)
        case let .failure(error):
            status.text = error.sentence
            code.isHidden = true
            copyCode.isHidden = true
            UIAccessibility.post(notification: .layoutChanged, argument: status)
        }
    }

    /// Shows this device's code, to read out, copy or send to the other device.
    fileprivate func show(code text: String) {
        status.text = words.waiting
        code.text = text
        code.isHidden = false
        copyCode.isHidden = false
        UIPasteboard.general.string = text
        UIAccessibility.post(notification: .layoutChanged, argument: status)
        Announcer.say(words.copied)
    }

    /// Asks whether the words match. The pairing thread waits for the answer.
    fileprivate func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        let said = self.words
        let alert = UIAlertController(
            title: said.matchTitle,
            message: "\(said.matchMessage) \(words.joined(separator: ", "))",
            preferredStyle: .alert
        )
        // What happens next is said while the devices finish, in the core's words.
        let answered = { [weak self] (yes: Bool) in
            self?.status.text = yes ? said.finishing : said.refusing
            Announcer.say(self?.status.text ?? "")
            answer(yes)
        }
        alert.addAction(UIAlertAction(title: said.matchNo, style: .cancel) { _ in answered(false) })
        alert.addAction(UIAlertAction(title: said.matchYes, style: .default) { _ in answered(true) })
        present(alert, animated: true)
    }
}

/// The pairing's questions, asked on the main thread while the pairing thread waits.
private final class Prompt: PairingPrompt, @unchecked Sendable {
    private weak var screen: PairingViewController?
    private let lock = NSLock()
    private var cancelled = false

    init(screen: PairingViewController) {
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
