import SwiftUI

/// The day as lived: its summary, then each block with the sittings in it, the free time
/// between, where now falls, and what is cancelled for the day, as the phone's day lists
/// them. Tapping a row offers its actions, the core's, as the phone's swipe actions do: a
/// watch's row has room for few. A block with sittings folds, as on the phone. Going
/// to another day, and a new block, are buttons at the foot.
struct DayView: View {
    @EnvironmentObject private var core: WatchCore
    /// The day shown, or nil for today, which follows midnight.
    @State private var chosen: Date?
    /// A sitting's task, opened by its Edit Task Details.
    @State private var opened: TaskOpened?
    @StateObject private var asker: WatchAsker
    @State private var form: BlockFormModel?
    @State private var folding = Folding()

    init(core: WatchCore) {
        _asker = StateObject(wrappedValue: WatchAsker(core: core))
    }

    /// A row of the day, as folding sees it.
    private enum Row {
        case block(PlanBlock)
        case sitting(PlanAssignment, in: PlanBlock)
        case free(start: String, end: String, minutes: UInt32, title: String, details: [String], actions: [Action])
        case now(time: String, title: String)

        var depth: Int {
            if case .sitting = self { return 1 }
            return 0
        }

        /// Only a block has anything under it.
        var key: String {
            switch self {
            case let .block(block): "block:\(block.id)"
            case let .sitting(sitting, _): "sitting:\(sitting.id)"
            case let .free(start, _, _, _, _, _): "free:\(start)"
            case .now: "now"
            }
        }
    }

    var body: some View {
        let plan = day
        let shown = plan.map { folding.shown(rows($0), depth: \.depth, key: \.key) } ?? []
        List {
            if let plan {
                // The day's heading, as every app says it: "<day>. <summary>".
                Text(plan.summary.isEmpty ? plan.announcement : "\(Clock.spokenDay(plan.date)). \(plan.summary)")
                    .font(.footnote)
                ForEach(Array(shown.enumerated()), id: \.element.item.key) { _, row in
                    rowView(row, in: plan)
                }
                ForEach(plan.cancelled, id: \.series) { block in
                    button(([Clock.time(block.start), block.title] + block.details).joined(separator: ", ")) {
                        asker.offer(block.title, block.actions)
                    }
                }
            }
            Section {
                Button("Add Block") { addBlock(on: plan) }
                Button("Previous Day") { step(-1, from: plan) }
                if chosen != nil { Button("Today") { chosen = nil } }
                Button("Next Day") { step(1, from: plan) }
                Button("Go to Day") {
                    asker.show(DayChoice(day: chosen ?? .now, read: { try core.lumenna.plan(date: $0).date }) { chosen = $0 })
                }
            }
        }
        .navigationTitle(plan.map { Clock.spokenDay($0.date) } ?? "Day")
        // One sheet for every question, and one for the block form: a third sheet on the
        // same view kept the form from showing.
        .sheet(item: $asker.asked) { asked in
            NavigationStack { asked.view }
        }
        .navigationDestination(item: $opened) { opened in
            TaskView(id: opened.id)
        }
        .sheet(item: $form) { model in
            BlockSheet(model: model)
        }
    }

    private var day: Plan? {
        _ = core.generation
        return core.read { try core.lumenna.plan(date: Clock.isoDay(chosen ?? .now)) }
    }

    /// The timeline as rows: blocks with their sittings under them, free time, now.
    private func rows(_ plan: Plan) -> [Row] {
        plan.timeline.flatMap { item -> [Row] in
            switch item {
            case let .block(number):
                guard let block = plan.blocks.first(where: { $0.row == number }) else { return [] }
                return [.block(block)] + block.assignments.map { .sitting($0, in: block) }
            case let .free(start, end, minutes, title, details, actions):
                return [.free(start: start, end: end, minutes: minutes, title: title, details: details, actions: actions)]
            case let .now(time, title): return [.now(time: time, title: title)]
            }
        }
    }

