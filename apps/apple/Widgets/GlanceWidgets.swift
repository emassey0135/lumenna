import Foundation
import LumennaApp
import WidgetKit

// What the widgets take from the core's glance, apart from how they are drawn, so the tests
// (`LumennaTests`, which compile this file) check it: which widgets there are and where they
// go, each moment as WidgetKit's entry, what a moment's Mark Done is called, and which of a
// widget's parts a smaller size shows. Nothing here words or decides what the glance says.

/// One moment of the glance, as WidgetKit takes it: from its `at` until the next.
struct GlanceTimelineEntry: TimelineEntry {
    let date: Date
    let glance: LumennaApp.GlanceEntry

    /// When the timer running counts from, which SwiftUI's timer text counts on by itself: a
    /// timer needs no moment of its own.
    var timerStart: Date? { glance.moment.timer.flatMap { Instant.parse($0.countingFrom) } }

    /// What the timer's text counts on by itself: down to the end of the sitting's planned
    /// length where one is set (and on past it, the time over), else up from `timerStart`.
    var timerCounts: Date? { glance.moment.timer?.runsOutAt.flatMap(Instant.parse) ?? timerStart }

    /// The core's words for what is left of the sitting's planned length at this entry's moment
    /// ("13 minutes left", "time is up", "3 minutes over"), shown beside the task; none for a
    /// timer with no planned length. A moment of its own each minute (`GlanceWidgets.counted`).
    var countdown: String? {
        glance.moment.timer.flatMap { LumennaApp.glanceCountdown(timer: $0, now: Instant.rfc3339(date)) }
    }

    /// The block under way, from its start to its end, for a gauge that fills by itself.
    var blockUnderWay: ClosedRange<Date>? {
        guard let block = glance.moment.current, let start = Instant.parse(block.startsAt),
              let end = Instant.parse(block.endsAt), start < end else { return nil }
        return start ... end
    }

    /// The Smart Stack's: a moment is worth showing while something is under way.
    var relevance: TimelineEntryRelevance? {
        TimelineEntryRelevance(score: GlanceWidgets.relevant(glance.moment) ? 1 : 0)
    }
}

enum GlanceWidgets {
    /// Each of the core's widgets (`glanceWidgets()`) by its id, which is its WidgetKit kind,
    /// and the sizes it comes in here. A widget the core adds has none until it is given them,
    /// which `WidgetTests` finds.
    static let families: [String: [WidgetFamily]] = {
        #if os(iOS)
        // The Lock Screen's too; StandBy shows the small one.
        ["now": [.systemSmall, .accessoryCircular, .accessoryRectangular, .accessoryInline], "today": [.systemMedium, .systemLarge]]
        #elseif os(watchOS)
        // A watch face's complications and the Smart Stack's: a watch has no room for the tasks.
        ["now": [.accessoryCircular, .accessoryRectangular, .accessoryInline, .accessoryCorner], "today": []]
        #else
        ["now": [.systemSmall], "today": [.systemMedium, .systemLarge]]
        #endif
    }()

    /// The core's widget `id`: its name and description in the gallery, and its parts.
    static func widget(_ id: String) -> LumennaApp.GlanceWidget? {
        LumennaApp.glanceWidgets().first { $0.id == id }
    }

    /// The glance's moments as WidgetKit's entries, each dated by its `at`, in order; one whose
    /// moment cannot be read is left out.
    static func entries(_ glance: [LumennaApp.GlanceEntry]) -> [GlanceTimelineEntry] {
        glance.compactMap { entry in Instant.parse(entry.moment.at).map { GlanceTimelineEntry(date: $0, glance: entry) } }
    }

    /// At most this many moments added for a countdown's minutes: three hours of them, past which
    /// the timeline is read again at its end, as any.
    static let countedMost = 180

