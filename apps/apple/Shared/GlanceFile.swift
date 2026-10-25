import Foundation
import LumennaApp
#if canImport(WidgetKit)
import WidgetKit
#endif

/// What a widget, complication or Smart Stack shows, written where the app's extensions can read
/// it: the core's worded glance (`App.glance`) for the next two days, as the core's own file
/// (`glanceFile`, which `glanceFromFile` reads back), in the App Group container. A widget runs
/// apart from the app and, in the Mac's direct download, cannot reach the store at all, so this
/// is what it reads in every edition.
///
/// Written at launch, whenever the store changes (`Core.changed`, and on the watch its own
/// writes), and in the iPhone's background refresh, which carries the two days forward while the
/// app goes unopened; then every widget is asked to read it again.
final class GlanceFile {
    /// The file's name in the group's container.
    static let name = "glance.json"
    /// How far ahead it reaches: as far as the core gives.
    static let hours: UInt32 = 48

    /// Where it is written.
    let url: URL
    private let glance: () throws -> [LumennaApp.GlanceEntry]
    private let queue = DispatchQueue(label: "dev.emassey0135.lumenna.glance", qos: .utility)
    private var observer: NSObjectProtocol?
    private var queued = false

    /// The glance of `app`, written into the App Group's container; `nil` where the build has
    /// none (an unsigned build, a UI test's).
    convenience init?(app: AppModel) {
        guard let container = AppGroup.container else { return nil }
        self.init(directory: container) { try app.glance(hours: GlanceFile.hours) }
    }

    /// `glance`'s entries, written into `directory`, which tests name.
    init(directory: URL, glance: @escaping () throws -> [LumennaApp.GlanceEntry]) {
        url = directory.appendingPathComponent(GlanceFile.name)
        self.glance = glance
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Writes it again whenever the store changes.
    func follow() {
        observer = NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.soon()
        }
    }

    /// Writes it a moment from now, once for any number of changes in between: a sync brings in
    /// many at once.
    func soon() {
        guard !queued else { return }
        queued = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            self?.queued = false
            self?.write()
        }
    }

    /// Writes it now, off the main thread, then asks the widgets to read it; `done` is called on
    /// the main thread once it is written, or could not be.
    func write(done: (() -> Void)? = nil) {
        queue.async { [weak self] in
            if let self {
                do {
                    try self.writeNow()
                    #if canImport(WidgetKit)
                    WidgetCenter.shared.reloadAllTimelines()
                    #endif
                } catch {
                    // Nothing to tell the person: a widget goes on showing the last one written.
                }
            }
            if let done { DispatchQueue.main.async(execute: done) }
        }
    }

    /// Writes it on this thread: replaced whole, so a widget never reads half of one.
    func writeNow() throws {
        let text = LumennaApp.glanceFile(entries: try glance())
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: url, options: .atomic)
    }
}
