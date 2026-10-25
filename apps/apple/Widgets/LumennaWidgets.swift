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
        completion(Timeline(entries: GlanceSource.entries(), policy: .atEnd))
    }
}

enum GlanceSource {
    /// Every moment in the glance file, or, with none written yet (the app not yet opened, a
    /// build without its group), the core's glance of a day with nothing in it.
    static func entries() -> [GlanceTimelineEntry] {
        let entries = GlanceWidgets.entries(read())
        return entries.isEmpty ? [nothing()] : entries
    }

    /// The moment true now.
    static func now() -> GlanceTimelineEntry {
        let entries = entries()
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
/// iPhone and iPad and in the Mac App Store edition. Opened only when an intent first asks, and
/// only if the app has made it: a widget must never make a store in the group, where one found
/// first would keep the app's from moving there (`AppGroup.profile`).
enum WidgetStore {
    static func serve() {
        AppDependencyManager.shared.add { () async throws -> AssistantStore in
            guard let store = AppGroup.store,
                  FileManager.default.fileExists(atPath: store.appendingPathComponent("lumenna.sqlite").path) else {
                throw CocoaError(.fileNoSuchFile)
            }
            let device = try LumennaApp.Device(directory: store.path)
            let glance = GlanceFile(device: device)
            // What the widget reads next: the app may not be running to write it.
            return AssistantStore(device: device) { try? glance?.writeNow() }
        }
    }
}
#endif
