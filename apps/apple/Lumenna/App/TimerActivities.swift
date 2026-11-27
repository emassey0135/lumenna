import ActivityKit
import Foundation
import LumennaApp

/// The running timer's Live Activity (`TimerActivityAttributes`), kept to the core's
/// `TimerControl`: started when a timer runs, written again when what runs changes, ended when
/// none does. Looked at as the app launches, after every change to the store (`Core.changed`:
/// the app's own, a sync's, the Timer control's press in Control Center, which the app's look at
/// the heads finds), and after the activity's own Pause and Stop. One per process.
final class TimerActivities {
    static var shared: TimerActivities?

    private let device: LumennaApp.Device
    private var observer: NSObjectProtocol?
    /// Where the control is read: off the main thread, as the glance is.
    private let queue = DispatchQueue(label: "dev.emassey0135.lumenna.timer-activity", qos: .utility)

    init(device: LumennaApp.Device) {
        self.device = device
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Whether this process shows Live Activities: never under a UI test's store, whose timers
    /// would leave one on the simulator's Lock Screen.
    static var shows: Bool {
        ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] == nil
    }

    /// Looks again after every change to the store.
    func follow() {
        observer = NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.refresh()
        }
    }

    /// The activity as the timer is now.
    func refresh() {
        guard Self.shows else { return }
        let device = device
        queue.async {
            let state = (try? device.timerControl()).flatMap { TimerActivityState.of($0) }
            Task { await TimerActivities.show(state) }
        }
    }

    /// Shows `state`: one activity, started where none shows, written again where it differs,
    /// every one ended where no timer runs.
    @MainActor
    static func show(_ state: TimerActivityAttributes.ContentState?) async {
        let shown = Activity<TimerActivityAttributes>.activities
        guard let state else {
            for activity in shown { await activity.end(nil, dismissalPolicy: .immediate) }
            return
        }
        // Stale once its planned length runs out, which is when it shows the core's words that
        // time is up and counts up instead.
        let content = ActivityContent(state: state, staleDate: TimerActivityState.countsDown(state, at: Date())?.upperBound)
        if let first = shown.first {
            if first.content.state != state { await first.update(content) }
            for other in shown.dropFirst() { await other.end(nil, dismissalPolicy: .immediate) }
            return
        }
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        _ = try? Activity.request(attributes: TimerActivityAttributes(), content: content, pushType: nil)
    }
}
