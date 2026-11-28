import LumennaApp
import Foundation
import UserNotifications

/// Reminders as the system's notifications, for every Apple app. The core decides when each
/// fires, what it says and what its buttons are called (`upcomingReminders`,
/// `deliverReminders`); this hands them to the notification center and answers its buttons.
///
/// - **iOS and watchOS schedule** (`ReminderSchedule`): the system fires what it was handed
///   even with the app gone, so the app hands over what fires in the next two days, at most
///   60 (iOS keeps 64 pending), and hands it over again on launch, on coming to the front,
///   after the store changes and on a background refresh.
/// - **The Mac delivers** (`ReminderDelivery`): it is resident, so it asks every 20 seconds
///   what has come due and shows that at once, while it holds the device's right to.
///
/// **One firing is shown once, on the iPhone and its watch alike.** Both keep the whole store,
/// so both schedule the same firing. A phone's notification goes to the watch when the phone
/// is locked and the watch worn, so the watch would show it twice: its own and the phone's.
/// Apple's answer (DTS, developer.apple.com/forums/thread/765669) is that watchOS waits for
/// the phone's notification to judge whether the two are one, and they are one when their
/// `UNNotificationRequest.identifier`s match and they fire together. So the identifier is the
/// firing's own, key and occurrence, the same on every device, and the trigger the same
/// moment on both. Without the phone's, the watch shows its own after about 13 seconds.
enum Reminders {
    /// The category every reminder is in, with the core's Dismiss and Snooze as its buttons.
    static let category = "dev.emassey0135.lumenna.reminder"
    static let dismissAction = "dev.emassey0135.lumenna.reminder.dismiss"
    static let snoozeAction = "dev.emassey0135.lumenna.reminder.snooze"
    /// The category of a block's end with work left short: Dismiss and Snooze, and the core's
    /// Carry Forward (`DueReminder.carry`), whose question the text already ends in.
    static let carryCategory = "dev.emassey0135.lumenna.reminder.carry"
    static let carryAction = "dev.emassey0135.lumenna.reminder.carry-forward"
    /// The category of a block's work outrunning its time: Dismiss and Snooze, and the core's
    /// Replan (`DueReminder.replan`), which opens the app at Replan from Now.
    static let replanCategory = "dev.emassey0135.lumenna.reminder.replan"
    static let replanAction = "dev.emassey0135.lumenna.reminder.replan-from-now"
    /// The category of a sitting's planned length running out with its timer running: the
    /// core's Stop Timer (`DueReminder.stop`), then Dismiss, and no Snooze, since it is said once
    /// and never again (its `snooze` is empty). An ordinary notification: nothing rings on.
    static let stopCategory = "dev.emassey0135.lumenna.reminder.time-up"
    static let stopAction = "dev.emassey0135.lumenna.reminder.stop-timer"
    /// The category of the day's energy check-in (`DueReminder.low`, its `kind` `day`): the
    /// core's I'm Low Today, then Dismiss and Snooze. Chosen itself, it opens Today with the
    /// check-in asked (the core's `energy-today` launch).
    static let energyCategory = "dev.emassey0135.lumenna.reminder.energy"
    static let lowAction = "dev.emassey0135.lumenna.reminder.low-today"
    /// What every reminder's identifier starts with, so re-arming cancels only its own.
    static let prefix = "reminder "
    /// How far ahead to schedule: the system holds 64 pending, so not a week of blocks.
    static let window: TimeInterval = 48 * 60 * 60
    /// At most this many pending, the earliest: under iOS's 64, with room to spare.
    static let most = 60

    /// The firing's own identifier, the same on every device: what a scheduled notification
    /// is replaced by, and how the watch knows the phone's is the same one.
    static func identifier(_ reminder: LumennaApp.DueReminder) -> String {
        "\(prefix)\(reminder.key) \(reminder.occurrence)"
    }

