import SwiftUI

/// The day as lived: its summary, then each block with the sittings in it, the free time
/// between, where now falls, and what is cancelled for the day, as the phone's day lists
/// them. Tapping a row offers what the phone's swipe actions do (`DayAction`, shared with
/// it): a watch's row has room for few. A block with sittings folds, as on the phone. Going
/// to another day, and a new block, are buttons at the foot.
struct DayView: View {
    @EnvironmentObject private var core: WatchCore
    /// The day shown, or nil for today, which follows midnight.
    @State private var chosen: Date?
    @State private var asked: Asked?
    @State private var form: BlockFormModel?
    @State private var folding = Folding()

    /// A row of the day, as folding sees it.
    private enum Row {
        case block(PlanBlock)
        case sitting(PlanAssignment, in: PlanBlock)
        case free(start: String, end: String, minutes: UInt32)
        case now(String)

        var depth: Int {
            if case .sitting = self { return 1 }
            return 0
        }

        /// Only a block has anything under it.
        var key: String {
            switch self {
            case let .block(block): "block:\(block.id)"
            case let .sitting(sitting, _): "sitting:\(sitting.id)"
            case let .free(start, _, _): "free:\(start)"
            case .now: "now"
            }
        }
    }

    var body: some View {
        let plan = day
        let shown = plan.map { folding.shown(rows($0), depth: \.depth, key: \.key) } ?? []
        List {
            if let plan {
                Text(plan.summary.isEmpty ? plan.announcement : plan.summary)
                    .font(.footnote)
                ForEach(Array(shown.enumerated()), id: \.element.item.key) { _, row in
                    rowView(row, in: plan)
                }
                ForEach(plan.cancelled, id: \.series) { block in
                    button("\(Clock.time(block.start)), \(block.title), cancelled for this day") {
                        choices(block.title, DayAction.ofCancelled) { _ in
                            core.act { try core.lumenna.restoreOccurrence(id: block.series, date: plan.date) }
                        }
                    }
                }
            }
            Section {
                Button("Add Block") { addBlock(on: plan) }
                Button("Previous Day") { step(-1, from: plan) }
                if chosen != nil { Button("Today") { chosen = nil } }
                Button("Next Day") { step(1, from: plan) }
                Button("Go to Day") { asked = Asked(DayChoice(day: chosen ?? .now) { chosen = $0 }) }
            }
        }
        .navigationTitle(plan.map { Clock.spokenDay($0.date) } ?? "Day")
        .sheet(item: $asked) { asked in
            NavigationStack { asked.view }
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
            case let .free(start, end, minutes): return [.free(start: start, end: end, minutes: minutes)]
            case let .now(time): return [.now(time)]
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
                ask(ChoicePrompt(
                    title: block.title,
                    choices: DayAction.of(block).map { action in (action.title, { run(action, block: block, in: plan) }) }
                        + (fold.map { fold in [(fold.title, { toggle(row.item.key, saying: fold.said) })] } ?? [])
                ))
            }
        case let .sitting(sitting, _):
            button(([sitting.title] + sitting.details).joined(separator: ", ")) {
                choices(sitting.title, DayAction.of(sitting)) { run($0, sitting: sitting) }
            }
            .padding(.leading, 8)
        case let .free(start, end, minutes):
            button("Free, \(Clock.length(minutes)), \(Clock.time(start)) to \(Clock.time(end))") {
                choices("Free time", DayAction.ofFreeTime) { _ in addBlock(on: plan, at: start, minutes: minutes) }
            }
        case let .now(time):
            Text("Now, \(Clock.time(time))").font(.headline)
        }
    }

    /// A row that offers its actions when tapped.
    private func button(_ text: String, heading: Bool = false, actions: @escaping () -> Void) -> some View {
        Button(action: actions) { Text(text) }
            .accessibilityAddTraits(heading ? .isHeader : [])
    }

    private func choices(_ title: String, _ actions: [DayAction], run: @escaping (DayAction) -> Void) {
        ask(ChoicePrompt(title: title, choices: actions.map { action in (action.title, { run(action) }) }))
    }

    private func toggle(_ key: String, saying said: String) {
        folding.toggle(key)
        Announcer.say(said)
    }

    private func run(_ action: DayAction, block: PlanBlock, in plan: Plan) {
        let lumenna = core.lumenna
        switch action {
        case .assignTask: assign(to: block, on: plan.date)
        case .edit: edit(block, on: plan.date)
        case .cancelThisDay: core.act { try lumenna.cancelOccurrence(id: block.series, date: plan.date) }
        case .restoreThisDay: core.act { try lumenna.restoreOccurrence(id: block.series, date: plan.date) }
        case .deleteBlock:
            later(ChoicePrompt(title: "Delete \(block.title)?", message: DayAction.deleting(block), choices: [
                ("Delete", { core.act { try lumenna.deleteBlock(id: block.series) } }),
            ]))
        default: break
        }
    }

    private func run(_ action: DayAction, sitting: PlanAssignment) {
        let lumenna = core.lumenna
        switch action {
        case .startTimer, .resumeTimer: core.act { try lumenna.startTimer(assignment: sitting.id) }
        case .pauseTimer: core.act { try lumenna.pauseTimer(assignment: sitting.id) }
        case .stopTimer: core.act { try lumenna.stopTimer(assignment: sitting.id, minutes: nil) }
        case .plannedLength:
            later(LengthChoice(title: "Planned length of \(sitting.title)", without: "No Planned Length") { minutes in
                core.act { try lumenna.planMinutes(assignment: sitting.id, minutes: minutes) }
            })
        case .logMinutes:
            later(TextPrompt("Minutes on \(sitting.title)", message: DayAction.loggingMinutes, placeholder: "45", action: "Log") { text in
                guard let minutes = UInt32(text) else {
                    core.failure = "That is not a number of minutes."
                    return
                }
                core.act { try lumenna.stopTimer(assignment: sitting.id, minutes: minutes) }
            })
        case .unassign: core.act { try lumenna.unassign(assignment: sitting.id) }
        default: break
        }
    }

    private func ask(_ view: some View) {
        asked = Asked(view)
    }

    /// Asks something once the sheet in front has closed, so the two do not collide.
    private func later(_ view: some View) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { asked = Asked(view) }
    }

    private func step(_ days: Int, from plan: Plan?) {
        let from = plan.flatMap { try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse($0.date) } ?? .now
        let next = Calendar.current.date(byAdding: .day, value: days, to: from) ?? from
        chosen = Calendar.current.isDateInToday(next) ? nil : next
    }

    /// From the block, pick a task: the other way round from the task's Put in a Block.
    private func assign(to block: PlanBlock, on date: String) {
        let tasks = core.read { try core.lumenna.listTasks(query: "").rows } ?? []
        later(ChoicePrompt(title: "Assign to \(block.title)", choices: tasks.map { task in
            (task.title, {
                later(LengthChoice(title: "Planned length", without: "No Planned Length") { minutes in
                    core.act { try core.lumenna.assign(task: task.id, block: block.series, date: date, minutes: minutes) }
                })
            })
        }))
    }

    /// Asks "this day, or every day?" of a repeating block — never guessed.
    private func edit(_ block: PlanBlock, on date: String) {
        let series = {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                form = core.read { try BlockFormModel.series(core: core, id: block.series, saved: saved) }
            }
        }
        guard block.repeats else { return series() }
        later(ChoicePrompt(title: "Change \(block.title)", message: "Which occurrences?", choices: [
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
            core: core, purpose: .add, start: start ?? "09:00", minutes: Int(min(minutes ?? 60, 720)), day: day, saved: saved
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

/// A day to go to, chosen as the phone chooses one: a date picker, then Go.
private struct DayChoice: View {
    @Environment(\.dismiss) private var dismiss
    @State var day: Date
    let chosen: (Date?) -> Void

    var body: some View {
        List {
            DatePicker("Day", selection: $day, displayedComponents: .date)
            Button("Go") {
                dismiss()
                chosen(Calendar.current.isDateInToday(day) ? nil : day)
            }
        }
        .navigationTitle("Go to Day")
    }
}
