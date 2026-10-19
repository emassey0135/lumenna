import Foundation
import LumennaApp

/// The app itself (crates/app, through crates/mobile and BoltFFI), over this device's store:
/// what the screens draw, and where they send what the person does.
///
/// What is in hand, what is folded, which question is open and what to say are the app's; a
/// screen draws `view` with `lines` and sends events, and decides none of them. Every turn the
/// app leaves comes in on a thread of the app's own and is taken on the main thread, in order:
/// drawn — each screen showing it is told (`AppModel.shown`) — then said.
///
/// While the screens move onto it, it runs beside the surface (`Core.lumenna`), each on a
/// connection of its own: what the surface writes it takes in on `Core.changed`, and what it
/// writes the surface sees as another process's write (`Core.watchForOtherProcesses`).
final class AppModel {
    /// Posted on the main thread once a turn is in `view`, for every screen showing it to draw.
    static let shown = Notification.Name("LumennaAppShown")
    /// Posted with the files a turn made for the person to keep — an export, a backup — under
    /// `AppModel.saving`, for the screen that asked to save them where the person chooses.
    static let files = Notification.Name("LumennaAppFiles")
    static let saving = "files"

    /// The store, as the app's own connection holds it: what runs without the screens.
    let device: LumennaApp.Device
    private var app: LumennaApp.App!
    private let relay = Relay()

    /// What to draw: the view model as the app last left it.
    private(set) var view: LumennaApp.ViewModel
    /// The view's lines, in this device's clock and words.
    private(set) var lines: LumennaApp.Lines
    private var seq: UInt32 = 0
    // Foundation's, named in full: the core has a record called `Timer` too.
    private var minutes: Foundation.Timer?

    /// The app over the profile in `directory`, saying it runs `platform` in pairing.
    init(directory: URL, platform: String) throws {
        device = try LumennaApp.Device(directory: directory.path)
        app = try LumennaApp.App(device: device, platform: platform, locale: Writing(), shell: relay)
        view = app.view()
        lines = app.lines()
        relay.model = self
        app.send(event: .clock(Clock.style == .twelveHour ? .twelveHour : .twentyFourHour))
        // Title Case, as Apple's platforms write buttons and menus.
        app.send(event: .capitals(.title))
        #if os(macOS)
        if ProcessInfo.processInfo.environment["LUMENNA_NO_SYNC"] != nil {
            // The UI tests' copy: syncing would listen on the network, and on a fresh machine
            // macOS then asks about local networks and incoming connections, over the window
            // under test.
            app.inFront(inFront: false)
        } else {
            // Resident, so in front for as long as it runs: the device's sync process, with no
            // daemon to set up. While it holds the device's endpoint it answers the command
            // surface, as the daemon would, so Emacs and the BTSpeak app reach it.
            app.serveCommands(app: "Lumenna for Mac", deviceName: Host.current().localizedName ?? "Mac")
        }
        #endif
        // What the surface writes, the app takes in; and the minutes, for what is now and due.
        NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.app.tick()
        }
        // Once a second while screens still write through the surface, not all of which say so:
        // a tick only looks whether the store or the minute moved.
        minutes = Foundation.Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.app.tick()
        }
    }

    /// Sends what the person did.
    func send(_ event: LumennaApp.ShellEvent) {
        app.send(event: event)
    }

    /// What could go at `cursor` (UTF-8 bytes) in a line of `syntax` outside the app's own quick
    /// add: a filter.
    func complete(_ text: String, cursor: UInt32, syntax: LumennaApp.Syntax) throws -> LumennaApp.Completions {
        try app.complete(text: text, cursor: cursor, syntax: syntax)
    }

    /// The view as the app holds it this moment, ahead of any turn still on its way: for what
    /// must not act on a turn made before the person's last answer (the question open).
    var current: (view: LumennaApp.ViewModel, lines: LumennaApp.Lines) { (app.view(), app.lines()) }

    /// The next file a turn makes, once, for whoever asked for it: an export to save.
    func nextFile(_ then: @escaping (LumennaApp.SaveFile) -> Void) {
        var token: NSObjectProtocol?
        token = NotificationCenter.default.addObserver(forName: AppModel.files, object: self, queue: .main) { note in
            guard let files = note.userInfo?[AppModel.saving] as? [LumennaApp.SaveFile], let file = files.first else { return }
            if let token { NotificationCenter.default.removeObserver(token) }
            then(file)
        }
    }

    /// The app came to the front or left it: it keeps the device in sync only while in front.
    func inFront(_ inFront: Bool) {
        app.inFront(inFront: inFront)
    }

    /// Takes in a change made elsewhere, or a new minute.
    func tick() {
        app.tick()
    }

    /// Whether the app is still: nothing with the store, and every turn drawn. Tests wait on it,
    /// as a person waits for the screen to settle.
    var isStill: Bool { relay.waiting == 0 && app.isStill() }

    /// Runs `then` on the main thread once the app is still: at once if it is, else as soon as
    /// it is found so, looking every 50 milliseconds. A form takes the app's values only then,
    /// since an answer for earlier typing would undo later typing; and the last turn's own
    /// notice can come while a tick of the store's is still under way, with no turn after it.
    func whenStill(_ then: @escaping () -> Void) {
        if isStill { return then() }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.whenStill(then) }
    }

    fileprivate func take(_ turn: LumennaApp.Turn) {
        guard turn.seq > seq else { return }
        seq = turn.seq
        view = turn.view
        lines = turn.lines
        NotificationCenter.default.post(name: AppModel.shown, object: self)
        if !turn.files.isEmpty {
            NotificationCenter.default.post(name: AppModel.files, object: self, userInfo: [AppModel.saving: turn.files])
        }
        // Said after the screens have drawn and moved focus, queued behind what VoiceOver is
        // saying, so neither cuts the other off.
        for said in turn.said { Announcer.say(said) }
    }
}

