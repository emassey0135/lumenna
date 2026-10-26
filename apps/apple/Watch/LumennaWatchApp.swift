import LumennaApp
import SwiftUI
import UserNotifications
import WatchKit

@main
struct LumennaWatchApp: SwiftUI.App {
    @WKApplicationDelegateAdaptor private var delegate: WatchDelegate
    @StateObject private var opened = Opened.shared
    @Environment(\.scenePhase) private var phase

    init() {
        // Siri and Shortcuts' intents run on this store, and may be what launched the app. A
        // launch they ask for goes where every other way in goes (`Launches`), so the root
        // follows it: sent to the app alone, the screens never went there.
        if let core = Opened.shared.core { Assistant.serve(core.app.device) { Launches.shared.ask($0) } }
    }

    var body: some Scene {
        WindowGroup {
            Group {
                if let core = opened.core {
                    RootView(core: core)
                        .environmentObject(core)
                        .environmentObject(core.phone)
                } else {
                    // Nothing works without the store, so the reason is the whole screen.
                    ScrollView {
                        Text(LumennaApp.storeUnopened(reason: opened.reason))
                    }
                }
            }
            .onChange(of: phase) { _, phase in
                switch phase {
                case .active:
                    // The wearer may have flown somewhere.
                    opened.core?.timeZoneMayHaveChanged()
                    opened.core?.phone.syncNow()
                    opened.reminders?.rearm(catchingUp: true)
                case .background:
                    opened.core.map(Self.settle)
                default:
                    break
                }
            }
        }
    }
}

extension LumennaWatchApp {
    /// Leaving: the app is given time to finish what it is doing with the store before the
    /// system suspends it. The store is in the App Group's container, and watchOS, as iOS, ends
    /// a process suspended while it holds a lock on a file there (`0xdead10cc`): a write under
    /// way as the wearer lowered their wrist would be one.
    static func settle(_ core: Core) {
        ProcessInfo.processInfo.performExpiringActivity(withReason: "Finishing with the store") { expired in
            guard !expired else { return }
            let still = DispatchSemaphore(value: 0)
            DispatchQueue.main.async { core.app.whenStill { still.signal() } }
            // Never held for long: the system's allowance is short.
            _ = still.wait(timeout: .now() + 10)
        }
    }
}

/// The store, opened once for the process's life, and the reminders the watch schedules from
/// it. Shared, since a refresh in the background starts the app without a scene.
final class Opened: ObservableObject {
    static let shared = Opened()
    /// Posted with a `RemindedOf` when a reminder's notification is chosen.
    static let reminded = Notification.Name("LumennaReminded")

    let core: Core?
    let reason: String
    /// The watch's own reminders: it keeps a whole store, so it schedules what fires from it,
    /// with the same identifiers as the phone's, so the two show once (`Shared/Reminders.swift`).
    let reminders: ReminderSchedule?
    private let responder: ReminderResponder?

    private init() {
        do {
            let core = try Core()
            self.core = core
            reason = ""
            let responder = ReminderResponder(device: core.app.device) { reminded in
                NotificationCenter.default.post(name: Opened.reminded, object: reminded)
            }
            UNUserNotificationCenter.current().delegate = responder
            self.responder = responder
            let reminders = ReminderSchedule(device: core.app.device)
            reminders.follow()
            reminders.rearm(catchingUp: true)
            self.reminders = reminders
        } catch {
            core = nil
            reason = error.sentence
            reminders = nil
            responder = nil
        }
    }
}

/// Launching, and the background refreshes that carry the reminders' two-day window forward
/// while the app is not opened.
final class WatchDelegate: NSObject, WKApplicationDelegate {
    /// How soon to ask for the next refresh; watchOS treats it as the earliest.
    private static let interval: TimeInterval = 6 * 60 * 60

    func applicationDidFinishLaunching() {
        // The notification center's delegate is set before launching finishes, so a
        // reminder's button pressed while the app was not running reaches it.
        _ = Opened.shared
        Self.scheduleRefresh()
    }

    func handle(_ backgroundTasks: Set<WKRefreshBackgroundTask>) {
        for task in backgroundTasks {
            // The phone handed something on while the app was not running: held until it is in.
            if let delivery = task as? WKWatchConnectivityRefreshBackgroundTask, let core = Opened.shared.core {
                core.phone.deliver(during: delivery)
                continue
            }
            guard let refresh = task as? WKApplicationRefreshBackgroundTask, let reminders = Opened.shared.reminders else {
                task.setTaskCompletedWithSnapshot(false)
                continue
            }
            Self.scheduleRefresh()
            reminders.rearm(catchingUp: true) { refresh.setTaskCompletedWithSnapshot(false) }
        }
    }

    private static func scheduleRefresh() {
        WKApplication.shared().scheduleBackgroundRefresh(withPreferredDate: Date(timeIntervalSinceNow: interval), userInfo: nil) { _ in }
    }
}
