import UIKit

/// Adding a task the way it would be said.
///
/// The readback under the field is what a sighted user gets from inline highlighting: what will
/// be saved, with the date resolved. It updates as the text changes but is not spoken until the
/// task is added, when the same sentence is the announcement.

final class QuickAddViewController: UIViewController {
    private let core: Core
    private let added: (Change) -> Void

    private let field = LineEntry(name: "New task")
    private let readback = UILabel()
    private lazy var completions = CompletionBar(core: core, syntax: .quickAdd, field: field)
    private var addButton: UIBarButtonItem!

    private let initial: String

    init(core: Core, initial: String = "", added: @escaping (Change) -> Void) {
        self.core = core
        self.initial = initial
        self.added = added
        super.init(nibName: nil, bundle: nil)
        title = "New Task"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        field.placeholder = "write the chapter tomorrow p1 #Work"
        field.autocapitalizationType = .sentences
        field.inputAccessoryView = completions
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
            title: "Add", primaryAction: UIAction { [weak self] _ in self?.add() }
        )
        addButton.style = .done
        addButton.isEnabled = false
        navigationItem.rightBarButtonItem = addButton
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if field.text.isEmpty && !initial.isEmpty {
            field.text = initial
            field.delegate?.textViewDidChange?(field)
        }
        field.becomeFirstResponder()
    }

    private func textChanged() {
        let text = field.text ?? ""
        completions.update()
        addButton.isEnabled = !text.trimmingCharacters(in: .whitespaces).isEmpty
        guard addButton.isEnabled, let preview = try? core.lumenna.previewTask(text: text) else {
            readback.text = nil
            return
        }
        // Everything worth saying before confirming, errors included: there is no squiggle
        // under the text, so this is the only channel.
        readback.text = ([preview.announcement] + preview.diagnostics.map(\.message))
            .joined(separator: ". ")
    }

    private func add() {
        do {
            let change = try core.lumenna.addTask(text: field.text ?? "")
            let added = self.added
            dismiss(animated: true) { added(change) }
        } catch {
            showFailure(error.sentence)
        }
    }

    override var keyCommands: [UIKeyCommand]? {
        [UIKeyCommand(title: "Cancel", action: #selector(cancel), input: UIKeyCommand.inputEscape)]
    }

    @objc private func cancel() {
        dismiss(animated: true)
    }
}
