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
        #if !os(watchOS) && !(os(macOS) && !APP_STORE)
        WidgetStore.serve()
        #endif
    }

    var body: some Widget {
        NowWidget()
        #if !os(watchOS)
        TodayWidget()
        #endif
    }
}

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

    /// The moment true now.
    static func now() -> GlanceTimelineEntry {
        let entries = timeline().entries
        return entries.last { $0.date <= Date() } ?? entries[0]
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
        }
        AppDependencyManager.shared.add(dependency: store)
    }
}
#endif
