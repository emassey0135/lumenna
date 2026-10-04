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
        do {
            let core = try Core()
            self.core = core
            window.rootViewController = UINavigationController(
                rootViewController: TaskListViewController(core: core)
            )
        } catch {
            // Nothing works without the store, so say why plainly rather than showing an
            // empty list that looks like there is nothing to do.
            window.rootViewController = FailureViewController(
                message: "Lumenna could not open its store. \(error.sentence)"
            )
        }
        self.window = window
        window.makeKeyAndVisible()
        core?.backUpIfDue(presentingFrom: window.rootViewController)
    }

    func sceneWillEnterForeground(_ scene: UIScene) {
        core?.timeZoneMayHaveChanged()
        core?.backUpIfDue(presentingFrom: window?.rootViewController)
        NotificationCenter.default.post(name: Core.changed, object: core)
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