    @ViewBuilder
    private func rowView(_ row: Folding.Shown<Row>, in plan: Plan) -> some View {
        switch row.item {
        case let .block(block):
            // The time and title, then the details the core words for every app, then
            // whether its sittings are folded away.
            let text = ([("\(Clock.time(block.start)) to \(Clock.time(block.end)), \(block.title)")] + block.details + [row.state].compactMap { $0 })
                .joined(separator: ", ")
            button(text, heading: true) {
                let fold = Folding.action(for: row)
                asker.offer(
                    block.title, block.actions,
                    extra: fold.map { fold in [(fold.title, { toggle(row.item.key, saying: fold.said) })] } ?? [],
                    form: { _ in edit(block, on: plan.date) }
                )
            }
        case let .sitting(sitting, _):
            button(([sitting.title] + sitting.details).joined(separator: ", ")) {
                asker.offer(sitting.title, sitting.actions, form: { action in
                    if action.kind == .editTask { later { opened = TaskOpened(id: action.target) } }
                })
            }
            .padding(.leading, 8)
        case let .free(start, end, minutes, title, details, actions):
            // "<title>, <details>, <start> to <end>", the core's words in every app's order.
            button(([title] + details + ["\(Clock.time(start)) to \(Clock.time(end))"]).joined(separator: ", ")) {
                asker.offer(title, actions, form: { action in
                    addBlock(on: plan, at: action.other ?? start, minutes: minutes)
                })
            }
        case let .now(time, title):
            Text("\(title), \(Clock.time(time))").font(.headline)
        }
    }

    /// A row that offers its actions when tapped.
    private func button(_ text: String, heading: Bool = false, actions: @escaping () -> Void) -> some View {
        Button(action: actions) { Text(text) }
            .accessibilityAddTraits(heading ? .isHeader : [])
    }

    private func toggle(_ key: String, saying said: String) {
        folding.toggle(key)
        Announcer.say(said)
    }

    /// Does something once the sheet in front has closed, so the two do not collide.
    private func later(_ run: @escaping () -> Void) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4, execute: run)
    }

    private func step(_ days: Int, from plan: Plan?) {
        let from = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        let next = Calendar.current.date(byAdding: .day, value: days, to: from) ?? from
        chosen = Calendar.current.isDateInToday(next) ? nil : next
    }

    /// Asks "this day, or every day?" of a repeating block — never guessed.
    private func edit(_ block: PlanBlock, on date: String) {
        let series = {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                form = core.read { try BlockFormModel.series(core: core, id: block.series, saved: saved) }
            }
        }
        guard block.repeats else { return series() }
        asker.show(ChoicePrompt(title: "Change \(block.title)", message: "Which occurrences?", choices: [
            ("\(Clock.spokenDay(date)) Only", {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                    form = .occurrence(core: core, block: block, day: date, saved: saved)
                }
            }),
            ("Every Occurrence", series),
        ]))
    }

    private func addBlock(on plan: Plan?, at start: String? = nil, minutes: UInt32? = nil) {
        let day = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        let model = BlockFormModel(
            core: core, purpose: .add, start: start, minutes: Int(min(minutes ?? 60, 720)), day: day, saved: saved
        )
        if start == nil {
            form = model
        } else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { form = model }
        }
    }

    /// After a form saves: every view reads the store again, and what changed is said.
    private func saved(_ change: Change) {
        core.changed()
        Announcer.say(change.announcement, notices: change.notices)
    }
}

/// A day to go to, chosen as the phone chooses one: a day said or typed in the core's words
/// ("next friday", "12 October") from the system's input screen, or a date picker, then Go.
private struct DayChoice: View {
    @Environment(\.dismiss) private var dismiss
    @State var day: Date
    /// The core's reading of a day named: its ISO date.
    let read: (String) throws -> String
    let chosen: (Date?) -> Void
    @State private var typed = ""
    @State private var problem: String?
    private let question = TextQuestion.goToDay

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

    /// The day typed if there is one, else the picker's. A day the core cannot read stays
    /// here, saying why.
    private func goTo(_ text: String) {
        let named = text.trimmingCharacters(in: .whitespacesAndNewlines)
        do {
            let iso = try read(named.isEmpty ? Clock.isoDay(day) : named)
            guard let date = try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse(iso) else { return }
            dismiss()
            chosen(Calendar.current.isDateInToday(date) ? nil : date)
        } catch {
            problem = error.sentence
            Announcer.say(error.sentence)
        }
    }
}
