import BackgroundTasks
import UIKit

/// Syncing while the app is not in front, within what iOS allows, and re-arming the reminders
/// the system fires after each round (and, after a refresh, writing the widgets' glance).
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
    static let identifier = "dev.emassey0135.lumenna.sync"

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

    /// Leaving the front: syncs once in the time iOS grants, then the app lets go of the
    /// endpoint — unless it came back to the front meanwhile, where it keeps syncing.
    ///
    /// The task is held until the app is still, too: the store is in the App Group's container,
    /// and iOS ends a process suspended while it holds a lock on a file there (`0xdead10cc`), as
    /// a write just made, or a tick, would.
    static func leaving(_ core: Core) {
        var task = UIBackgroundTaskIdentifier.invalid
        let end = {
            guard task != .invalid else { return }
            UIApplication.shared.endBackgroundTask(task)
            task = .invalid
        }
        let stop = {
            guard UIApplication.shared.applicationState != .active else { return }
            core.app.inFront(false)
        }
        task = UIApplication.shared.beginBackgroundTask(withName: "Sync on leaving") {
            // Out of time: whatever the round had not sent goes at the next one.
            stop()
            end()
        }
        // The round runs off the main thread, on the app's own sync service; then it stops.
        let device = core.app.device
        DispatchQueue.global(qos: .utility).async {
            _ = try? device.syncNow()
            DispatchQueue.main.async {
                let finish = {
                    stop()
                    core.app.whenStill { end() }
                }
                // What the round brought in may fire before the app is next opened.
                guard let reminders = AppDelegate.reminders else { return finish() }
                reminders.rearm(then: finish)
            }
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
        // Finished once, on the main thread, where the round's end comes too: the expiration
        // handler is called on a queue of the system's.
        var finished = false
        let finish = { (success: Bool) in
            guard !finished else { return }
            finished = true
            task.setTaskCompleted(success: success)
        }
        task.expirationHandler = { DispatchQueue.main.async { finish(false) } }
        // A round of its own, on an endpoint opened for it: the app is not in front.
        let device = core.app.device
        DispatchQueue.global(qos: .utility).async {
            let synced = (try? device.syncNow()) != nil
            DispatchQueue.main.async {
                if synced {
                    core.app.tick()
                    NotificationCenter.default.post(name: Core.changed, object: core)
                }
                // Re-armed, and the widgets' glance written, before the task ends, whatever the
                // round did: a refresh is also what carries the two-day window forward while the
                // app is not opened.
                // Once the app is still, so nothing holds the store as the task ends.
                let done = { core.app.whenStill { finish(synced) } }
                let glanced = {
                    guard let glance = core.glance else { return done() }
                    glance.write { done() }
                }
                let reminders = AppDelegate.reminders
                guard let reminders else { return glanced() }
                reminders.rearm(catchingUp: true) { glanced() }
            }
        }
    }
}
