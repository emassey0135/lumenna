import AppIntents
import Foundation
import LumennaApp

/// The core's launches (`launchShortcuts()`, `records::launch`) asked for from outside the app — a
/// widget's or complication's URL (`LaunchURL`), a control's press — handed to whoever shows the
/// app: its scene, its window, the watch's root. A launch asked for before that is ready (the app
/// started by it) waits for it, so none is lost to the order a cold start runs in.
@MainActor
final class Launches {
    static let shared = Launches()

    private var pending: [String] = []
    private var handler: ((String) -> Void)?

    /// Asks for launch `id`: now, or once something shows the app.
    func ask(_ id: String) {
        if let handler { handler(id) } else { pending.append(id) }
    }

    /// What does each launch from now on, given every one that waited.
    func handle(_ handler: @escaping (String) -> Void) {
        self.handler = handler
        let waiting = pending
        pending = []
        waiting.forEach(handler)
    }

    /// What showed the app has gone (iOS disconnected its scene): a launch waits again for
    /// whatever shows it next, rather than going to a window no longer there.
    func letGo() {
        handler = nil
    }

    /// The core's launches, as the platform offers them by themselves: the app icon's quick
    /// actions, the Dock's menu. Their order and words are the core's.
    nonisolated static var shortcuts: [LumennaApp.LaunchShortcut] { LumennaApp.launchShortcuts() }

    /// The system symbol for one of the core's generic icon names.
    nonisolated static func symbol(_ icon: String) -> String {
        switch icon {
        case "add": "plus"
        case "today": "calendar"
        case "next": "arrow.forward.circle"
        default: "circle"
        }
    }
}

/// New Task, Control Center's button (`controls()`, whose id is the launch shortcut's): opens the
/// app on the core's `new-task` launch, quick add. Compiled into the app as well as the widgets,
/// since an intent that opens its app runs in the app: a control cannot open an app by a URL
/// scheme of its own (Apple's engineers: only a universal link, which this app has none of).
struct NewTaskControlIntent: AppIntent {
    static let title: LocalizedStringResource = "New Task"
    static let openAppWhenRun = true
    /// The control's alone: Shortcuts and Siri offer Add a Task.
    static let isDiscoverable = false

    /// The launch it opens.
    static let launch = "new-task"

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult {
        Launches.shared.ask(Self.launch)
        return .result()
    }
}