    /// The missed summary's identifier, named by the firings it sums up: the same on the phone
    /// and the watch, so a worn watch can take the phone's for its own rather than show both,
    /// and a summary shown again replaces the one before rather than stacking beside it.
    static func missedIdentifier(_ missed: [LumennaApp.DueReminder]) -> String {
        let last = missed.last.map { "\($0.key) \($0.occurrence)" } ?? ""
        return "\(prefix)missed \(missed.count) \(last)"
    }

    /// What the notification says: the target's name as its title, the core's whole message
    /// as its body, so a screen reader reads all of it and nothing rests on a sound or a look.
    static func content(_ reminder: LumennaApp.DueReminder) -> UNMutableNotificationContent {
        let content = UNMutableNotificationContent()
        content.title = reminder.title
        content.body = reminder.text
        content.sound = .default
        content.categoryIdentifier = !reminder.stop.isEmpty ? stopCategory
            : !reminder.low.isEmpty ? energyCategory
            : !reminder.carry.isEmpty ? carryCategory : !reminder.replan.isEmpty ? replanCategory : category
        // The reminders of one task or block group together.
        content.threadIdentifier = reminder.target
        content.userInfo = [
            "key": reminder.key, "occurrence": reminder.occurrence, "kind": reminder.kind, "target": reminder.target,
            // The sitting whose time ran out, which Stop Timer stops; empty for any other.
            "sitting": reminder.sitting,
        ]
        return content
    }

    /// The time-up category: the core's Stop Timer, then Dismiss, in the core's order
    /// (`Due::buttons`), and no Snooze.
    static func stopCategory(for reminder: LumennaApp.DueReminder) -> UNNotificationCategory {
        UNNotificationCategory(
            identifier: stopCategory,
            actions: [
                UNNotificationAction(identifier: stopAction, title: reminder.stop, options: []),
                UNNotificationAction(identifier: dismissAction, title: reminder.dismiss, options: []),
            ],
            intentIdentifiers: [],
            options: []
        )
    }

    /// The check-in's category: the core's I'm Low Today, then Dismiss and Snooze, in the core's
    /// order (`Due::buttons`).
    static func energyCategory(for reminder: LumennaApp.DueReminder) -> UNNotificationCategory {
        UNNotificationCategory(
            identifier: energyCategory,
            actions: [
                UNNotificationAction(identifier: lowAction, title: reminder.low, options: []),
                UNNotificationAction(identifier: dismissAction, title: reminder.dismiss, options: []),
                UNNotificationAction(identifier: snoozeAction, title: reminder.snooze, options: []),
            ],
            intentIdentifiers: [],
            options: []
        )
    }

    /// The category, its buttons worded by the core. Every reminder that snoozes words them
    /// alike, so any one of them names them; a time-up, which has no Snooze, never does.
    static func category(for reminder: LumennaApp.DueReminder) -> UNNotificationCategory {
        UNNotificationCategory(
            identifier: category,
            actions: [
                UNNotificationAction(identifier: dismissAction, title: reminder.dismiss, options: []),
                UNNotificationAction(identifier: snoozeAction, title: reminder.snooze, options: []),
            ],
            intentIdentifiers: [],
            options: []
        )
    }

    /// The carry category: the same buttons, then the core's Carry Forward.
    static func carryCategory(for reminder: LumennaApp.DueReminder) -> UNNotificationCategory {
        UNNotificationCategory(
            identifier: carryCategory,
            actions: [
                UNNotificationAction(identifier: dismissAction, title: reminder.dismiss, options: []),
                UNNotificationAction(identifier: snoozeAction, title: reminder.snooze, options: []),
                UNNotificationAction(identifier: carryAction, title: reminder.carry, options: []),
            ],
            intentIdentifiers: [],
            options: []
        )
    }

