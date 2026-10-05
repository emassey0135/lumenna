import BackgroundTasks
import UIKit

/// Syncing while the app is not in front (§8), within what iOS allows.
///
/// - **On leaving**, one round in the time iOS grants a backgrounded app to finish its work,
///   so what was just edited reaches the other devices now rather than at the next launch.
/// - **Now and then**, a background refresh: iOS decides when, from how the app is used, the
///   battery and the network — a few times a day, not on a schedule. One round each time.
///
/// A round reaches only the devices that are running: a Mac, a daemon, a phone that is open.
/// Two phones both in the background meet through those, or when either is opened.
enum BackgroundSync {
    /// Also listed under `BGTaskSchedulerPermittedIdentifiers` in Info.plist.
    static let identifier = "io.github.emassey0135.lumenna.sync"

    /// How soon to ask for the next refresh. iOS treats it as the earliest, not a promise.
    private static let interval: TimeInterval = 15 * 60

    static func register() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: identifier, using: nil) { task in
            guard let refresh = task as? BGAppRefreshTask else {
                task.setTaskCompleted(success: false)
                return
            }
            handle(refresh)
        }
    }

    /// Asks for a refresh some time after `interval`. Asking again replaces the last request.
    static func schedule() {
        let request = BGAppRefreshTaskRequest(identifier: identifier)
        request.earliestBeginDate = Date(timeIntervalSinceNow: interval)
        try? BGTaskScheduler.shared.submit(request)
    }

    /// Leaving the front: syncs once in the time iOS grants, then lets go of the endpoint.
    static func leaving(_ core: Core) {
        var task = UIBackgroundTaskIdentifier.invalid
        let end = {
            guard task != .invalid else { return }
            UIApplication.shared.endBackgroundTask(task)
            task = .invalid
        }
        task = UIApplication.shared.beginBackgroundTask(withName: "Sync on leaving") {
            // Out of time: whatever the round had not sent goes at the next one.
            core.stopSyncing()
            end()
        }
        core.syncNow { _ in
            core.stopSyncing(waiting: true)
            end()
        }
        schedule()
    }

    private static func handle(_ task: BGAppRefreshTask) {
        // The next one is asked for first, so a refresh that fails does not end them.
        schedule()
        guard case let .success(core) = AppDelegate.core else {
            task.setTaskCompleted(success: false)
            return
        }
        var finished = false
        let finish = { (success: Bool) in
            guard !finished else { return }
            finished = true
            task.setTaskCompleted(success: success)
        }
        task.expirationHandler = { finish(false) }
        core.syncNow { result in
            if case .success = result {
                NotificationCenter.default.post(name: Core.changed, object: core)
            }
            finish((try? result.get()) != nil)
        }
    }
}
