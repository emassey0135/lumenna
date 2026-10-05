import UIKit

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private var core: Core?

    func scene(
        _ scene: UIScene,
        willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let scene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: scene)
        window.tintColor = .lumennaTint
        switch AppDelegate.core {
        case let .success(core):
            self.core = core
            window.rootViewController = RootTabs(core: core)
        case let .failure(error):
            // Nothing works without the store, so say why plainly rather than showing an
            // empty list that looks like there is nothing to do.
            window.rootViewController = FailureViewController(
                message: "Lumenna could not open its store. \(error.sentence)"
            )
        }
        self.window = window
        window.makeKeyAndVisible()
        // Launched by the Home Screen's New Task.
        if let shortcut = connectionOptions.shortcutItem {
            DispatchQueue.main.async { [weak self] in _ = self?.perform(shortcut) }
        }
        core?.backUpIfDue(presentingFrom: window.rootViewController)
    }

    /// The Home Screen's quick action, chosen while the app was running.
    func windowScene(
        _ windowScene: UIWindowScene,
        performActionFor shortcutItem: UIApplicationShortcutItem,
        completionHandler: @escaping (Bool) -> Void
    ) {
        completionHandler(perform(shortcutItem))
    }

    /// New Task, from the app icon's quick actions (long press, or VoiceOver's actions rotor):
    /// quick add, without first finding the Tasks tab.
    private func perform(_ shortcut: UIApplicationShortcutItem) -> Bool {
        guard shortcut.type == Self.newTask, let tabs = window?.rootViewController as? RootTabs else { return false }
        tabs.newTask()
        return true
    }

    /// Also under `UIApplicationShortcutItems` in Info.plist.
    static let newTask = "io.github.emassey0135.lumenna.new-task"

    func sceneWillEnterForeground(_ scene: UIScene) {
        core?.timeZoneMayHaveChanged()
        core?.backUpIfDue(presentingFrom: window?.rootViewController)
        NotificationCenter.default.post(name: Core.changed, object: core)
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        core?.startSyncing()
    }

    func sceneDidEnterBackground(_ scene: UIScene) {
        // One last round before stopping, so what was just edited is sent now.
        if let core { BackgroundSync.leaving(core) }
    }
}

/// What shows when the store cannot be opened.
private final class FailureViewController: UIViewController {
    private let message: String

    init(message: String) {
        self.message = message
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        let label = UILabel()
        label.text = message
        label.numberOfLines = 0
        label.font = .preferredFont(forTextStyle: .body)
        label.adjustsFontForContentSizeCategory = true
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: view.readableContentGuide.leadingAnchor),
            label.trailingAnchor.constraint(equalTo: view.readableContentGuide.trailingAnchor),
            label.centerYAnchor.constraint(equalTo: view.centerYAnchor),
        ])
    }
}