    /// The replan category: the same buttons, then the core's Replan, which brings the app to
    /// the front, since what it opens is a screen.
    static func replanCategory(for reminder: LumennaApp.DueReminder) -> UNNotificationCategory {
        UNNotificationCategory(
            identifier: replanCategory,
            actions: [
                UNNotificationAction(identifier: dismissAction, title: reminder.dismiss, options: []),
                UNNotificationAction(identifier: snoozeAction, title: reminder.snooze, options: []),
                UNNotificationAction(identifier: replanAction, title: reminder.replan, options: [.foreground]),
            ],
            intentIdentifiers: [],
            options: []
        )
    }

    /// The last reminder seen offering Replan, for the same reason as `lastCarried`.
    private static var lastReplanned: LumennaApp.DueReminder?

    /// A reminder that offered Carry Forward, last seen: what names the carry category's
    /// buttons when what is handed over now has none, since setting the categories replaces
    /// them all, and a carry notification already shown still needs its button.
    private static var lastCarried: LumennaApp.DueReminder?

    /// The last reminder seen offering Stop Timer, for the same reason as `lastCarried`.
    private static var lastStopped: LumennaApp.DueReminder?
    /// The last check-in seen, for the same reason as `lastCarried`.
    private static var lastChecked: LumennaApp.DueReminder?
    /// The last reminder seen that snoozes, which names the ordinary category's buttons when
    /// what is handed over now is only a time running out.
    private static var lastSnoozed: LumennaApp.DueReminder?

    /// Every category these reminders need, and the carry, replan and time-up ones once any has
    /// offered them.
    static func categories(for due: [LumennaApp.DueReminder]) -> Set<UNNotificationCategory> {
        if let carried = due.first(where: { !$0.carry.isEmpty }) { lastCarried = carried }
        if let replanned = due.first(where: { !$0.replan.isEmpty }) { lastReplanned = replanned }
        if let stopped = due.first(where: { !$0.stop.isEmpty }) { lastStopped = stopped }
        if let checked = due.first(where: { !$0.low.isEmpty }) { lastChecked = checked }
        if let snoozed = due.first(where: { !$0.snooze.isEmpty }) { lastSnoozed = snoozed }
        var categories = Set<UNNotificationCategory>()
        if let lastSnoozed { categories.insert(category(for: lastSnoozed)) }
        if let lastCarried { categories.insert(carryCategory(for: lastCarried)) }
        if let lastReplanned { categories.insert(replanCategory(for: lastReplanned)) }
        if let lastStopped { categories.insert(stopCategory(for: lastStopped)) }
        if let lastChecked { categories.insert(energyCategory(for: lastChecked)) }
        return categories
    }

