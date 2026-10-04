import UIKit

/// Pairing this phone with another of the person's devices (§7).
///
/// One device shows a code and the other enters it. Then both show the same three words, and
/// only if the person says they match on both does anything get paired. Finding each other on
/// the local network without a code needs multicast, which iOS allows only with an
/// entitlement Apple grants; a code works anywhere.
final class PairingViewController: UIViewController {
    private let core: Core
    private let status = UILabel()
    private let code = UITextView()
    // Wraps rather than scrolling sideways, as every entry here does: the code is long.
    private let entry = LineEntry(name: "Code from the other device")
    private let stack = UIStackView()
    private var prompt: Prompt?

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Pair a Device"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        status.font = .preferredFont(forTextStyle: .body)
        status.adjustsFontForContentSizeCategory = true
        status.numberOfLines = 0
        status.text = "On the other device, run lum pair, or choose Pair a Device. One of you shows a code, and the other enters it."

        code.isEditable = false
        code.isScrollEnabled = false
        code.font = .monospacedSystemFont(ofSize: UIFont.preferredFont(forTextStyle: .body).pointSize, weight: .regular)
        code.adjustsFontForContentSizeCategory = true
        code.backgroundColor = .secondarySystemBackground
        code.layer.cornerRadius = 8
        code.accessibilityLabel = "Pairing code"
        code.isHidden = true

        entry.placeholder = "the code the other device shows"
        entry.autocapitalizationType = .none
        entry.autocorrectionType = .no
        entry.spellCheckingType = .no

        let show = button("Show a Code") { [weak self] in self?.start(code: nil) }
        let enter = button("Pair With This Code") { [weak self] in self?.pairWithEnteredCode() }
        entry.submitted = { [weak self] in self?.pairWithEnteredCode() }

        stack.axis = .vertical
        stack.spacing = 16
        stack.translatesAutoresizingMaskIntoConstraints = false
        [status, show, code, entry, enter].forEach(stack.addArrangedSubview)
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
        if code.isEmpty, let pasted = UIPasteboard.general.string?.trimmingCharacters(in: .whitespacesAndNewlines) {
            code = pasted
            entry.text = pasted
        }
        guard !code.isEmpty else {
            showFailure("Type or paste the code the other device shows.")
            return
        }
        entry.resignFirstResponder()
        start(code: code)
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
        status.text = given == nil ? "Opening a pairing session." : "Connecting to the other device."
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
        switch result {
        case let .success(paired):
            Announcer.say(paired.announcement, notices: paired.notices)
            NotificationCenter.default.post(name: Core.changed, object: nil)
            navigationController?.popViewController(animated: true)
        case let .failure(error):
            status.text = error.sentence
            code.isHidden = true
            UIAccessibility.post(notification: .layoutChanged, argument: status)
        }
    }

    /// Shows this device's code, to read out, copy or send to the other device.
    fileprivate func show(code text: String) {
        status.text = "Waiting for the other device. On it, run lum pair followed by this code, or enter it under Pair a Device. Waiting up to ten minutes."
        code.text = text
        code.isHidden = false
        UIPasteboard.general.string = text
        UIAccessibility.post(notification: .layoutChanged, argument: status)
        Announcer.say("The code is copied, so it can be pasted on the other device.")
    }

    /// Asks whether the words match. The pairing thread waits for the answer.
    fileprivate func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        let alert = UIAlertController(
            title: "Do these words match?",
            message: "\(words.joined(separator: ", ")). Say yes only if the other device shows the same three words.",
            preferredStyle: .alert
        )
        alert.addAction(UIAlertAction(title: "No", style: .cancel) { _ in answer(false) })
        alert.addAction(UIAlertAction(title: "Yes, They Match", style: .default) { _ in answer(true) })
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
