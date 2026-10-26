import AppIntents
import Foundation
import LumennaApp
import SwiftUI
import WidgetKit

/// Lumenna's widgets, the core's (`glanceWidgets()`): Now and Today, each named and described in
/// the gallery in the core's words, read at run time, and filled from the glance the app writes
/// into the App Group (`GlanceFile`), which the core reads back (`glanceFromFile`).
@main
struct LumennaWidgets: WidgetBundle {
    init() {
        // The core asks for the zone through `TZ`, which the sandbox is no place to leave unset.
        setenv("TZ", TimeZone.current.identifier, 1)
        #if !os(watchOS)
        WidgetStore.serve()
        #endif
    }

    var body: some Widget {
        NowWidget()
        #if os(watchOS)
        NewTaskComplication()
        #else
        TodayWidget()
        #endif
        // Control Center's, the core's (`controls()`).
        if #available(iOS 18.0, macOS 26.0, watchOS 26.0, *) {
            TimerControlWidget()
        }
        if #available(iOS 18.0, macOS 26.0, watchOS 26.0, *) {
            NewTaskControlWidget()
        }
    }
}

#if os(watchOS)
/// New Task on a watch face: the core's launch shortcut (`LaunchComplication`), tapped to open quick
/// add. It shows no glance, so it has a timeline of one entry, read again never.
struct NewTaskComplication: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: LaunchComplication.kind, provider: LaunchProvider()) { _ in
            LaunchComplicationView()
        }
        .configurationDisplayName(LaunchComplication.name)
        .description(LaunchComplication.description)
        .supportedFamilies(LaunchComplication.families)
    }
}

struct LaunchEntry: TimelineEntry {
    let date: Date
}

struct LaunchProvider: TimelineProvider {
    func placeholder(in context: Context) -> LaunchEntry { LaunchEntry(date: Date()) }

    func getSnapshot(in context: Context, completion: @escaping (LaunchEntry) -> Void) {
        completion(LaunchEntry(date: Date()))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<LaunchEntry>) -> Void) {
        completion(Timeline(entries: [LaunchEntry(date: Date())], policy: .never))
    }
}

/// A plus, and the core's name where there is room for it.
struct LaunchComplicationView: View {
    @Environment(\.widgetFamily) private var family

    var body: some View {
        Group {
            switch family {
            case .accessoryInline:
                Label(LaunchComplication.name, systemImage: LaunchComplication.symbol)
            case .accessoryCorner:
                Image(systemName: LaunchComplication.symbol)
                    .font(.title2.weight(.semibold))
                    .widgetLabel(LaunchComplication.name)
            default:
                ZStack {
                    AccessoryWidgetBackground()
                    Image(systemName: LaunchComplication.symbol)
                        .font(.title2.weight(.semibold))
                }
            }
        }
        .widgetAccentable()
        .containerBackground(.clear, for: .widget)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(LaunchComplication.name)
        .accessibilityAddTraits(.isButton)
        .widgetURL(LaunchURL.of(LaunchComplication.launch))
    }
}
#endif

struct NowWidget: Widget {
    static let kind = "now"

    var body: some WidgetConfiguration {
        GlanceConfiguration.of(Self.kind)
    }
}

struct TodayWidget: Widget {
    static let kind = "today"

    var body: some WidgetConfiguration {
        GlanceConfiguration.of(Self.kind)
    }
}

enum GlanceConfiguration {
    /// The core's widget `kind`, in the sizes it comes in here.
    static func of(_ kind: String) -> some WidgetConfiguration {
        let widget = GlanceWidgets.widget(kind)
        return StaticConfiguration(kind: kind, provider: GlanceProvider()) { entry in
            GlanceView(entry: entry, parts: widget?.parts ?? [.headline])
        }
        .configurationDisplayName(widget?.name ?? kind)
        .description(widget?.description ?? "")
        .supportedFamilies(GlanceWidgets.families[kind] ?? [])
    }
}

/// The glance's timeline, as the app last wrote it: a moment each time what it shows changes,
/// read again once the last has come (`.atEnd`), and whenever the app reloads the widgets.
struct GlanceProvider: TimelineProvider {
    func placeholder(in context: Context) -> GlanceTimelineEntry {
        GlanceSource.nothing()
    }

