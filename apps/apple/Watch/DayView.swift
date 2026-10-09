import SwiftUI

/// The day as lived: its summary, then each block with the sittings in it, the free time
/// between, and where now falls, as the phone's day lists them. A sitting's timer is started,
/// paused and stopped from its row; the previous and next days are a button away.
struct DayView: View {
    @EnvironmentObject private var core: WatchCore
    /// The day shown, as an offset from today, so it follows midnight.
    @State private var offset = 0

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
                    Text("\(Clock.time(block.start)), \(block.title), cancelled for this day")
                        .foregroundStyle(.secondary)
                }
            }
            Section {
                Button("Previous Day") { offset -= 1 }
                if offset != 0 { Button("Today") { offset = 0 } }
                Button("Next Day") { offset += 1 }
            }
        }
        .navigationTitle(plan.map { Clock.spokenDay($0.date) } ?? "Day")
    }

    private var day: Plan? {
        _ = core.generation
        let date = Calendar.current.date(byAdding: .day, value: offset, to: .now) ?? .now
        return core.read { try core.lumenna.plan(date: Clock.isoDay(date)) }
    }

    @ViewBuilder
    private func timelineRow(_ item: PlanItem, in plan: Plan) -> some View {
        switch item {
        case let .block(row):
            if let block = plan.blocks.first(where: { $0.row == row }) {
                blockRows(block)
            }
        case let .free(start, end, minutes):
            Text("Free, \(Clock.length(minutes)), \(Clock.time(start)) to \(Clock.time(end))")
                .foregroundStyle(.secondary)
        case let .now(time):
            Text("Now, \(Clock.time(time))").font(.headline)
        }
    }

    @ViewBuilder
    private func blockRows(_ block: PlanBlock) -> some View {
        // The time and title, then the details the core words for every app.
        Text(([("\(Clock.time(block.start)) to \(Clock.time(block.end)), \(block.title)")] + block.details)
            .joined(separator: ", "))
            .accessibilityAddTraits(.isHeader)
        ForEach(block.assignments, id: \.id) { sitting in
            Text(([sitting.title] + sitting.details).joined(separator: ", "))
                .padding(.leading, 8)
                .swipeActions(edge: .leading) {
                    if sitting.running {
                        Button("Pause Timer") { core.act { try core.lumenna.pauseTimer(assignment: sitting.id) } }
                    } else {
                        Button(sitting.status == "paused" ? "Resume Timer" : "Start Timer") {
                            core.act { try core.lumenna.startTimer(assignment: sitting.id) }
                        }
                    }
                }
                .swipeActions(edge: .trailing) {
                    if sitting.running || sitting.status == "paused" {
                        Button("Stop Timer") {
                            core.act { try core.lumenna.stopTimer(assignment: sitting.id, minutes: nil) }
                        }
                    }
                    Button("Remove from Block", role: .destructive) {
                        core.act { try core.lumenna.unassign(assignment: sitting.id) }
                    }
                }
        }
    }
}
