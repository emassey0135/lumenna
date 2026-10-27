import LumennaApp
import UIKit

/// A question whose answer is in the filter language (a saved filter's query), asked in a sheet
/// rather than an alert: an alert's field can carry no completion. The line completes as quick
/// add's does, a strip above the keyboard, and a list its Show Completions action, the strip's
/// button or Down opens; the hint and why the last answer was refused beneath it; Cancel and the
/// app's word for the answer in the bar.
final class CompletingQuestionViewController: UIViewController {
    private let field: LineEntry
    private let completions: CompletionBar
    private let note = UILabel()
    private let initial: String
    private let message: String
    private let yes: String
    private let answered: (String?) -> Void
    private var done = false

    init(core: Core, title: String, label: String, initial: String, message: String, yes: String, syntax: LumennaApp.Syntax, answered: @escaping (String?) -> Void) {
        field = LineEntry(name: label)
        completions = CompletionBar(core: core, syntax: syntax, field: field)
        self.initial = initial
        self.message = message
        self.yes = yes
        self.answered = answered
        super.init(nibName: nil, bundle: nil)
        self.title = title
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        field.text = initial
        field.autocapitalizationType = .none
        field.autocorrectionType = .no
        field.inputAccessoryView = completions
        completions.offerList(on: field)
        field.changed = { [weak self] in self?.completions.update() }
        field.selectionChanged = { [weak self] in self?.completions.update() }
        field.submitted = { [weak self] in self?.answer() }
        note.text = message
        note.numberOfLines = 0
        note.font = .preferredFont(forTextStyle: .footnote)
        note.adjustsFontForContentSizeCategory = true
        note.textColor = .quietLabel
        let stack = UIStackView(arrangedSubviews: [field, note])
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            stack.leadingAnchor.constraint(equalTo: view.readableContentGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: view.readableContentGuide.trailingAnchor),
        ])
        navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.finish(nil) })
        let go = UIBarButtonItem(title: yes, primaryAction: UIAction { [weak self] _ in self?.answer() })
        go.style = .done
        navigationItem.rightBarButtonItem = go
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        field.becomeFirstResponder()
        completions.update()
    }

    override var keyCommands: [UIKeyCommand]? {
        // Down offers what fits, as on every keyboard.
        let down = UIKeyCommand(title: CompletionWords.show, action: #selector(showCompletions), input: UIKeyCommand.inputDownArrow)
        down.wantsPriorityOverSystemBehavior = true
        return [UIKeyCommand(title: LumennaApp.cancel(sentence: false), action: #selector(cancel), input: UIKeyCommand.inputEscape), down]
    }

    @objc private func showCompletions() { completions.showList() }
    @objc private func cancel() { finish(nil) }

    private func answer() { finish(field.text ?? "") }

    private func finish(_ text: String?) {
        guard !done else { return }
        done = true
        dismiss(animated: true) { [answered] in answered(text) }
    }

    /// Swiped away: cancelled, as Cancel is.
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        if !done, presentedViewController == nil, navigationController?.isBeingDismissed ?? isBeingDismissed {
            done = true
            answered(nil)
        }
    }
}
