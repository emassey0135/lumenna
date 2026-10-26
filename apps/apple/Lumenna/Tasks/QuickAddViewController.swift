import LumennaApp
import UIKit

/// Adding a task the way it would be said: the app's quick-add line (`ViewModel.line`), which
/// starts with what the place shown adds (`#Work ` in a project's list).
///
/// The readback under the field is what a sighted user gets from inline highlighting: what will
/// be saved, with the date resolved. It updates as the text changes but is not spoken until the
/// task is added, when the app says the same sentence and puts the task in hand.

final class QuickAddViewController: UIViewController {
    private let core: Core

    private let field = LineEntry(name: QuickAddText.label)
    private let readback = UILabel()
    private lazy var completions = CompletionBar(core: core, syntax: .quickAdd, field: field)
    private var addButton: UIBarButtonItem!

    private var observer: NSObjectProtocol?
    /// Whether the line was added, so closing does not also put it away.
    private var addedLine = false

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = QuickAddText.title
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    /// Called once it has gone, added or cancelled: on iPad a sheet's closing does not make
    /// the list appear again, so the list takes keyboard commands back here.
    var closed: () -> Void = {}

    // Add and Cancel both close it here; a sheet swiped away closes in
    // `presentationControllerDidDismiss`.
    override func dismiss(animated: Bool, completion: (() -> Void)? = nil) {
        let closed = self.closed
        closeLine()
        super.dismiss(animated: animated) {
            completion?()
            closed()
        }
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        field.placeholder = QuickAddText.example
        field.autocapitalizationType = .sentences
        field.inputAccessoryView = completions
        completions.offerList(on: field)
        field.changed = { [weak self] in self?.textChanged() }
        field.selectionChanged = { [weak self] in self?.completions.update() }
        field.submitted = { [weak self] in
            if self?.addButton.isEnabled == true {
                self?.add()
            }
        }

        readback.font = .preferredFont(forTextStyle: .subheadline)
        readback.adjustsFontForContentSizeCategory = true
        readback.textColor = .quietLabel
        readback.numberOfLines = 0

        let stack = UIStackView(arrangedSubviews: [field, readback])
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            stack.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: view.layoutMarginsGuide.trailingAnchor),
        ])

        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) }
        )
        addButton = UIBarButtonItem(
            title: QuickAddText.add, primaryAction: UIAction { [weak self] _ in self?.add() }
        )
        addButton.style = .done
        addButton.isEnabled = false
        navigationItem.rightBarButtonItem = addButton
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.drawReadback()
        }
        // The line opens with what the place adds.
        core.app.send(.typing(.open))
        if let line = core.app.current.view.line, !line.text.isEmpty {
            field.text = line.text
            textChanged()
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        navigationController?.presentationController?.delegate = self
        field.becomeFirstResponder()
    }

    private func textChanged() {
        let text = field.text ?? ""
        completions.update()
        addButton.isEnabled = !text.trimmingCharacters(in: .whitespaces).isEmpty
        let cursor = field.selectedRange.location == NSNotFound ? text.utf16.count : field.selectedRange.location
        let bytes = String(text.utf16.prefix(cursor)).map { $0.utf8.count } ?? text.utf8.count
        core.app.send(.typing(.text(text: text, cursor: UInt32(bytes))))
    }

    /// What the line will add, the app's: everything worth saying before confirming, errors
    /// included, since there is no squiggle under the text, so this is the only channel.
    private func drawReadback() {
        let line = core.app.view.line
        readback.text = addButton.isEnabled ? line?.readback : nil
    }

    private func add() {
        addedLine = true
        // The app adds it, says what it added and puts it in hand in the list.
        core.app.send(.quickAdd(field.text ?? ""))
        dismiss(animated: true)
    }

    /// Puts the line away in the app, unless it was added.
    private func closeLine() {
        guard !addedLine else { return }
        addedLine = true
        core.app.send(.typing(.close))
    }

    override var keyCommands: [UIKeyCommand]? {
        // Down offers what fits, as on every keyboard: the line has no line below to move to.
        let down = UIKeyCommand(title: CompletionWords.show, action: #selector(showCompletions), input: UIKeyCommand.inputDownArrow)
        down.wantsPriorityOverSystemBehavior = true
        return [UIKeyCommand(title: QuickAddText.cancel, action: #selector(cancel), input: UIKeyCommand.inputEscape), down]
    }

    @objc private func showCompletions() {
        completions.showList()
    }

    @objc private func cancel() {
        dismiss(animated: true)
    }
}

extension QuickAddViewController: UIAdaptivePresentationControllerDelegate {
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        closeLine()
        closed()
    }
}
