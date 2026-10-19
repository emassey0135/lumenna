import BackgroundTasks
import UIKit
import UserNotifications

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    /// The open store, one per process. Owned here rather than by a scene, because iOS starts
    /// the app in the background for a refresh with no scene at all (`BackgroundSync`).
    static let core: Result<Core, Error> = Result { try Core() }
    /// The Apple Watch's sync, answered here (`WatchLink`).
    private var watchLink: WatchLink?
    /// The reminders handed to the system to fire (`Shared/Reminders.swift`), one per process.
    static private(set) var reminders: ReminderSchedule?
    /// Answers a reminder's Dismiss and Snooze; the notification center holds it weakly.
    private var responder: ReminderResponder?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        // A refresh task's handler has to be registered before launching finishes.
        BackgroundSync.register()
        // At launch, since a message from the watch is what starts the app in the background.
        if case let .success(core) = AppDelegate.core {
            // What the Crux app's connection writes, the screens still on the surface redraw for.
            core.watchForOtherProcesses()
            watchLink = WatchLink(core: core)
            // Before launching finishes, so a reminder's button pressed while the app was not
            // running reaches it.
            let responder = ReminderResponder(device: core.app.device) { reminded in
                if reminded.replan {
                    Self.root?.openReplan?()
                } else {
                    Self.root?.openReminded?(kind: reminded.kind, target: reminded.target, day: reminded.day)
                }
            }
            UNUserNotificationCenter.current().delegate = responder
            self.responder = responder
            let reminders = ReminderSchedule(device: core.app.device)
            reminders.follow()
            reminders.rearm(catchingUp: true)
            Self.reminders = reminders
        }
        return true
    }

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        UISceneConfiguration(name: "Default", sessionRole: connectingSceneSession.role)
    }

    /// What the window shows, for a reminder chosen to open what it is about.
    private static var root: CommandActions? {
        UIApplication.shared.connectedScenes
            .compactMap { ($0 as? UIWindowScene)?.keyWindow ?? ($0 as? UIWindowScene)?.windows.first }
            .first?.rootViewController as? CommandActions
    }

    /// The iPad's menu bar, and the shortcuts a held ⌘ lists (`KeyboardCommands`).
    override func buildMenu(with builder: UIMenuBuilder) {
        super.buildMenu(with: builder)
        KeyboardCommands.build(builder)
    }
}
