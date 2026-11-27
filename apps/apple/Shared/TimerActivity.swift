#if os(iOS)
import ActivityKit
import AppIntents
import Foundation
import LumennaApp

// The running timer as a Live Activity, on the iPhone's Lock Screen and in its Dynamic Island,
// and from there in a paired watch's Smart Stack: what the core's `TimerControl.timer` says runs,
// its buttons the core's Pause Timer and Stop Timer. Compiled into the app, which starts, updates
// and ends it (`TimerActivities`), and into the iPhone's widget extension, which draws it
// (`TimerLiveActivity`). Nothing here words or decides what it shows.

/// What a Live Activity of the timer holds: nothing fixed, since the sitting it runs on can
/// change while it shows, and its state.
struct TimerActivityAttributes: ActivityAttributes {
    /// The timer as the core's control had it when the activity was last written.
    struct ContentState: Codable, Hashable {
        /// The sitting whose timer runs, which Pause and Stop act on.
        var sitting: String
        /// The task, and the block it is in (empty in free time), the core's titles.
        var task: String
        var block: String
        /// What counting up from gives everything logged.
        var countingFrom: Date
        /// When the sitting's planned length runs out, while some of it was left: what it counts
        /// down to.
        var runsOutAt: Date?
        /// The sitting's planned length, in minutes.
        var plannedMins: UInt32?
        /// The core's countdown at the moment it runs out ("time is up"), shown once it has: the
        /// one moment a Live Activity's text cannot count to by itself, so it is written ahead.
        var upWords: String?
        /// Its buttons, the core's action names.
        var pause: String
        var stop: String
    }
}

enum TimerActivityState {
    /// The Live Activity's state for `control`, the core's, at `now`; none while no timer runs.
    static func of(_ control: LumennaApp.TimerControl, at now: Date = Date()) -> TimerActivityAttributes.ContentState? {
        guard control.running, let timer = control.timer, let from = Instant.parse(timer.countingFrom) else { return nil }
        let out = timer.runsOutAt.flatMap(Instant.parse)
        return TimerActivityAttributes.ContentState(
            sitting: timer.sitting,
            task: timer.task,
            block: timer.block,
            countingFrom: from,
            runsOutAt: out,
            plannedMins: timer.plannedMins,
            upWords: out.flatMap { LumennaApp.glanceCountdown(timer: timer, now: Instant.rfc3339($0)) },
            pause: Wording.pauseTimer,
            stop: Wording.stopTimer
        )
    }

    /// Until when it counts down, and whether it still does at `now`: past its planned length it
    /// counts up from `countingFrom`, all that was logged, with the core's words that time is up.
    static func countsDown(_ state: TimerActivityAttributes.ContentState, at now: Date) -> ClosedRange<Date>? {
        guard let out = state.runsOutAt, out > now, state.countingFrom < out else { return nil }
        return state.countingFrom ... out
    }
}

/// The Live Activity's Pause Timer: the core's, on the sitting it shows (`Device.pauseTimer`).
/// Run in the app's process, which then writes the activity again.
struct PauseTimerActivityIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "Pause Timer"
    /// The Live Activity's alone: Shortcuts and Siri offer Pause a Timer.
    static let isDiscoverable = false

    @Parameter(title: "Sitting")
    var sitting: String

    init() {}

    init(sitting: String) {
        self.sitting = sitting
    }

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult {
        try store.timer { try $0.pauseTimer(assignment: sitting) }
        await TimerActivityIntents.changed()
        return .result()
    }
}

/// The Live Activity's Stop Timer: the core's, logging the timer's time (`Device.stopTimer`).
struct StopTimerActivityIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "Stop Timer"
    /// The Live Activity's alone: Shortcuts and Siri offer Stop a Timer.
    static let isDiscoverable = false

    @Parameter(title: "Sitting")
    var sitting: String

    init() {}

    init(sitting: String) {
        self.sitting = sitting
    }

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult {
        try store.timer { try $0.stopTimer(assignment: sitting, minutes: nil) }
        await TimerActivityIntents.changed()
        return .result()
    }
}

enum TimerActivityIntents {
    /// After a button's write, in the app: the store changed (the reminders re-armed, the
    /// glance written, the watch told) and the Live Activity written again at once, since the
    /// app may be in the background, where its look at the store's heads waits.
    static func changed() async {
        #if !LUMENNA_WIDGETS
        await MainActor.run {
            NotificationCenter.default.post(name: Core.changed, object: nil)
            TimerActivities.shared?.refresh()
        }
        #endif
    }
}
#endif