    /// What to hand a system that schedules: each firing still to come, at its moment, the
    /// earliest `most`. One due now or past has fired already, or will not be scheduled late.
    static func requests(_ due: [LumennaApp.DueReminder], after now: Date) -> [UNNotificationRequest] {
        let calendar = Calendar.current
        return due
            .compactMap { reminder in Instant.parse(reminder.at).map { (reminder, $0) } }
            .filter { $0.1 > now }
            .sorted { $0.1 < $1.1 }
            .prefix(most)
            .map { reminder, at in
                // A calendar trigger, so the phone's and the watch's fire at the same moment.
                let parts = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: at)
                return UNNotificationRequest(
                    identifier: identifier(reminder),
                    content: content(reminder),
                    trigger: UNCalendarNotificationTrigger(dateMatching: parts, repeats: false)
                )
            }
    }

    #if !os(macOS)
    /// What a process with no `ReminderSchedule` of its own hands over once it has changed the
    /// store: the Timer control's press in the widget extension (the iPhone's, the watch's), the
    /// app perhaps not running, so a sitting's time running out is armed, or no longer, as the
    /// press left the timer. Setting the categories replaces them all, so those already set are
    /// kept beside these, which this process alone may not know of.
    static func arm(_ upcoming: LumennaApp.DueReminders, now: Date = Date(), centre: UNUserNotificationCenter = .current()) async {
        let requests = requests(upcoming.due, after: now)
        let wanted = Set(requests.map(\.identifier))
        if !upcoming.due.isEmpty {
            let ours = categories(for: upcoming.due)
            let kept = await centre.notificationCategories().filter { old in !ours.contains { $0.identifier == old.identifier } }
            centre.setNotificationCategories(kept.union(ours))
        }
        let stale = await centre.pendingNotificationRequests().map(\.identifier).filter { $0.hasPrefix(prefix) && !wanted.contains($0) }
        if !stale.isEmpty { centre.removePendingNotificationRequests(withIdentifiers: stale) }
        for request in requests { try? await centre.add(request) }
    }
    #endif

    /// Whether this process may ask for permission. A UI test's never does: the system's
    /// question would cover the app under test.
    static var mayAsk: Bool {
        ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] == nil
    }

    /// Asks to show notifications if the person has not yet been asked: when they first open
    /// a reminder list, add a reminder, turn reminders on, or make something a reminder will
    /// fire for. Never at launch, before there is anything to be reminded of.
    static func askIfUndecided(_ centre: NotificationCentre = UNUserNotificationCenter.current(), then done: @escaping () -> Void = {}) {
        guard mayAsk else { return done() }
        centre.authorization { status in
            guard status == .notDetermined else { return done() }
            centre.ask { _ in done() }
        }
    }

    /// Whether a change notice is the person's own edit here, rather than a sync bringing
    /// something in or the app coming back to the front: only their own asks for permission.
    static func isOwnEdit(_ note: Notification) -> Bool {
        note.object == nil && note.userInfo?["arrived"] == nil && note.userInfo?["fromPhone"] == nil
    }
}

/// The notification center as reminders use it: the seam a test replaces to see what would be
/// handed over. Callbacks come on the main thread.
protocol NotificationCentre: AnyObject {
    func pendingIdentifiers(_ done: @escaping ([String]) -> Void)
    func schedule(_ request: UNNotificationRequest)
    func cancel(_ identifiers: [String])
    func useCategories(_ categories: Set<UNNotificationCategory>)
    func authorization(_ done: @escaping (UNAuthorizationStatus) -> Void)
    func ask(_ done: @escaping (Bool) -> Void)
}

extension UNUserNotificationCenter: NotificationCentre {
    func pendingIdentifiers(_ done: @escaping ([String]) -> Void) {
        getPendingNotificationRequests { requests in
            let identifiers = requests.map(\.identifier)
            DispatchQueue.main.async { done(identifiers) }
        }
    }

    func schedule(_ request: UNNotificationRequest) {
        add(request, withCompletionHandler: nil)
    }

    func cancel(_ identifiers: [String]) {
        removePendingNotificationRequests(withIdentifiers: identifiers)
    }

    func useCategories(_ categories: Set<UNNotificationCategory>) {
        setNotificationCategories(categories)
    }

    func authorization(_ done: @escaping (UNAuthorizationStatus) -> Void) {
        getNotificationSettings { settings in
            let status = settings.authorizationStatus
            DispatchQueue.main.async { done(status) }
        }
    }

    func ask(_ done: @escaping (Bool) -> Void) {
        requestAuthorization(options: [.alert, .sound]) { granted, _ in
            DispatchQueue.main.async { done(granted) }
        }
    }
}

// What follows is the apps' own: a widget extension arms only what one press left (`arm`).
#if !LUMENNA_WIDGETS
/// Keeps the system's pending notifications to what the core says fires next (iOS, watchOS).
final class ReminderSchedule {
    private let device: LumennaApp.Device
    private let centre: NotificationCentre
    private let now: () -> Date
    private var queued = false
    private var observer: NSObjectProtocol?
    /// How many times it has re-armed: a test holds it to one per change, since a re-arm that
    /// caused another would keep the app from ever going idle.
    private(set) var rearms = 0

    init(device: LumennaApp.Device, centre: NotificationCentre = UNUserNotificationCenter.current(), now: @escaping () -> Date = Date.init) {
        self.device = device
        self.centre = centre
        self.now = now
    }

