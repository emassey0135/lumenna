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
        // Under UI tests, nothing animates: under load, an animation left unfinished as the
        // iPad's sidebar was hidden had XCUITest wait a minute a step for the app to be still,
        // though the app was idle; and the audit judged text still fading in.
        if ProcessInfo.processInfo.environment["LUMENNA_NO_ANIMATIONS"] != nil {
            UIView.setAnimationsEnabled(false)
        }
        // A refresh task's handler has to be registered before launching finishes.
        BackgroundSync.register()
        // At launch, since a message from the watch is what starts the app in the background.
        if case let .success(core) = AppDelegate.core {
            // Siri, Shortcuts and Spotlight's intents run on this store, and may be what
            // launched the app. A launch they ask for (a search result chosen) goes where every
            // other way in goes, so the screens follow it.
            Assistant.serve(core.app.device) { Launches.shared.ask($0) }
            // What the Crux app's connection writes, the screens still on the surface redraw for.
            core.watchForChanges()
            watchLink = WatchLink(core: core)
            // Before launching finishes, so a reminder's button pressed while the app was not
            // running reaches it.
            let responder = ReminderResponder(device: core.app.device) { reminded in
                if reminded.replan {
                    // The app's: today, with Replan from Now over it, which the screens follow.
                    core.app.replan(from: reminded)
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
