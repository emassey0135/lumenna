import LumennaApp
import SwiftUI

/// The day as lived, the app's day (`ViewModel.day`): its heading, then each block with the
/// sittings in it, the free time between, where now falls, and what is cancelled for the day,
/// each as the core words its line. Tapping a row offers its actions, the app's, as the
/// phone's swipe actions do: a watch's row has room for few. A block with sittings folds, by
/// its own Expand or Collapse. Going to another day, and a new block, are buttons at the foot.
struct DayView: View {
    @EnvironmentObject private var core: WatchCore
    @EnvironmentObject private var asker: WatchAsker
    @StateObject private var model: DayModel
    @AccessibilityFocusState private var spoken: String?

    init(core: WatchCore) {
        _model = StateObject(wrappedValue: DayModel(core: core))
    }

    var body: some View {
        List {
            if !model.heading.isEmpty { heading }
            ForEach(model.shown) { row in
                rowView(row.item, line: row.line)
                    .accessibilityFocused($spoken, equals: row.id)
            }
            Section {
                Button("Add Block") { core.app.send(.block(.new(start: nil, minutes: nil))) }
                Button("Previous Day") { model.move(.previous) }
                if core.app.view.day?.today == false { Button("Today") { model.move(.now) } }
                Button("Next Day") { model.move(.next) }
                Button("Go to Day") {
                    asker.show(DayChoice(model: model, day: model.date.flatMap(Self.date) ?? .now))
                }
            }
        }
        .navigationTitle(model.title)
        // Today opening on now, unless the app shows the day already.
        .onAppear { model.appear() }
        .onChange(of: model.landed?.count) { _, _ in
            // The app moved the row in hand: VoiceOver goes there, once the row is drawn.
            guard let key = model.landed?.key else { return }
            DispatchQueue.main.async { spoken = key }
        }
        .onChange(of: spoken) { _, key in
            if let key { model.moved(to: key) }
        }
    }

    /// The day's heading, as every app says it: "<day>. <summary>". Tapped, it offers the
    /// day's own actions, the app's, as a row offers its own.
    @ViewBuilder private var heading: some View {
        if model.actions.isEmpty {
            Text(model.heading).font(.footnote)
        } else {
            Button {
                // In the order every row's are offered: what removes something last.
                let ordered = model.actions.filter { !$0.destructive } + model.actions.filter(\.destructive)
                asker.show(ChoicePrompt(title: model.title, choices: ordered.map { action in
                    (action.title, { [model] in model.perform(action) })
                }))
            } label: { Text(model.heading).font(.footnote) }
                .accessibilityAddTraits(.isHeader)
                .accessibilityIdentifier("day.heading")
        }
    }

    @ViewBuilder
    private func rowView(_ item: LumennaApp.DayItem, line: LumennaApp.Line?) -> some View {
        let text = line?.text ?? ""
        let value = AppModel.spoken(detail: nil, expanded: item.expanded, level: item.level)
        switch item.kind {
        case .summary:
            EmptyView()
        case .now:
            Text(text).font(.headline)
        case .sitting:
            Button { offer(item, line: line) } label: { Text(text) }
                .accessibilityValue(value)
                .padding(.leading, 8)
        case .block:
            Button { offer(item, line: line) } label: { Text(text) }
                .accessibilityAddTraits(.isHeader)
                .accessibilityValue(value)
        case .free, .cancelled:
            Button { offer(item, line: line) } label: { Text(text) }
        }
    }

    /// A row's actions, offered when it is tapped: a sitting's Edit Task Details opens its task.
    private func offer(_ item: LumennaApp.DayItem, line: LumennaApp.Line?) {
        asker.offer(line?.title ?? "", item.actions, on: item.key)
    }

    static func date(_ iso: String) -> Date? {
        try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse(iso)
    }
}

/// A day to go to, chosen as the phone chooses one: a day said or typed in the core's words
/// ("next friday", "12 October") from the system's input screen, or a date picker, then Go —
/// the answer to the app's Go to Day.
private struct DayChoice: View {
    @Environment(\.dismiss) private var dismiss
    let model: DayModel
    @State var day: Date
    @State private var typed = ""
    @State private var problem: String?
    private let question: (title: String, label: String, hint: String, yes: String) = {
        if case let .text(title, label, _, hint, _, yes, _) = LumennaApp.goToDayQuestion() { return (title, label, hint, yes) }
        return ("Go to Day", "Day", "", "Go")
    }()

    var body: some View {
        List {
            Section {
                TextField(question.label, text: $typed)
                    .textInputAutocapitalization(.never)
                    // Named explicitly: with text in it, a field's title gives way to the text.
                    .accessibilityLabel(question.label)
                    .accessibilityHint(question.hint)
                    .onSubmit { goTo(typed) }
                if let problem {
                    Text(problem).foregroundStyle(Color.warningLabel)
                }
            } footer: {
                FormParts.caption(question.hint)
            }
            Section {
                NamedDatePicker(question.label, selection: $day, displayedComponents: .date)
                Button(question.yes) { goTo(typed) }
            }
        }
        .navigationTitle(question.title)
    }

    /// The day typed if there is one, else the picker's. A day the app cannot read stays here,
    /// saying why.
    private func goTo(_ text: String) {
        let named = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if let refusal = model.goTo(named.isEmpty ? Clock.isoDay(day) : named) {
            problem = refusal
            Announcer.say(refusal)
            return
        }
        dismiss()
    }
}
