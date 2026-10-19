import SwiftUI
import UserNotifications
import WatchKit

@main
struct LumennaWatchApp: App {
    @WKApplicationDelegateAdaptor private var delegate: WatchDelegate
    @StateObject private var opened = Opened.shared
    @Environment(\.scenePhase) private var phase

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
                        Text("Lumenna could not open its store. \(opened.reason)")
                    }
                }
            }
            .onChange(of: phase) { _, phase in
                guard phase == .active else { return }
                opened.core?.phone.syncNow()
                opened.reminders?.rearm(catchingUp: true)
            }
        }
    }
}

/// The store, opened once for the process's life, and the reminders the watch schedules from
/// it. Shared, since a refresh in the background starts the app without a scene.
final class Opened: ObservableObject {
    static let shared = Opened()
    /// Posted with a `RemindedOf` when a reminder's notification is chosen.
    static let reminded = Notification.Name("LumennaReminded")

    let core: WatchCore?
    let reason: String
    /// The watch's own reminders: it keeps a whole store, so it schedules what fires from it,
    /// with the same identifiers as the phone's, so the two show once (`Shared/Reminders.swift`).
    let reminders: ReminderSchedule?
    private let responder: ReminderResponder?

    private init() {
        do {
            let core = try WatchCore()
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