    /// Re-arms after every change to the store, whoever made it, and asks for permission
    /// after the person's own edit left something to remind them of.
    func follow() {
        observer = NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] note in
            self?.soon(asking: Reminders.isOwnEdit(note))
        }
    }

    /// Re-arms a moment from now, once for any number of changes in between: a sync brings
    /// in many at once.
    func soon(asking: Bool = false) {
        guard !queued else { return }
        queued = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            guard let self else { return }
            queued = false
            rearm(asking: asking)
        }
    }

    /// Shows what fired while nothing was armed for it: past the window last handed over, as
    /// when the app went unopened for two days. The system keeps what it was handed, so the
    /// core counts everything within that window as given (`kept`). Recent ones are shown as
    /// they are, at once, with their buttons; older ones as one summary. Returns what it
    /// handed over. Asked just before re-arming, which moves the window on.
    @discardableResult
    func catchUp() -> [UNNotificationRequest] {
        show(missed())
    }

    /// What the core says was missed; asked anywhere but the main thread when re-arming.
    private func missed() -> LumennaApp.DueReminders? {
        try? device.remindersMissed(shown: [], kept: true)
    }

    /// Hands what was missed to the notification center, on the main thread.
    @discardableResult
    private func show(_ found: LumennaApp.DueReminders?) -> [UNNotificationRequest] {
        guard let found else { return [] }
        let requests = ReminderDelivery.requests(found)
        if !found.due.isEmpty { centre.useCategories(Reminders.categories(for: found.due)) }
        requests.forEach(centre.schedule)
        return requests
    }

    /// Hands the notification center everything that fires in the next two days, replacing
    /// what it held of ours and cancelling what no longer fires; `catchingUp` first shows what
    /// was missed (on launch, on coming to the front, in a background refresh). `done` runs
    /// on the main thread once it has.
    ///
    /// Off the main thread, on a queue of its own: asking the core loads every year of the
    /// store, after every change, and the main thread must never wait on it, nor on the
    /// sync queue, which this never touches. One re-arm at a time, in order.
    func rearm(asking: Bool = false, catchingUp: Bool = false, then done: @escaping () -> Void = {}) {
        let device = device
        let now = now()
        rearms += 1
        work.async { [weak self] in
            let missed = catchingUp ? self?.missed() : nil
            let until = Instant.rfc3339(now.addingTimeInterval(Reminders.window))
            let upcoming = try? device.upcomingReminders(until: until)
            // What is handed over is the notification center's to do, quickly, on the main
            // thread, where its answers come back too.
            DispatchQueue.main.async {
                self?.show(missed)
                guard let upcoming else { return done() }
                self?.arm(upcoming, now: now, asking: asking, then: done)
            }
        }
    }

    private func arm(_ upcoming: LumennaApp.DueReminders, now: Date, asking: Bool, then done: @escaping () -> Void) {
        let centre = centre
        let requests = Reminders.requests(upcoming.due, after: now)
        if !upcoming.due.isEmpty { centre.useCategories(Reminders.categories(for: upcoming.due)) }
        let wanted = Set(requests.map(\.identifier))
        centre.pendingIdentifiers { pending in
            let stale = pending.filter { $0.hasPrefix(Reminders.prefix) && !wanted.contains($0) }
            if !stale.isEmpty { centre.cancel(stale) }
            // The same identifier replaces what was pending, so a snooze or an edit moves it.
            requests.forEach(centre.schedule)
            if asking, !requests.isEmpty {
                Reminders.askIfUndecided(centre, then: done)
            } else {
                done()
            }
        }
    }

    /// Where re-arming asks the core: never the main thread, never the sync queue. At the
    /// main thread's own priority but one, since the main thread may wait for the store while
    /// this holds it, and waiting on a lower priority is an inversion.
    private let work = DispatchQueue(label: "lumenna.reminders", qos: .userInitiated)
}

