import Foundation
import LumennaApp
#if os(iOS)
import UIKit
#endif

/// The app itself (crates/app, through crates/ffi and BoltFFI), over this device's store:
/// what the screens draw, and where they send what the person does.
///
/// What is in hand, what is folded, which question is open and what to say are the app's; a
/// screen draws `view` with `lines` and sends events, and decides none of them. Every turn the
/// app leaves comes in on a thread of the app's own and is taken on the main thread, in order:
/// drawn — each screen showing it is told (`AppModel.shown`) — then said.
final class AppModel {
    /// Posted on the main thread once a turn is in `view`, for every screen showing it to draw.
    static let shown = Notification.Name("LumennaAppShown")

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
    /// Who asked for the file a turn makes next (an export, to save where the person chooses),
    /// and which request it was: one at a time, the latest asking.
    private var fileWanted: (request: Int, then: (LumennaApp.SaveFile) -> Void)?
    private var fileRequests = 0

    /// The app over the profile in `directory`, saying it runs `platform` in pairing.
    init(directory: URL, platform: String) throws {
        device = try LumennaApp.Device(directory: directory.path)
        app = try LumennaApp.App(device: device, platform: platform, locale: Writing(), shell: relay)
        view = app.view()
        lines = app.lines()
        relay.model = self
        app.send(event: .clock(Clock.style))
        // Title Case, as Apple's platforms write buttons and menus.
        app.send(event: .capitals(.title))
        // What pairing's sentences call this device, which finds others on its network.
        app.send(event: .pairingAs(called: AppModel.thisDevice, nearby: true))
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
        // Opening at login, which Settings offers in the core's words; told again as the app
        // comes to the front, since System Settings can change it meanwhile.
        app.send(event: LoginItem.offered)
        #endif
        // What a sync, the watch or another process wrote, the app takes in; and the minutes,
        // for what is now and due. A tick only looks whether the store or the minute moved.
        NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.app.tick()
        }
        minutes = Foundation.Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.app.tick()
        }
        // The person changed the region or chose a 12- or 24-hour clock: every time is worded
        // again in it, the core's sentences and the lines alike.
        NotificationCenter.default.addObserver(forName: NSLocale.currentLocaleDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            Clock.localeChanged()
            self?.app.send(event: .clock(Clock.style))
        }
    }

    /// Sends what the person did.
    func send(_ event: LumennaApp.ShellEvent) {
        lastSent = Date()
        app.send(event: event)
    }

    /// When the person last did something, for telling their changes from arrivals.
    private(set) var lastSent = Date.distantPast

    /// Tells the app something of this device's own, not the person's doing: how opening at
    /// login stands. Not counted as theirs, so a change it brings is not taken for their edit.
    func tell(_ event: LumennaApp.ShellEvent) {
        app.send(event: event)
    }

    /// An overrun warning's Replan, pressed in its notification: the app's (`DueEvent::Replan`),
    /// which shows today with Replan from Now over it, the screens following.
    func replan(from reminded: RemindedOf) {
        send(.due(.replan(reminded.due)))
    }

    /// What a widget or complication shows for the next `hours`, the core's, worded in this
    /// device's clock (`GlanceFile`). Safe off the main thread.
    func glance(hours: UInt32) throws -> [LumennaApp.GlanceEntry] {
        try app.glance(hours: hours)
    }

    /// What could go at `cursor` (UTF-8 bytes) in a line of `syntax` outside the app's own quick
    /// add: a filter.
    func complete(_ text: String, cursor: UInt32, syntax: LumennaApp.Syntax) throws -> LumennaApp.Completions {
        try app.complete(text: text, cursor: cursor, syntax: syntax)
    }

    /// The view as the app holds it this moment, ahead of any turn still on its way: for what
    /// must not act on a turn made before the person's last answer (the question open).
    var current: (view: LumennaApp.ViewModel, lines: LumennaApp.Lines) { (app.view(), app.lines()) }

    /// Sends `event`, which makes a file (an export), and hands that file to `then`, once. Asked
    /// again, the newer request is the one answered; and once the app is done with it without a
    /// file (it refused, and said why), nobody waits for one, so a later export never reaches an
    /// earlier handler.
    func file(from event: LumennaApp.ShellEvent, then: @escaping (LumennaApp.SaveFile) -> Void) {
        fileRequests += 1
        let request = fileRequests
        fileWanted = (request, then)
        send(event)
        whenStill { [weak self] in
            if self?.fileWanted?.request == request { self?.fileWanted = nil }
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
        if let file = turn.files.first, let wanted = fileWanted {
            fileWanted = nil
            wanted.then(file)
        }
        // Said after the screens have drawn and moved focus, queued behind what VoiceOver is
        // saying, so neither cuts the other off.
        for said in turn.said { Announcer.say(said) }
        #if os(macOS)
        // What the person chose for opening at login: the system's to do, and the app told how
        // it went, which it says.
        if let on = turn.background { tell(LoginItem.set(on)) }
        #endif
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

/// What a row's menus, swipes and keys need of an action: one offered whole (`Action`), or a
/// list row's in short (`Offered`), which the app runs whole for the row in hand.
protocol RowAction: Hashable {
    var kind: LumennaApp.ActionKind { get }
    var title: String { get }
    var sentence: String { get }
    var primary: Bool { get }
    var destructive: Bool { get }
    /// Whether it asks something before it runs: a menu item for it ends in "…" on the Mac.
    var asksFirst: Bool { get }
}

extension LumennaApp.Action: RowAction {
    var asksFirst: Bool { question != .immediate }
}

extension LumennaApp.Offered: RowAction {
    var asksFirst: Bool { asks != .nothing }
}

extension AppModel {
    /// Runs one of a row's actions, in short, on the row `key` names: a row of the place shown,
    /// or of a list beside the main one (`side`: the places, a reminder list, the devices), so
    /// the change lands back in that list. The app puts the row in hand and runs its action
    /// whole as it has it now.
    func run(_ action: LumennaApp.Offered, on key: String, side: LumennaApp.Side? = nil) {
        act(action, on: side.map { .held(field0: $0, field1: key) } ?? .row(key))
    }

    /// Runs one of the actions offered on what `on` names: a row, the day, the details' task,
    /// the reminder list.
    func act(_ action: LumennaApp.Offered, on: LumennaApp.ActingOn) {
        send(.actOn(field0: on, field1: action))
    }

    /// A shared key pressed on the row `key` names (or held in `side`), by its id in the core's
    /// table: "delete", "toggle". What it runs is the core's (`keys::runs`), the same on every
    /// app, refused in the core's words where the row has none of it.
    func key(_ id: String, on key: String, side: LumennaApp.Side? = nil) {
        if let side {
            send(.hold(field0: side, field1: key))
        } else {
            send(.focus(key))
        }
        send(.key(id))
    }

    /// A command of the core's table (`keys::what_runs`) by its id: the row in hand's own action
    /// ("mark-done"), the day's ("plan-day"), or the app's ("go-today"), refused in the core's
    /// words where it has none. What each id runs is the core's, never a list of kinds here.
    func key(_ id: String) {
        send(.key(id))
    }

    /// How pairing's sentences name this device: "this iPhone", "this Mac".
    static var thisDevice: String {
        #if os(iOS)
        "this \(UIDevice.current.model)"
        #elseif os(macOS)
        "this Mac"
        #else
        "this watch"
        #endif
    }

    /// A row's speech after its title: its line's detail, whether what is under it is folded
    /// away, and the level where it changes from the row shown before — what a tree control
    /// would say by itself, and a list cell does not.
    static func spoken(detail: String?, expanded: Bool?, level: String?) -> String {
        [detail.flatMap { $0.isEmpty ? nil : $0 }, expanded.map { LumennaApp.foldState(expanded: $0) }, level]
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

    /// The letter shown beside one of the day's actions in a menu.
    static func key(for kind: LumennaApp.ActionKind) -> String? {
        switch kind {
        case .planDay: planDay
        case .replan: replan
        default: nil
        }
    }

    /// The core's command a letter runs.
    static func command(for key: String) -> String? {
        switch key {
        case planDay: "plan-day"
        case replan: "replan"
        default: nil
        }
    }
}

/// One row of a list drawn from the app: what identifies it to the app (the key it is held
/// by), how it reads, and what can be done to it, the app's.
struct AppItem: Hashable {
    var key: String
    var title: String
    var detail: String?
    var depth: UInt32 = 0
    /// What VoiceOver says after the title: the detail, then folding and the level.
    var spoken: String?
    /// Whether what is under it is shown, for a row with something under it.
    var expanded: Bool?
    /// "level 2", where the level changes from the row shown before.
    var level: String?
    /// A heading over the items under it, as the sidebar's Projects and Labels are.
    var heading = false
    var actions: [LumennaApp.Offered] = []
    /// Why it is offered, a line beneath and VoiceOver's hint, never its name.
    var reason: String?
    /// A project's, label's, saved filter's or colour's colour, drawn beside its name.
    var swatch: LumennaApp.Swatch?
}

extension AppModel {
    /// The sidebar's rows as a list shows them, not folded away, each saying its detail, whether
    /// it is folded and, against the row shown before, its level: the app's (`SidebarEntry`'s
    /// `hidden`, `expanded`, `level`). `under` limits them to one heading's places, a level up,
    /// whose level is said against those rows alone, in the core's words.
    func sidebarItems(under group: LumennaApp.SidebarGroup? = nil) -> [AppItem] {
        var items: [AppItem] = []
        var inside = group == nil
        var previous: UInt32 = 0
        for entry in view.sidebar {
            if let group {
                if entry.depth == 0 {
                    if case let .group(heading) = entry.kind { inside = heading == group } else { inside = false }
                    continue
                }
                guard inside else { continue }
            }
            guard !entry.hidden else { continue }
            let depth = group == nil ? entry.depth : entry.depth - 1
            let level: String?
            if group == nil {
                level = entry.level
            } else {
                level = depth != previous ? LumennaApp.levelWords(depth: depth) : nil
                previous = depth
            }
            var item = AppItem(key: entry.key, title: entry.title, detail: entry.detail.isEmpty ? nil : entry.detail, depth: depth, actions: entry.offered, swatch: entry.swatch)
            item.spoken = AppModel.spoken(detail: entry.detail, expanded: entry.expanded, level: level)
            item.expanded = entry.expanded
            item.level = level
            if case .group = entry.kind { item.heading = true }
            items.append(item)
        }
        return items
    }

    /// The sidebar entry held by `key`.
    func sidebarEntry(_ key: String) -> LumennaApp.SidebarEntry? {
        view.sidebar.first { $0.key == key }
    }
}
