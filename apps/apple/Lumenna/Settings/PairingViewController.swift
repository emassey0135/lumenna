import Combine
import LumennaApp
import UIKit

/// Pairing this phone with another of the person's devices: `PairingModel`, the Mac's too,
/// which follows the app's pairing, drawn in UIKit.
///
/// UIKit rather than the Mac's SwiftUI screen for the system's Paste button: SwiftUI's
/// `PasteButton` is 34 points tall whatever size it is asked for, under the 44-point touch
/// target, and UIKit's `UIPasteControl` hosted in SwiftUI never pasted, whatever it targeted.
final class PairingViewController: UIViewController {
    private let model: PairingModel
    private var words: LumennaApp.PairingWords { model.words }
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
    private var following: Set<AnyCancellable> = []

    init(core: Core) {
        model = PairingModel(core: core, thisDevice: "this \(UIDevice.current.model)", name: UIDevice.current.name)
        super.init(nibName: nil, bundle: nil)
        title = model.words.title
        model.failed = { [weak self] message in self?.showFailure(message) }
        model.finished = { [weak self] in
            guard let self, self.navigationController?.topViewController === self else { return }
            self.navigationController?.popViewController(animated: true)
        }
        model.ask = { [weak self] words, answer in self?.ask(words, answer: answer) }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        status.font = .preferredFont(forTextStyle: .body)
        status.adjustsFontForContentSizeCategory = true
        status.numberOfLines = 0

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
        entry.changed = { [weak self] in self?.model.entered = self?.entry.text ?? "" }
        entry.submitted = { [weak self] in self?.join() }

        let show = button(words.wait) { [weak self] in self?.model.wait() }
        let enter = button(words.join) { [weak self] in self?.join() }
        copyCode = button(words.copyCode) { [weak self] in self?.model.copyCode() }
        copyCode.isHidden = true
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

        // The model's pairing, onto the screen: what it is doing, this device's code.
        model.$status.sink { [weak self] text in self?.status.text = text }.store(in: &following)
        model.$mine.sink { [weak self] mine in
            self?.code.text = mine
            self?.code.isHidden = mine == nil
            self?.copyCode.isHidden = mine == nil
        }.store(in: &following)
        model.$newsworthy.dropFirst().sink { [weak self] _ in
            // A problem, or a code to give: VoiceOver goes there, once it is drawn.
            DispatchQueue.main.async { UIAccessibility.post(notification: .layoutChanged, argument: self?.status) }
        }.store(in: &following)
        model.open()
    }

    private func layOutEntryRow() {
        let large = traitCollection.preferredContentSizeCategory.isAccessibilityCategory
        entryRow.axis = large ? .vertical : .horizontal
        entryRow.alignment = large ? .leading : .center
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        // Leaving the screen gives up, which ends the wait for the other device, then the screen.
        if isMovingFromParent { model.cancel() }
    }

    /// Pairs with the code typed or pasted in, the field's text as it stands: a paste changes
    /// it without the field saying so.
    private func join() {
        model.entered = entry.text
        if !entry.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { entry.resignFirstResponder() }
        model.join()
    }

    private func button(_ title: String, action: @escaping () -> Void) -> UIButton {
        var configuration = UIButton.Configuration.filled()
        configuration.title = title
        return UIButton(configuration: configuration, primaryAction: UIAction { _ in action() })
    }

    /// Asks whether the words match, in the system's alert. The pairing waits for the answer.
    private func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        let said = self.words
        let alert = UIAlertController(
            title: said.matchTitle,
            message: "\(said.matchMessage) \(words.joined(separator: ", "))",
            preferredStyle: .alert
        )
        alert.addAction(UIAlertAction(title: said.matchNo, style: .cancel) { _ in answer(false) })
        alert.addAction(UIAlertAction(title: said.matchYes, style: .default) { _ in answer(true) })
        present(alert, animated: true)
    }
}