/// Gives reminders as they come due, for a process that runs (the Mac): while it holds the
/// device's right to (`takeReminders`), every 20 seconds and on coming to the front.
final class ReminderDelivery {
    private let device: LumennaApp.Device
    private let centre: NotificationCentre
    /// Whether this process has the device's right to give reminders, as the last look found.
    private var holding = false
    private var last: Date?
    // Foundation's, named in full: the core has a record called `Timer` too.
    private var timer: Foundation.Timer?
    static let every: TimeInterval = 20
    /// How often to ask again for the right while another process holds it.
    static let waiting: TimeInterval = 5

    init(device: LumennaApp.Device, centre: NotificationCentre = UNUserNotificationCenter.current()) {
        self.device = device
        self.centre = centre
    }

    func start() {
        timer = Foundation.Timer.scheduledTimer(withTimeInterval: Self.waiting, repeats: true) { [weak self] _ in
            self?.tick(force: false)
        }
        tick(force: true)
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        device.letGoOfReminders()
        holding = false
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    private var observer: NSObjectProtocol?

    /// After the person's own edit leaves something to remind them of, asks to show
    /// notifications if never asked, then looks at once.
    func follow() {
        observer = NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] note in
            guard let self, Reminders.isOwnEdit(note) else { return }
            let device = device
            work.async { [weak self] in
                let until = Instant.rfc3339(Date().addingTimeInterval(Reminders.window))
                guard let upcoming = try? device.upcomingReminders(until: until), !upcoming.due.isEmpty else { return }
                DispatchQueue.main.async {
                    guard let self else { return }
                    Reminders.askIfUndecided(self.centre) { [weak self] in self?.now() }
                }
            }
        }
    }

    /// Looks now, as on coming to the front, or when the store changed.
    func now() { tick(force: true) }

    /// Where the store is asked what is due: never the main thread, one look at a time.
    private let work = DispatchQueue(label: "lumenna.reminders.delivery", qos: .userInitiated)

    private func tick(force: Bool) {
        if !force, holding, let last, Date().timeIntervalSince(last) < Self.every { return }
        centre.authorization { [weak self] status in
            guard let self else { return }
            // Without permission nothing would be shown, so the right is left for another
            // process (`lum notify`) that can say them; given later, it starts where it last
            // stopped, with what was missed in one summary.
            guard [.authorized, .provisional].contains(status) else {
                device.letGoOfReminders()
                holding = false
                return
            }
            // Asking reads the store, every year of it: never on the main thread.
            let device = device
            work.async { [weak self] in
                let found = try? device.deliverReminders(now: nil)
                DispatchQueue.main.async { self?.show(found) }
            }
        }
    }

    /// Takes the right if free and shows what has come due, all at once on the calling thread;
    /// `now` is for tests. Returns what was handed to the notification center.
    @discardableResult
    func deliver(now: String?) -> [UNNotificationRequest] {
        show(try? device.deliverReminders(now: now))
    }

    /// What the core gave: shown, if this process holds the right to give them.
    @discardableResult
    private func show(_ found: LumennaApp.DueReminders?) -> [UNNotificationRequest] {
        // The right is the device's to give one process at a time: taken here when free, and
        // while another holds it, nothing is given.
        guard let found else { return [] }
        holding = !found.elsewhere
        guard holding else { return [] }
        last = Date()
        let requests = Self.requests(found)
        if !found.due.isEmpty { centre.useCategories(Reminders.categories(for: found.due)) }
        requests.forEach(centre.schedule)
        return requests
    }

    /// Each firing shown now, then what was missed as one notification of its own.
    static func requests(_ found: LumennaApp.DueReminders) -> [UNNotificationRequest] {
        var requests = found.due.map { reminder in
            UNNotificationRequest(identifier: Reminders.identifier(reminder), content: Reminders.content(reminder), trigger: nil)
        }
        if !found.missedSummary.isEmpty {
            let content = UNMutableNotificationContent()
            content.body = found.missedSummary
            content.sound = .default
            requests.append(UNNotificationRequest(identifier: Reminders.missedIdentifier(found.missed), content: content, trigger: nil))
        }
        return requests
    }
}

