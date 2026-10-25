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
    #if os(macOS) && !APP_STORE && !LUMENNA_WIDGETS
    /// The timer's control, written beside the glance (`TimerControlFile`).
    private var control: TimerControlFile?
    #endif

    #if !LUMENNA_WIDGETS
    /// The glance of `app`, written into the App Group's container; `nil` where the build has
    /// none (an unsigned build, a UI test's).
    convenience init?(app: AppModel) {
        guard let container = AppGroup.container else { return nil }
        self.init(directory: container) { try app.glance(hours: GlanceFile.hours) }
        #if os(macOS) && !APP_STORE
        let device = app.device
        control = TimerControlFile(directory: container) { try device.timerControl() }
        #endif
    }
    #endif

    /// The glance of `device`, worded in this device's clock, written into the App Group's
    /// container: a widget's own, after a Mark Done it ran on the store, so the timeline it is
    /// about to read again holds the change whether or not the app is running.
    convenience init?(device: LumennaApp.Device) {
        guard let container = AppGroup.container else { return nil }
        self.init(directory: container) { try device.glance(hours: GlanceFile.hours, locale: Writing()) }
    }

    /// `glance`'s entries, written into `directory`, which tests name.
    init(directory: URL, glance: @escaping () throws -> [LumennaApp.GlanceEntry]) {
        url = directory.appendingPathComponent(GlanceFile.name)
        self.glance = glance
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    #if !LUMENNA_WIDGETS
    /// Writes it again whenever the store changes.
    func follow() {
        observer = NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.soon()
        }
    }
    #endif

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
                    #if os(macOS) && !APP_STORE && !LUMENNA_WIDGETS
                    // The timer's control, which reaches no store in this edition, with it.
                    self.control?.write()
                    #endif
                    GlanceFile.reload()
                } catch {
                    // Nothing to tell the person: a widget goes on showing the last one written.
                }
            }
            if let done { DispatchQueue.main.async(execute: done) }
        }
    }

    #if os(macOS) && !APP_STORE && !LUMENNA_WIDGETS
    /// The minute the timer's control was last written in.
    private var minute: Int?

    /// Writes the timer's control again once a minute, off the main thread: what a press would
    /// start moves with the clock, with no change to the store.
    func tick() {
        let now = Int(Date().timeIntervalSince1970 / 60)
        guard now != minute, let control else { return }
        minute = now
        queue.async { control.write() }
    }
    #endif

    /// Asks every widget and control to read again: what they show is the store's, so any change
    /// may have moved it.
    static func reload() {
        #if canImport(WidgetKit)
        WidgetCenter.shared.reloadAllTimelines()
        if #available(iOS 18.0, macOS 26.0, watchOS 26.0, *) {
            ControlCenter.shared.reloadAllControls()
        }
        #endif
    }

    /// Writes it on this thread: replaced whole, so a widget never reads half of one.
    func writeNow() throws {
        let text = LumennaApp.glanceFile(entries: try glance())
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: url, options: .atomic)
    }
}

/// The timer's control as the core has it (`Device.timerControl`, `timerControlFile`), written
/// into the group for the one control that cannot reach its store: the Mac's direct download's,
/// sandboxed away from `~/Library/Application Support`. Its app writes it with the glance, after
/// every change, and on the minute, since what a press would start moves with the clock; a write
/// that changes nothing tells no control. Elsewhere a control reads the store itself.
final class TimerControlFile {
    /// The file's name in the group's container.
    static let name = "timer-control.json"

    let url: URL
    private let control: () throws -> LumennaApp.TimerControl
    /// What was written last, so an unchanged control is not told to read again.
    private var last: String?

    /// `control`'s, written into `directory`.
    init(directory: URL, control: @escaping () throws -> LumennaApp.TimerControl) {
        url = directory.appendingPathComponent(TimerControlFile.name)
        self.control = control
    }

    /// Writes it on this thread, replaced whole; returns whether it changed.
    @discardableResult
    func writeNow() throws -> Bool {
        let text = LumennaApp.timerControlFile(control: try control())
        guard text != last else { return false }
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(text.utf8).write(to: url, options: .atomic)
        last = text
        return true
    }

    /// Writes it, and tells the controls if it changed.
    func write() {
        guard (try? writeNow()) == true else { return }
        #if canImport(WidgetKit)
        if #available(macOS 26.0, iOS 18.0, watchOS 26.0, *) {
            ControlCenter.shared.reloadControls(ofKind: "timer")
        }
        #endif
    }

    /// The control as the app last wrote it into the group, for the control to show.
    static func read() -> LumennaApp.TimerControl? {
        guard let url = AppGroup.container?.appendingPathComponent(name),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        return try? LumennaApp.timerControlFromFile(contents: text)
    }
}
