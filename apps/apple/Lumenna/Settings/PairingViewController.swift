import LumennaApp
import UIKit

/// Pairing this phone with another of the person's devices: the app's pairing
/// (`ViewModel.devices.pairing`), which runs it and says each step.
///
/// On one network the two find each other: both wait, and Bonjour does the rest. Anywhere
/// else, one shows a code and the other enters it. Either way both show the same three words,
/// and only if the person says they match on both does anything get paired.
final class PairingViewController: UIViewController {
    private let core: Core
    /// Every sentence and button, the core's. Another device finds this one on the network by
    /// itself: the system's responder advertises it.
    private let words = LumennaApp.pairingWords(thisDevice: "this \(UIDevice.current.model)", local: true, capitals: .title)
    private let status = UILabel()
    private let code = UITextView()
    private var copyCode: UIButton!
    // Wraps rather than scrolling sideways, as every entry here does: the code is long.
    private lazy var entry = LineEntry(name: words.theirCode)
    /// The system's Paste button: it pastes into the field without the "allow paste" prompt,
    /// because the person pressed it. Nothing here ever reads the clipboard by itself.
    private lazy var paste = UIPasteControl(configuration: {
        let configuration = UIPasteControl.Configuration()
        configuration.displayMode = .iconAndLabel
        configuration.cornerStyle = .medium
        // The filled buttons' colours, which pass contrast; the system's own may not.
        configuration.baseBackgroundColor = .lumennaTint
        configuration.baseForegroundColor = .white
        return configuration
    }())
    /// The field and its Paste button, side by side, or one above the other at the largest
    /// text sizes, where side by side leaves the field too narrow to read.
    private let entryRow = UIStackView()
    private let stack = UIStackView()
    private var observer: NSObjectProtocol?
    /// The words asked about, so they are asked once.
    private var askedWords: [String]?
    /// Whether this screen began a pairing, so its end closes the screen.
    private var began = false

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = words.title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        if core.app.view.devices == nil { core.app.send(.devices(.show)) }
        core.app.send(.devices(.pair(.open(UIDevice.current.name))))
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.follow()
        }

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

        entry.autocapitalizationType = .none
        entry.autocorrectionType = .no
        entry.spellCheckingType = .no

        let show = button(words.wait) { [weak self] in self?.start(code: nil) }
        let enter = button(words.join) { [weak self] in self?.pairWithEnteredCode() }
        copyCode = button(words.copyCode) { [weak self] in self?.copyTheCode() }
        copyCode.isHidden = true
        entry.submitted = { [weak self] in self?.pairWithEnteredCode() }
        paste.target = entry
        paste.setContentHuggingPriority(.required, for: .horizontal)
        paste.setContentCompressionResistancePriority(.required, for: .horizontal)
        // Never under the 44-point touch target, whatever the system draws it at.
        paste.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        paste.widthAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        entryRow.spacing = 8
        [entry, paste].forEach(entryRow.addArrangedSubview)
        layOutEntryRow()
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (self: Self, _) in
            self.layOutEntryRow()
        }

        stack.axis = .vertical
        stack.spacing = 16
        stack.translatesAutoresizingMaskIntoConstraints = false
        [status, show, code, copyCode, entryRow, enter].forEach(stack.addArrangedSubview)
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

    private func layOutEntryRow() {
        let large = traitCollection.preferredContentSizeCategory.isAccessibilityCategory
        entryRow.axis = large ? .vertical : .horizontal
        entryRow.alignment = large ? .leading : .center
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        // Leaving the screen gives up, which ends the wait for the other device, then the screen.
        if isMovingFromParent {
            core.app.send(.devices(.pair(.cancel)))
            core.app.send(.devices(.pair(.cancel)))
        }
    }

    /// Pairs with the code typed or pasted in. An empty field asks for one, the app's words: it
    /// never reads the clipboard, which brought up the system's paste prompt and sent whatever
    /// happened to be copied. A code entered while this device waits means the person chose the
    /// other way: the app gives up the wait and joins with the code once it has ended.
    private func pairWithEnteredCode() {
        let code = entry.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else {
            showFailure(words.needCode)
            return
        }
        entry.resignFirstResponder()
        began = true
        core.app.send(.devices(.pair(.code(code))))
        core.app.send(.devices(.pair(.join)))
    }

    /// Copies this device's code, which only Copy Code does.
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
        began = true
        core.app.send(.devices(.pair(.wait)))
    }

    /// The app's pairing, onto the screen: what it is doing, this device's code, the words to
    /// compare; gone once it has paired, which the app says.
    private func follow() {
        guard let pairing = core.app.view.devices?.pairing else {
            if began, navigationController?.topViewController === self {
                began = false
                navigationController?.popViewController(animated: true)
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
        if status.text != text {
            status.text = text
            if pairing.problem != nil || pairing.mine != nil { UIAccessibility.post(notification: .layoutChanged, argument: status) }
        }
        // Copied only when asked, by Copy Code: the clipboard is the person's.
        code.text = pairing.mine
        code.isHidden = pairing.mine == nil
        copyCode.isHidden = pairing.mine == nil
        if let three = pairing.words, three != askedWords {
            askedWords = three
            ask(three) { [weak self] yes in self?.core.app.send(.devices(.pair(.confirm(yes)))) }
        } else if pairing.words == nil {
            askedWords = nil
        }
    }

    /// Asks whether the words match. The pairing thread waits for the answer.
    fileprivate func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        let said = self.words
        let alert = UIAlertController(
            title: said.matchTitle,
            message: "\(said.matchMessage) \(words.joined(separator: ", "))",
            preferredStyle: .alert
        )
        // What happens next the app says while the devices finish.
        let answered = { [weak self] (yes: Bool) in
            self?.status.text = yes ? said.finishing : said.refusing
            answer(yes)
        }
        alert.addAction(UIAlertAction(title: said.matchNo, style: .cancel) { _ in answered(false) })
        alert.addAction(UIAlertAction(title: said.matchYes, style: .default) { _ in answered(true) })
        present(alert, animated: true)
    }
}