    /// `entries` with a moment of their own wherever the core says a countdown's words change
    /// (`glanceRedrawAt`: the minute before it runs out, and so on) from `now`, each the moment
    /// it falls in again: WidgetKit takes every moment at once, so words that change each minute
    /// need one each, where the timer's own text counts by itself. A glance with no planned
    /// length running is as it was.
    static func counted(_ entries: [GlanceTimelineEntry], from now: Date, most: Int = countedMost) -> [GlanceTimelineEntry] {
        let glance = entries.map(\.glance)
        var out: [GlanceTimelineEntry] = []
        var added = 0
        for (index, entry) in entries.enumerated() {
            out.append(entry)
            guard entry.glance.moment.timer?.plannedMins != nil else { continue }
            let end = index + 1 < entries.count ? entries[index + 1].date : entry.date.addingTimeInterval(TimeInterval(countedMost * 60))
            var at = max(entry.date, now)
            while added < most, let next = LumennaApp.glanceRedrawAt(entries: glance, now: Instant.rfc3339(at)).flatMap(Instant.parse),
                  next > at, next < end {
                out.append(GlanceTimelineEntry(date: next, glance: entry.glance))
                added += 1
                at = next
            }
        }
        return out
    }

    /// Whether the glance has run out by `now`: none written, or its last moment come. The app
    /// writes two days ahead, so it has gone unopened for about that long.
    static func stale(_ entries: [GlanceTimelineEntry], at now: Date) -> Bool {
        guard let last = entries.last else { return true }
        return last.date <= now
    }

    /// Whether something is under way: a block, or a timer running.
    static func relevant(_ moment: LumennaApp.GlanceMoment) -> Bool {
        moment.current != nil || moment.timer != nil
    }

    /// What a task's Mark Done is called to VoiceOver: the core's action, then the task, as the
    /// Android widget names its button, so each says which task it marks.
    static func markDone(_ entry: LumennaApp.GlanceEntry, _ task: LumennaApp.Line) -> String {
        LumennaApp.markDoneDescribed(markDone: entry.markDone, task: task.title)
    }

    /// Today's New Task (`GlancePart.newTask`): the core's launch shortcut it opens, whose title
    /// names it; `nil` should the core drop it, which `WidgetTests` finds.
    static var newTask: LumennaApp.LaunchShortcut? {
        LumennaApp.launchShortcuts().first { $0.id == "new-task" }
    }

    /// What a size may show of `parts`, most first: all of them, then each shorter run of the
    /// first, for SwiftUI to take the first that fits. A smaller size shows the first of a
    /// widget's parts that fit, never a later part without an earlier one.
    static func fitting(_ parts: [LumennaApp.GlancePart]) -> [[LumennaApp.GlancePart]] {
        stride(from: parts.count, to: 0, by: -1).map { Array(parts.prefix($0)) }
    }

}

/// The store, as a widget may touch it: opened afresh for one piece of work and let go before it
/// returns, never held while the widget is suspended (iOS ends a process that is suspended with a
/// lock on a file in a group, `0xdead10cc`), and never made: one a widget made in the group would
/// keep the app's from moving there (`AppGroup.profile`).
enum GlanceStore {
    /// The store's file in its directory, as the core names it.
    static let file = "lumenna.sqlite"

    /// The store in `directory`, opened afresh, if the app has made one there.
    static func open(_ directory: URL?) throws -> LumennaApp.Device {
        guard let directory, FileManager.default.fileExists(atPath: directory.appendingPathComponent(file).path) else {
            throw CocoaError(.fileNoSuchFile)
        }
        return try LumennaApp.Device(directory: directory.path)
    }

    /// `body` on the store in `directory`, closed again once it returns (nothing else holds the
    /// device, and `body` may keep nothing of it); `nil` where there is no store.
    static func visit<T>(_ directory: URL?, _ body: (LumennaApp.Device) throws -> T) throws -> T? {
        guard let directory, FileManager.default.fileExists(atPath: directory.appendingPathComponent(file).path) else { return nil }
        return try body(try open(directory))
    }
}
