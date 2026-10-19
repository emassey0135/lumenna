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

    var body: some View {
        _ = core.generation
        let view = core.app.view
        let lines = core.app.lines
        let day = view.place == .today ? view.day : nil
        let rows: [(item: LumennaApp.DayItem, line: LumennaApp.Line?)] = (day?.items ?? []).enumerated()
            .map { index, item in (item, index < lines.day.count ? lines.day[index] : nil) }
            .filter { !$0.item.hidden }
        return List {
            if let day {
                ForEach(rows, id: \.item.key) { row in
                    rowView(row.item, line: row.line, in: day)
                }
            }
            Section {
                Button("Add Block") { core.app.send(.block(.new(start: nil, minutes: nil))) }
                Button("Previous Day") { core.app.send(.day(.previous)) }
                if day?.today == false { Button("Today") { core.app.send(.day(.now)) } }
                Button("Next Day") { core.app.send(.day(.next)) }
                Button("Go to Day") {
                    asker.show(DayChoice(day: day.flatMap { Self.date($0.date) } ?? .now))
                }
            }
        }
        .navigationTitle(day != nil ? lines.dayTitle : "Day")
        .onAppear {
            if core.app.view.place != .today { core.app.send(.open(.today)) }
        }
    }

    @ViewBuilder
    private func rowView(_ item: LumennaApp.DayItem, line: LumennaApp.Line?, in day: LumennaApp.DayView) -> some View {
        let text = line?.text ?? ""
        let value = AppModel.spoken(detail: nil, expanded: item.expanded, level: item.level)
        switch item.kind {
        case .summary:
            // The day's heading, as every app says it: "<day>. <summary>". Tapped, it offers
            // the day's own actions, the app's, as a row offers its own.
            if day.actions.isEmpty {
                Text(text).font(.footnote)
            } else {
                Button { asker.offer(core.app.lines.dayTitle, day.actions, on: item.key) } label: { Text(text).font(.footnote) }
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityIdentifier("day.heading")
            }
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
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
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
        core.app.send(.day(.ask))
        core.app.send(.answer(.text(text: named.isEmpty ? Clock.isoDay(day) : named)))
        if let refusal = core.app.current.view.asking?.refusal {
            core.app.send(.cancel)
            problem = refusal
            Announcer.say(refusal)
            return
        }
        dismiss()
    }
}
