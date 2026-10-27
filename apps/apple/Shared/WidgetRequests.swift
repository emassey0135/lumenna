import Foundation
import LumennaApp

/// Today's Mark Done and the Timer control's press in the Mac's direct download, handed from its
/// widget extension to the app through the App Group's container: the extension, sandboxed, cannot
/// reach that edition's store (`~/Library/Application Support/lumenna`), and the app, resident in
/// the menu bar, holds it.
///
/// The widget writes a request into `requests/`, one file each: the assistant operation's id and
/// its arguments, as text. The app takes them in at launch, on its one-second tick and when the
/// folder changes, runs each through the core's dispatcher (`Device.assist`), and removes it once
/// it ran. Only what `allowed` names is run, whatever a file names, so nothing that can write the
/// folder can run another operation, an erasure among them, and each is given only the arguments
/// `allowed` lists; a request naming another operation, or one that does not read, is removed
/// unrun. A request whose run fails is kept and tried again, a few
/// times only: the core's refusal (a task since deleted elsewhere) reads as any other failure, so
/// a refusal must not be tried for ever. With the app not running, a request waits for its next
/// start.
final class WidgetRequests {
    /// The folder in the group's container.
    static let folder = "requests"
    /// Complete a Task's (`CompleteTaskIntent`), Today's Mark Done.
    static let completeTask = "complete_task"
    /// The Timer control's press (`TimerControlIntent`): Start or Stop the Timer's
    /// (`ToggleTimerIntent`), `Device.toggleTimer` as an assistant operation.
    static let toggleTimer = "toggle_timer"
    /// Every operation a request may run, each with the arguments it is given and must have:
    /// Complete a Task its task, the Timer's press nothing. Named one by one, never a pattern, so
    /// adding to it is a decision about what anything that can write the group may do.
    static let allowed: [String: [String]] = [completeTask: ["id"], toggleTimer: []]
    /// How many times a request whose run fails is run before it is given up.
    static let tries = 3

    /// One request: an operation and its arguments, by name.
    struct Request: Codable, Equatable {
        var operation: String
        var arguments: [String: String]
    }

    /// Writes `request` into `container`'s requests: to a name of its own, then renamed to one no
    /// other request has (the moment, then a UUID, so they are taken in the order made), so the
    /// app never reads half of one. Returns where it is.
    @discardableResult
    static func hand(_ request: Request, into container: URL) throws -> URL {
        let directory = container.appendingPathComponent(folder, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let name = "\(moment())-\(UUID().uuidString)"
        // Not a request until renamed: the app takes only `.json` that do not start with a dot.
        let partial = directory.appendingPathComponent(".\(name).partial")
        let whole = directory.appendingPathComponent("\(name).json")
        try JSONEncoder().encode(request).write(to: partial)
        do {
            try FileManager.default.moveItem(at: partial, to: whole)
        } catch {
            try? FileManager.default.removeItem(at: partial)
            throw error
        }
        return whole
    }

    /// The last moment a request was named by in this process.
    private static var named: UInt64 = 0
    private static let naming = NSLock()

    /// Now in nanoseconds, twenty digits, so names sort as they were made: later than the last
    /// this process named, since two can be made within the clock's step, and a UUID alone would
    /// order those at random.
    private static func moment() -> String {
        naming.lock()
        defer { naming.unlock() }
        named = max(clock_gettime_nsec_np(CLOCK_REALTIME), named + 1)
        let digits = String(named)
        return String(repeating: "0", count: max(0, 20 - digits.count)) + digits
    }

    #if !LUMENNA_WIDGETS
    /// Where requests are taken from.
    let directory: URL
    /// How many times each request kept has failed, by name.
    private var failures: [String: Int] = [:]
    /// Requests run whose file could not be removed: never run twice, since a repeating task
    /// would move on twice.
    private var finished: Set<String> = []
    private var source: DispatchSourceFileSystemObject?

    /// The requests in `container`'s.
    init(container: URL) {
        directory = container.appendingPathComponent(WidgetRequests.folder, isDirectory: true)
    }

    deinit {
        source?.cancel()
    }

    /// Takes in every request there, oldest first, running each with `run` (the operation's id
    /// and its arguments): `Device.assist`, in the app. Returns whether any ran, so the glance is
    /// written again.
    @discardableResult
    func take(running run: (String, [LumennaApp.Argument]) throws -> Void) -> Bool {
        let manager = FileManager.default
        guard let names = try? manager.contentsOfDirectory(atPath: directory.path) else { return false }
        var ran = false
        for name in names.sorted() where name.hasSuffix(".json") && !name.hasPrefix(".") && !finished.contains(name) {
            let file = directory.appendingPathComponent(name)
            guard let data = try? Data(contentsOf: file) else { continue }
            guard let request = try? JSONDecoder().decode(Request.self, from: data),
                  let taken = WidgetRequests.allowed[request.operation],
                  taken.allSatisfy({ !(request.arguments[$0] ?? "").isEmpty })
            else {
                try? manager.removeItem(at: file)
                continue
            }
            do {
                // What it takes alone, whatever else the file names.
                let arguments = taken.map { LumennaApp.Argument(name: $0, value: .text(request.arguments[$0] ?? "")) }
                try run(request.operation, arguments)
                ran = true
                failures[name] = nil
                if (try? manager.removeItem(at: file)) == nil { finished.insert(name) }
            } catch {
                let failed = (failures[name] ?? 0) + 1
                if failed >= WidgetRequests.tries {
                    failures[name] = nil
                    try? manager.removeItem(at: file)
                } else {
                    failures[name] = failed
                }
            }
        }
        return ran
    }

    /// Calls `changed` on the main thread whenever a file is added to or removed from the folder,
    /// which is made for it if there is none yet.
    func watch(_ changed: @escaping () -> Void) {
        guard source == nil else { return }
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let descriptor = open(directory.path, O_EVTONLY)
        guard descriptor >= 0 else { return }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: descriptor, eventMask: .write, queue: .main)
        source.setEventHandler(handler: changed)
        source.setCancelHandler { close(descriptor) }
        source.resume()
        self.source = source
    }
    #endif
}
