import SwiftUI

/// The day as lived: its summary, then each block with the sittings in it, the free time
/// between, where now falls, and what is cancelled for the day, as the phone's day lists
/// them. Tapping a row offers what the phone's swipe actions do: a watch's row has room for
/// few. The previous and next days, and a new block, are buttons at the foot.
struct DayView: View {
    @EnvironmentObject private var core: WatchCore
    /// The day shown, as an offset from today, so it follows midnight.
    @State private var offset = 0
    @State private var asked: Asked?
    @State private var form: BlockFormModel?

    var body: some View {
        let plan = day
        List {
            if let plan {
                Text(plan.summary.isEmpty ? plan.announcement : plan.summary)
                    .font(.footnote)
                ForEach(Array(plan.timeline.enumerated()), id: \.offset) { _, item in
                    timelineRow(item, in: plan)
                }
                ForEach(plan.cancelled, id: \.series) { block in
                    row("\(Clock.time(block.start)), \(block.title), cancelled for this day") {
                        ChoicePrompt(title: block.title, choices: [
                            ("Restore This Day", { core.act { try core.lumenna.restoreOccurrence(id: block.series, date: plan.date) } }),
                        ])
                    }
                }
            }
            Section {
                Button("Add Block") { addBlock(on: plan) }
                Button("Previous Day") { offset -= 1 }
                if offset != 0 { Button("Today") { offset = 0 } }
                Button("Next Day") { offset += 1 }
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
        let date = Calendar.current.date(byAdding: .day, value: offset, to: .now) ?? .now
        return core.read { try core.lumenna.plan(date: Clock.isoDay(date)) }
    }

    @ViewBuilder
    private func timelineRow(_ item: PlanItem, in plan: Plan) -> some View {
        switch item {
        case let .block(number):
            if let block = plan.blocks.first(where: { $0.row == number }) {
                // The time and title, then the details the core words for every app.
                row(([("\(Clock.time(block.start)) to \(Clock.time(block.end)), \(block.title)")] + block.details).joined(separator: ", "), heading: true) {
                    ChoicePrompt(title: block.title, choices: blockActions(block, in: plan))
                }
                ForEach(block.assignments, id: \.id) { sitting in
                    row(([sitting.title] + sitting.details).joined(separator: ", ")) {
                        ChoicePrompt(title: sitting.title, choices: sittingActions(sitting))
                    }
                    .padding(.leading, 8)
                }
            }
        case let .free(start, end, minutes):
            row("Free, \(Clock.length(minutes)), \(Clock.time(start)) to \(Clock.time(end))") {
                ChoicePrompt(title: "Free time", choices: [
                    ("Add Block Here", { addBlock(on: plan, at: start, minutes: minutes) }),
                ])
            }
        case let .now(time):
            Text("Now, \(Clock.time(time))").font(.headline)
        }
    }

    /// A row that offers its actions when tapped.
    private func row(_ text: String, heading: Bool = false, actions: @escaping () -> some View) -> some View {
        Button {
            asked = Asked(actions())
        } label: {
            Text(text)
        }
        .accessibilityAddTraits(heading ? .isHeader : [])
    }

    private func blockActions(_ block: PlanBlock, in plan: Plan) -> [(String, () -> Void)] {
        var actions: [(String, () -> Void)] = []
        if block.acceptsTasks {
            actions.append(("Assign Task", { assign(to: block, on: plan.date) }))
        }
        actions.append(("Edit", { edit(block, on: plan.date) }))
        if block.repeats {
            actions.append(("Cancel This Day", { core.act { try core.lumenna.cancelOccurrence(id: block.series, date: plan.date) } }))
        }
        if block.changedForThisDay {
            actions.append(("Restore This Day", { core.act { try core.lumenna.restoreOccurrence(id: block.series, date: plan.date) } }))
        }
        actions.append(("Delete Block", {
            let message = block.repeats
                ? "Every occurrence goes, not only this day. To skip one day, cancel it instead."
                : "It goes to the trash with its assignments."
            ask(ChoicePrompt(title: "Delete \(block.title)?", message: message, choices: [
                ("Delete", { core.act { try core.lumenna.deleteBlock(id: block.series) } }),
            ]))
        }))
        return actions
    }

    private func sittingActions(_ sitting: PlanAssignment) -> [(String, () -> Void)] {
        // Start, pause and stop: a paused sitting is still in progress, and stopping either a
        // running or a paused one ends it.
        var actions: [(String, () -> Void)] = []
        if sitting.running {
            actions.append(("Pause Timer", { core.act { try core.lumenna.pauseTimer(assignment: sitting.id) } }))
        } else {
            actions.append((sitting.status == "paused" ? "Resume Timer" : "Start Timer", {
                core.act { try core.lumenna.startTimer(assignment: sitting.id) }
            }))
        }
        if sitting.running || sitting.status == "paused" {
            actions.append(("Stop Timer", { core.act { try core.lumenna.stopTimer(assignment: sitting.id, minutes: nil) } }))
        }
        actions.append(("Planned Length", {
            ask(LengthChoice(title: "Planned length of \(sitting.title)", without: "No Planned Length") { minutes in
                core.act { try core.lumenna.planMinutes(assignment: sitting.id, minutes: minutes) }
            })
        }))
        actions.append(("Log Minutes", {
            ask(TextPrompt("Minutes on \(sitting.title)", message: "The whole of this sitting, replacing what is logged.", placeholder: "45", action: "Log") { text in
                guard let minutes = UInt32(text) else {
                    core.failure = "That is not a number of minutes."
                    return
                }
                core.act { try core.lumenna.stopTimer(assignment: sitting.id, minutes: minutes) }
            })
        }))
        actions.append(("Unassign", { core.act { try core.lumenna.unassign(assignment: sitting.id) } }))
        return actions
    }

    /// Asks something after the sheet in front has closed, so the two do not collide.
    private func ask(_ view: some View) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { asked = Asked(view) }
    }

    /// From the block, pick a task: the other way round from the task's Put in a Block.
    private func assign(to block: PlanBlock, on date: String) {
        let tasks = core.read { try core.lumenna.listTasks(query: "").rows } ?? []
        ask(ChoicePrompt(title: "Assign to \(block.title)", choices: tasks.map { task in
            (task.title, {
                ask(LengthChoice(title: "Planned length", without: "No Planned Length") { minutes in
                    core.act { try core.lumenna.assign(task: task.id, block: block.series, date: date, minutes: minutes) }
                })
            })
        }))
    }

    /// Asks "this day, or every day?" of a repeating block — never guessed.
    private func edit(_ block: PlanBlock, on date: String) {
        let series = {
            form = core.read { try BlockFormModel.series(core: core, id: block.series, saved: saved) }
        }
        guard block.repeats else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4, execute: series)
            return
        }
        ask(ChoicePrompt(title: "Change \(block.title)", message: "Which occurrences?", choices: [
            ("\(Clock.spokenDay(date)) Only", {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                    form = .occurrence(core: core, block: block, day: date, saved: saved)
                }
            }),
            ("Every Occurrence", { DispatchQueue.main.asyncAfter(deadline: .now() + 0.4, execute: series) }),
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
