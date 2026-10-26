import LumennaApp
import SwiftUI
import UIKit

/// A question asking for a day — Go to Day, Another Day's — the app's question, its words the
/// core's: the day typed, the calendar's, and why the last answer was refused. It stays open
/// while the app refuses what was given, saying why with the typing kept, and goes once the app
/// has taken an answer.
final class DayPickerModel: ObservableObject {
    let label: String
    let hint: String
    @Published var typed: String
    @Published var date: Date
    /// Why the last day given could not be read, in the core's words.
    @Published var problem: String?
    /// Moves each time a day is refused, for VoiceOver to go to why.
    @Published private(set) var refusals = 0
    /// Where the answer goes, once: the day typed if there is one, else the calendar's as ISO;
    /// nil is cancelled.
    private var answer: ((String?) -> Void)?
    /// Closes the sheet.
    var close: () -> Void = {}
    private let app: AppModel

    init(app: AppModel, label: String, hint: String, typed: String, problem: String?, day: Date, answered: @escaping (String?) -> Void) {
        self.app = app
        self.label = label
        self.hint = hint
        self.typed = typed
        self.problem = problem
        date = day
        answer = answered
    }

    /// Asked again: the app refused the last answer, and says why.
    func askAgain(problem: String?, answered: @escaping (String?) -> Void) {
        self.problem = problem
        answer = answered
        refusals += 1
    }

    /// Whether it waits for an answer: false once one is given, until asked again.
    var waiting: Bool { answer != nil }

    /// Answers with the day typed if there is one, else the calendar's. The app reads it: a day
    /// it cannot read asks again (`askAgain`), and once it has asked nothing more, the sheet goes.
    func goThere() {
        guard waiting else { return }
        let typed = typed.trimmingCharacters(in: .whitespacesAndNewlines)
        give(typed.isEmpty ? Clock.isoDay(date) : typed)
        app.whenStill { [weak self] in
            guard let self, !self.waiting else { return }
            self.close()
        }
    }

    /// Answers nothing: Cancel, or the sheet swiped away.
    func cancel() {
        give(nil)
    }

    private func give(_ given: String?) {
        let answer = self.answer
        self.answer = nil
        answer?(given)
    }
}

/// Choosing a day: a day typed in the core's words ("next friday", "12 October"), or the
/// system's own calendar, in a sheet.
struct DayPickerScreen: View {
    @ObservedObject var model: DayPickerModel
    @AccessibilityFocusState private var refused: Bool

    var body: some View {
        // Scrolls, so the calendar is still reached under the field at the largest sizes.
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                // Wraps rather than scrolling sideways at large sizes, as every line here does.
                TextField(model.label, text: $model.typed, axis: .vertical)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityLabel(model.label)
                    // What it takes, said by VoiceOver as the hint and shown beneath for sight.
                    .accessibilityHint(model.hint)
                    .onChange(of: model.typed) { _, text in
                        // Return, which a wrapping field takes as a new line, goes there.
                        guard text.contains("\n") else { return }
                        model.typed = text.replacingOccurrences(of: "\n", with: "")
                        model.goThere()
                    }
                Divider()
                FormParts.caption(model.hint)
                    .accessibilityHidden(true)
                if let problem = model.problem {
                    Text(problem)
                        .font(.footnote)
                        .foregroundStyle(Color.warningLabel)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityFocused($refused)
                }
                DatePicker(model.label, selection: $model.date, displayedComponents: .date)
                    .datePickerStyle(.graphical)
                    .tint(Color.lumennaTint)
                    .padding(.top, 8)
            }
            .padding()
        }
        .onChange(of: model.refusals, initial: true) { _, _ in
            // Refused: VoiceOver goes to why, once it is on screen.
            if model.problem != nil { DispatchQueue.main.async { refused = true } }
        }
    }
}

/// A day asked for, as a sheet: the SwiftUI screen under UIKit's bar, with Cancel and the
/// question's own answer button.
final class DayPickerViewController: UIHostingController<DayPickerScreen>, UIAdaptivePresentationControllerDelegate {
    let model: DayPickerModel

    init(app: AppModel, title: String, label: String, hint: String, typed: String, problem: String?, day: Date, yes: String, answered: @escaping (String?) -> Void) {
        let model = DayPickerModel(app: app, label: label, hint: hint, typed: typed, problem: problem, day: day, answered: answered)
        self.model = model
        super.init(rootView: DayPickerScreen(model: model))
        self.title = title
        model.close = { [weak self] in self?.dismiss(animated: true) }
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in
                self?.model.cancel()
                self?.dismiss(animated: true)
            }
        )
        let go = UIBarButtonItem(title: yes, primaryAction: UIAction { [weak model] _ in model?.goThere() })
        go.style = .done
        navigationItem.rightBarButtonItem = go
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Swiped away, the question is cancelled, as Cancel cancels it.
        navigationController?.presentationController?.delegate = self
    }

    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        model.cancel()
    }

    /// Whether it is still up, to be asked again in place.
    var isOpen: Bool { view.window != nil && !(navigationController?.isBeingDismissed ?? true) }
}
