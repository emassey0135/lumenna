import BackgroundTasks
import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    /// The open store, one per process. Owned here rather than by a scene, because iOS starts
    /// the app in the background for a refresh with no scene at all (`BackgroundSync`).
    static let core: Result<Core, Error> = Result { try Core() }

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        // A refresh task's handler has to be registered before launching finishes.
        BackgroundSync.register()
        return true
    }

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        UISceneConfiguration(name: "Default", sessionRole: connectingSceneSession.role)
    }

    /// The iPad's menu bar, and the shortcuts a held ⌘ lists (`KeyboardCommands`).
    override func buildMenu(with builder: UIMenuBuilder) {
        super.buildMenu(with: builder)
        KeyboardCommands.build(builder)
    }
}