    func getSnapshot(in context: Context, completion: @escaping (GlanceTimelineEntry) -> Void) {
        completion(GlanceSource.now())
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<GlanceTimelineEntry>) -> Void) {
        let (entries, fresh) = GlanceSource.timeline()
        // A glance run out is asked for again a little later, not the moment it is drawn: the app
        // or the store may have more by then.
        completion(Timeline(entries: entries, policy: fresh ? .atEnd : .after(Date().addingTimeInterval(15 * 60))))
    }
}

enum GlanceSource {
    /// The glance file's moments, and whether they still reach past now. Run out or never
    /// written, the glance is worked out afresh from the store where the widget can reach it (the
    /// iPhone and iPad, the App Store edition), opened for that alone. Elsewhere (the direct
    /// download, whose store is outside the sandbox, and the watch) the last moment is shown as
    /// now, and with none, the core's glance of a day with nothing in it.
    static func timeline() -> (entries: [GlanceTimelineEntry], fresh: Bool) {
        let now = Date()
        let written = GlanceWidgets.entries(read())
        if !GlanceWidgets.stale(written, at: now) { return (written, true) }
        #if !os(watchOS) && !(os(macOS) && !APP_STORE)
        if let worked = try? GlanceStore.visit(AppGroup.store, { try $0.glance(hours: GlanceFile.hours, locale: Writing()) }) {
            // Kept for the next reading, as the app would have written it.
            if let container = AppGroup.container { try? GlanceFile(directory: container) { worked }.writeNow() }
            let entries = GlanceWidgets.entries(worked)
            if !entries.isEmpty { return (entries, true) }
        }
        #endif
        let last = written.last?.glance ?? LumennaApp.glanceEmpty(locale: Writing())
        return ([GlanceTimelineEntry(date: now, glance: last)], false)
    }

    /// The moment true now, the core's (`glanceCurrent`: the last whose moment has come, else
    /// the first).
    static func now() -> GlanceTimelineEntry {
        let entries = timeline().entries
        let now = Date()
        guard let current = LumennaApp.glanceCurrent(entries: entries.map(\.glance), now: Instant.rfc3339(now)) else { return entries[0] }
        return entries.first { $0.glance == current } ?? GlanceTimelineEntry(date: now, glance: current)
    }

    /// The core's words for a day with nothing in it, now.
    static func nothing() -> GlanceTimelineEntry {
        GlanceTimelineEntry(date: Date(), glance: LumennaApp.glanceEmpty(locale: Writing()))
    }

    private static func read() -> [LumennaApp.GlanceEntry] {
        guard let url = AppGroup.container?.appendingPathComponent(GlanceFile.name),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        return (try? LumennaApp.glanceFromFile(contents: text)) ?? []
    }
}

#if !os(watchOS) && !(os(macOS) && !APP_STORE)
/// The store, for Today's Mark Done, where the widget can reach it: in the App Group, on the
/// iPhone and iPad and in the Mac App Store edition. Opened for each operation and closed before
/// it returns (`GlanceStore`), never held by the process.
enum WidgetStore {
    static func serve() {
        let store = AssistantStore(opening: { try GlanceStore.open(AppGroup.store) }) { device in
            // What the widget reads next, written while the store is still open: the app may not
            // be running to write it.
            try? GlanceFile(device: device)?.writeNow()
            // Done, a task's timer stops: the Timer's control reads again.
            if #available(iOS 18.0, macOS 26.0, *) { ControlCenter.shared.reloadAllControls() }
        }
        AppDependencyManager.shared.add(dependency: store)
    }
}
#elseif os(macOS)
/// The direct download's widget reaches no store: its Mark Done hands the task to the app
/// (`WidgetRequests`). The intents it compiles still name a store, so they are given one that
/// opens nothing, and any use of it is a refusal, never a missing dependency.
enum WidgetStore {
    static func serve() {
        let store = AssistantStore(opening: { throw CocoaError(.fileNoSuchFile) }) { _ in }
        AppDependencyManager.shared.add(dependency: store)
    }
}
#endif