/// The app's turns, from its own thread onto the main one, in order, counted until drawn.
private final class Relay: LumennaApp.Shell, @unchecked Sendable {
    weak var model: AppModel?
    private let lock = NSLock()
    private var handed = 0

    var waiting: Int { lock.withLock { handed } }

    func turn(turn: LumennaApp.Turn) {
        lock.withLock { handed += 1 }
        DispatchQueue.main.async { [weak self] in
            // Counted as drawn before it is shown, so what follows the last turn finds the
            // app still.
            self?.lock.withLock { self?.handed -= 1 }
            self?.model?.take(turn)
        }
    }
}

/// This device's way of writing a time and a date, for the lines the core words. "Today",
/// "Tomorrow" and when to give the year are the core's.
private final class Writing: LumennaApp.Locale, @unchecked Sendable {
    func time(hour: Int8, minute: Int8) -> String {
        Clock.time(String(format: "%02d:%02d", hour, minute))
    }

    func date(year: Int16, month: Int8, day: Int8, withYear: Bool) -> String {
        var parts = DateComponents()
        parts.year = Int(year)
        parts.month = Int(month)
        parts.day = Int(day)
        guard let date = Calendar.current.date(from: parts) else { return "\(year)-\(month)-\(day)" }
        let style = Date.FormatStyle.dateTime.weekday(.wide).day().month(.wide)
        return withYear ? date.formatted(style.year()) : date.formatted(style)
    }
}

extension LumennaApp.Action {
    /// Whether it asks something before it runs: a menu item for it ends in "…" on the Mac.
    var asks: Bool { question != .immediate }
}

extension AppModel {
    /// What running one of a row's actions sends: the row in hand, then the action. A row of a
    /// list beside the main one (`side`: the places, a reminder list, the devices) is held
    /// there instead, so the change lands back in that list.
    func run(_ action: LumennaApp.Action, on key: String, side: LumennaApp.Side? = nil) {
        if let side {
            send(.hold(field0: side, field1: key))
        } else {
            send(.focus(key))
        }
        send(.perform(action))
    }

    /// A row's speech after its title: its line's detail, whether what is under it is folded
    /// away, and the level where it changes from the row shown before — what a tree control
    /// would say by itself, and a list cell does not.
    static func spoken(detail: String?, expanded: Bool?, level: String?) -> String {
        [detail.flatMap { $0.isEmpty ? nil : $0 }, expanded.map { $0 ? "expanded" : "collapsed" }, level]
            .compactMap { $0 }
            .joined(separator: ", ")
    }
}

/// The keys of the day's own commands, Command-Option and a letter, the same on the Mac and
/// the iPad: Plan the Day and Replan from Now, the two a person reaches for mid-day. The rest
/// are menu items only. Not Control-Option, which is VoiceOver's.
enum DayKeys {
    static let planDay = "p"
    static let replan = "r"

    static func key(for kind: LumennaApp.ActionKind) -> String? {
        switch kind {
        case .planDay: planDay
        case .replan: replan
        default: nil
        }
    }

    static func kind(for key: String) -> LumennaApp.ActionKind? {
        switch key {
        case planDay: .planDay
        case replan: .replan
        default: nil
        }
    }
}
