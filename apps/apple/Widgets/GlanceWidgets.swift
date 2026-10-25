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
    var timerStart: Date? { glance.moment.timer.flatMap { GlanceWidgets.instant($0.countingFrom) } }

    /// The block under way, from its start to its end, for a gauge that fills by itself.
    var blockUnderWay: ClosedRange<Date>? {
        guard let block = glance.moment.current, let start = GlanceWidgets.instant(block.startsAt),
              let end = GlanceWidgets.instant(block.endsAt), start < end else { return nil }
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
        glance.compactMap { entry in instant(entry.moment.at).map { GlanceTimelineEntry(date: $0, glance: entry) } }
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
        "\(entry.markDone), \(task.title)"
    }

    /// What a size may show of `parts`, most first: all of them, then each shorter run of the
    /// first, for SwiftUI to take the first that fits. A smaller size shows the first of a
    /// widget's parts that fit, never a later part without an earlier one.
    static func fitting(_ parts: [LumennaApp.GlancePart]) -> [[LumennaApp.GlancePart]] {
        stride(from: parts.count, to: 0, by: -1).map { Array(parts.prefix($0)) }
    }

    /// An instant as the core writes one (RFC 3339, in UTC, to the nanosecond or to the second).
    static func instant(_ text: String) -> Date? {
        let whole = Date.ISO8601FormatStyle()
        guard let dot = text.firstIndex(of: "."), let zone = text[dot...].firstIndex(where: { !$0.isNumber && $0 != "." }) else {
            return try? whole.parse(text)
        }
        // Foundation reads at most milliseconds: the seconds whole, then their fraction.
        guard let seconds = try? whole.parse(String(text[..<dot] + text[zone...])),
              let fraction = Double("0" + text[dot ..< zone]) else { return nil }
        return seconds.addingTimeInterval(fraction)
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
