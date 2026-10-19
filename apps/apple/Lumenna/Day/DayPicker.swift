import LumennaApp
import UIKit

/// Choosing a day to go to: a day typed in the core's words ("next friday", "12 October"),
/// or the system's own calendar, in a sheet — the app's Go to Day, its words the core's.
final class DayPickerViewController: UIViewController {
    private let question: (title: String, label: String, hint: String, yes: String) = {
        if case let .text(title, label, _, hint, _, yes, _) = LumennaApp.goToDayQuestion() { return (title, label, hint, yes) }
        return ("Go to Day", "Day", "", "Go")
    }()
    private let picker = UIDatePicker()
    private lazy var entry = LineEntry(name: question.label)
    private let hint = UILabel()
    private let problem = UILabel()
    /// Reads the day named, an ISO date from the calendar or what was typed, and gives back
    /// what shows it, run once the sheet has gone. A day the core cannot read throws, and the
    /// sheet stays open saying why.
    private let go: (String) throws -> () -> Void

    init(showing day: Date, go: @escaping (String) throws -> () -> Void) {
        self.go = go
        super.init(nibName: nil, bundle: nil)
        title = question.title
        picker.date = day
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        // What it takes, said by VoiceOver as the hint and shown beneath for sight.
        entry.accessibilityHint = question.hint
        entry.autocapitalizationType = .none
        entry.autocorrectionType = .no
        entry.submitted = { [weak self] in self?.goThere() }
        for label in [hint, problem] {
            label.font = .preferredFont(forTextStyle: .footnote)
            label.adjustsFontForContentSizeCategory = true
            label.numberOfLines = 0
        }
        hint.text = question.hint
        hint.textColor = .quietLabel
        hint.isAccessibilityElement = false
        problem.textColor = .warningLabel
        problem.isHidden = true

        picker.datePickerMode = .date
        picker.preferredDatePickerStyle = .inline
        picker.tintColor = .lumennaTint
        picker.accessibilityLabel = question.label

        let stack = UIStackView(arrangedSubviews: [entry, hint, problem, picker])
        stack.axis = .vertical
        stack.spacing = 8
        stack.setCustomSpacing(16, after: problem)
        stack.translatesAutoresizingMaskIntoConstraints = false
        // Scrolls, so the calendar is still reached under the field at the largest sizes.
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        view.addSubview(scroll)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 12),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -12),
            stack.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: view.layoutMarginsGuide.trailingAnchor),
        ])
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) }
        )
        let go = UIBarButtonItem(title: question.yes, primaryAction: UIAction { [weak self] _ in self?.goThere() })
        go.style = .done
        navigationItem.rightBarButtonItem = go
    }

    /// The day typed if there is one, else the calendar's.
    private func goThere() {
        let typed = entry.text.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let show = try go(typed.isEmpty ? Clock.isoDay(picker.date) : typed)
            dismiss(animated: true, completion: show)
        } catch {
            // A day the core cannot read stays here to be put right.
            problem.text = error.sentence
            problem.isHidden = false
            UIAccessibility.post(notification: .layoutChanged, argument: problem)
        }
    }
}
