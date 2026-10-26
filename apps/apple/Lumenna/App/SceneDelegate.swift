import LumennaApp
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
            // The iPad's sidebar, as on the Mac; the iPhone's tabs.
            window.rootViewController = UIDevice.current.userInterfaceIdiom == .pad
                ? IPadRoot(core: core) : RootTabs(core: core)
        case let .failure(error):
            // Nothing works without the store, so say why plainly rather than showing an
            // empty list that looks like there is nothing to do.
            window.rootViewController = FailureViewController(
                message: LumennaApp.storeUnopened(reason: error.sentence)
            )
        }
        self.window = window
        window.makeKeyAndVisible()
        // What the app says, shown for a moment, for a person who reads rather than listens.
        SaidBanner.install(on: window)
        // The app icon's quick actions: the core's launches.
        UIApplication.shared.shortcutItems = QuickActions.items
        // Launched by one of them.
        if let shortcut = connectionOptions.shortcutItem {
            DispatchQueue.main.async { [weak self] in _ = self?.perform(shortcut) }
        }
        // Opened by a widget, on one of the core's launches.
        if let url = connectionOptions.urlContexts.first?.url {
            DispatchQueue.main.async { [weak self] in self?.open(url) }
        }
        // A control's launch (`NewTaskControlIntent`), and any that came before the window did.
        DispatchQueue.main.async { [weak self] in
            Launches.shared.handle { self?.launch($0) }
        }
        core?.backUpIfDue(presentingFrom: window.rootViewController)
    }

    /// A widget tapped while the app was running.
    func scene(_ scene: UIScene, openURLContexts contexts: Set<UIOpenURLContext>) {
        if let url = contexts.first?.url { open(url) }
    }

    /// A launch the URL names (`LaunchURL`).
    private func open(_ url: URL) {
        guard let id = LaunchURL.id(in: url) else { return }
        Launches.shared.ask(id)
    }

    /// Launch `id`, which the app does, then the screen showing the place it left, once it is
    /// still, and quick add over it if the app opened its line (New Task).
    private func launch(_ id: String) {
        guard let core else { return }
        core.app.send(.launch(id))
        core.app.whenStill { [weak self] in
            guard let root = self?.window?.rootViewController else { return }
            (root as? PlaceFollowing)?.showAppPlace()
            if core.app.view.line != nil { (root as? CommandActions)?.newTask?() }
        }
    }

    /// The Home Screen's quick action, chosen while the app was running.
    func windowScene(
        _ windowScene: UIWindowScene,
        performActionFor shortcutItem: UIApplicationShortcutItem,
        completionHandler: @escaping (Bool) -> Void
    ) {
        completionHandler(perform(shortcutItem))
    }

    /// One of the app icon's quick actions (long press, or VoiceOver's actions rotor): the core's
    /// launch it names, taken where every way in from outside goes.
    private func perform(_ shortcut: UIApplicationShortcutItem) -> Bool {
        guard let id = QuickActions.launch(of: shortcut) else { return false }
        Launches.shared.ask(id)
        return true
    }

    func sceneWillEnterForeground(_ scene: UIScene) {
        core?.timeZoneMayHaveChanged()
        core?.backUpIfDue(presentingFrom: window?.rootViewController)
        // What fired past the window last handed over, then the window moved on.
        AppDelegate.reminders?.rearm(catchingUp: true)
        NotificationCenter.default.post(name: Core.changed, object: core)
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        // The app keeps this device in sync while it is in front.
        core?.app.inFront(true)
    }

    func sceneDidDisconnect(_ scene: UIScene) {
        // A launch asked for from now on waits for the next scene, not this one's window.
        Launches.shared.letGo()
    }

    func sceneDidEnterBackground(_ scene: UIScene) {
        // One last round before stopping, so what was just edited is sent now.
        if let core { BackgroundSync.leaving(core) }
    }
}

/// The app icon's quick actions: the core's launches (`Launches.shortcuts`), each handed back as
/// its launch, so what each opens is the core's.
enum QuickActions {
    /// What a quick action's type starts with; its launch's id follows.
    static let prefix = "dev.emassey0135.lumenna.launch."

    static var items: [UIApplicationShortcutItem] {
        Launches.shortcuts.map { shortcut in
            UIApplicationShortcutItem(
                type: prefix + shortcut.id, localizedTitle: shortcut.title, localizedSubtitle: nil,
                icon: UIApplicationShortcutIcon(systemImageName: Launches.symbol(shortcut.icon))
            )
        }
    }

    /// The launch a quick action asks for, if it is one of these.
    static func launch(of item: UIApplicationShortcutItem) -> String? {
        item.type.hasPrefix(prefix) ? String(item.type.dropFirst(prefix.count)) : nil
    }
}

/// A top level that shows the app's place when a launch from outside has moved it.
protocol PlaceFollowing: AnyObject {
    func showAppPlace()
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
