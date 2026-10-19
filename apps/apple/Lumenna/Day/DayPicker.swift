import LumennaApp
import SwiftUI
import UIKit

/// What Go to Day holds: the day typed, the calendar's, and why a typed one was refused.
final class DayPickerModel: ObservableObject {
    let question: (title: String, label: String, hint: String, yes: String) = {
        if case let .text(title, label, _, hint, _, yes, _) = LumennaApp.goToDayQuestion() { return (title, label, hint, yes) }
        return ("Go to Day", "Day", "", "Go")
    }()
    @Published var typed = ""
    @Published var date: Date
    /// Why the day typed could not be read, in the core's words.
    @Published var problem: String?
    /// Reads the day named, an ISO date from the calendar or what was typed, and gives back
    /// what shows it, run once the sheet has gone. A day the core cannot read throws.
    private let go: (String) throws -> () -> Void
    /// Closes the sheet, then runs what it is given.
    var close: (@escaping () -> Void) -> Void = { _ in }

    init(showing day: Date, go: @escaping (String) throws -> () -> Void) {
        date = day
        self.go = go
    }

    /// The day typed if there is one, else the calendar's; a day the core cannot read stays
    /// here to be put right.
    func goThere() {
        let typed = typed.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let show = try go(typed.isEmpty ? Clock.isoDay(date) : typed)
            close(show)
        } catch {
            problem = error.sentence
        }
    }
}

/// Choosing a day to go to: a day typed in the core's words ("next friday", "12 October"),
/// or the system's own calendar, in a sheet — the app's Go to Day, its words the core's.
struct DayPickerScreen: View {
    @ObservedObject var model: DayPickerModel
    @AccessibilityFocusState private var refused: Bool

    var body: some View {
        // Scrolls, so the calendar is still reached under the field at the largest sizes.
        ScrollView {
            VStack(alignment: .leading, spacing: 8) {
                // Wraps rather than scrolling sideways at large sizes, as every line here does.
                TextField(model.question.label, text: $model.typed, axis: .vertical)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityLabel(model.question.label)
                    // What it takes, said by VoiceOver as the hint and shown beneath for sight.
                    .accessibilityHint(model.question.hint)
                    .onChange(of: model.typed) { _, text in
                        // Return, which a wrapping field takes as a new line, goes there.
                        guard text.contains("\n") else { return }
                        model.typed = text.replacingOccurrences(of: "\n", with: "")
                        model.goThere()
                    }
                Divider()
                FormParts.caption(model.question.hint)
                    .accessibilityHidden(true)
                if let problem = model.problem {
                    Text(problem)
                        .font(.footnote)
                        .foregroundStyle(Color.warningLabel)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityFocused($refused)
                }
                DatePicker(model.question.label, selection: $model.date, displayedComponents: .date)
                    .datePickerStyle(.graphical)
                    .tint(Color.lumennaTint)
                    .padding(.top, 8)
            }
            .padding()
        }
        .onChange(of: model.problem) { _, problem in
            // VoiceOver goes to why, once it is on screen.
            if problem != nil { DispatchQueue.main.async { refused = true } }
        }
    }
}

/// Go to Day's sheet: the SwiftUI screen under UIKit's bar, with Cancel and the core's Go.
final class DayPickerViewController: UIHostingController<DayPickerScreen> {
    private let model: DayPickerModel

    init(showing day: Date, go: @escaping (String) throws -> () -> Void) {
        let model = DayPickerModel(showing: day, go: go)
        self.model = model
        super.init(rootView: DayPickerScreen(model: model))
        title = model.question.title
        model.close = { [weak self] then in self?.dismiss(animated: true, completion: then) }
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) }
        )
        let go = UIBarButtonItem(title: model.question.yes, primaryAction: UIAction { [weak model] _ in model?.goThere() })
        go.style = .done
        navigationItem.rightBarButtonItem = go
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }
}