/// What a reminder's notification opens: its task, or its block on its day.
struct RemindedOf {
    let kind: String
    let target: String
    /// The occurrence's day, ISO: with the key, which firing it was.
    let day: String
    /// Whether it was the overrun warning's Replan: the app opens at Replan from Now.
    var replan = false
    /// The firing's key (`DueReminder.key`).
    var key = ""
}

/// Answers a reminder's buttons, the core's Dismiss, Snooze, Carry Forward and Stop Timer, and opens what it is about
/// when it is chosen. Set as the notification center's delegate before launching finishes,
/// so a button pressed while the app was not running reaches it.
final class ReminderResponder: NSObject, UNUserNotificationCenterDelegate {
    private let device: LumennaApp.Device
    private let open: (RemindedOf) -> Void

    init(device: LumennaApp.Device, open: @escaping (RemindedOf) -> Void) {
        self.device = device
        self.open = open
    }

    /// In front, a reminder still shows and sounds, so a screen reader announces it as any
    /// notification.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter, willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .list, .sound])
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let info = response.notification.request.content.userInfo
        let action = response.actionIdentifier
        DispatchQueue.main.async { [self] in
            answer(action, info: info)
            completionHandler()
        }
    }

    /// What one of a reminder's buttons does, or choosing the notification itself.
    func answer(_ action: String, info: [AnyHashable: Any]) {
        guard let key = info["key"] as? String, let occurrence = info["occurrence"] as? String else { return }
        do {
            let change: LumennaApp.Change
            switch action {
            case Reminders.dismissAction: change = try device.dismissReminder(key: key, occurrence: occurrence)
            case Reminders.snoozeAction: change = try device.snoozeReminder(key: key, occurrence: occurrence, minutes: nil)
            case Reminders.carryAction:
                guard let target = info["target"] as? String else { return }
                change = try device.carryRestForward(block: "\(target)@\(occurrence)")
            case Reminders.stopAction:
                // The sitting's timer stopped, its time logged, and the firing dismissed on every
                // device, as the core's Stop Timer does in the app.
                guard let sitting = info["sitting"] as? String, !sitting.isEmpty else { return }
                let stopped = Result { try device.stopTimer(assignment: sitting, minutes: nil) }
                _ = try? device.dismissReminder(key: key, occurrence: occurrence)
                NotificationCenter.default.post(name: Core.changed, object: nil)
                switch stopped {
                case let .success(timer): Announcer.say(timer.announcement, notices: timer.notices)
                case let .failure(error): Announcer.say(error.sentence)
                }
                return
            case Reminders.lowAction:
                // Low today, said as the core says it, and the check-in dismissed on every device.
                change = try device.checkInEnergy(level: "low")
                _ = try? device.dismissReminder(key: key, occurrence: occurrence)
            case UNNotificationDefaultActionIdentifier where info["kind"] as? String == "day":
                // The day's energy check-in: Today, with its question asked, the core's launch.
                Task { @MainActor in Launches.shared.ask("energy-today") }
                return
            case Reminders.replanAction:
                open(RemindedOf(kind: info["kind"] as? String ?? "block", target: info["target"] as? String ?? "", day: occurrence, replan: true, key: key))
                return
            case UNNotificationDefaultActionIdentifier:
                if let kind = info["kind"] as? String, let target = info["target"] as? String {
                    open(RemindedOf(kind: kind, target: target, day: occurrence, key: key))
                }
                return
            default: return
            }
            // Every view reads the store again, the schedule follows, and the watch sends it on.
            NotificationCenter.default.post(name: Core.changed, object: nil)
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            Announcer.say(error.sentence)
        }
    }
}
#endif
