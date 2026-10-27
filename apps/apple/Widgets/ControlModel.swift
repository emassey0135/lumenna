import Foundation
import LumennaApp
import WidgetKit

// What the controls take from the core, apart from how they are drawn, so the tests
// (`LumennaTests`, which compile this file) check it: which controls there are and their words,
// what the timer's control shows of the core's `TimerControl`, and the watch's New Task
// complication. Nothing here words or decides what a control says or what a press starts.

enum Controls {
    /// The timer's control: its kind is the core's id.
    static let timer = "timer"
    /// New Task's control: its kind is the core's id, its launch shortcut's.
    static let newTask = "new-task"

    /// The core's control `id`: its name and description in the gallery.
    static func control(_ id: String) -> LumennaApp.Control? {
        LumennaApp.controls().first { $0.id == id }
    }

    /// The name a control is shown by: the core's, or its id should the core drop it.
    static func name(_ id: String) -> String {
        control(id)?.name ?? id
    }

    /// The timer's control where nothing has said what it shows (the Mac's direct download before
    /// its app has written it): off, saying why in the core's words, pressed to no effect.
    static func unknown(reason: String = "") -> LumennaApp.TimerControl {
        LumennaApp.TimerControl(
            running: false, detail: LumennaApp.storeUnopened(reason: reason), action: "", actionSentence: "", enabled: false
        )
    }
}

/// The timer's control as Control Center draws it, from the core's `TimerControl` alone.
struct TimerControlState: Equatable {
    /// The toggle's state: whether a timer runs.
    let isOn: Bool
    /// Its title: the core's name.
    let title: String
    /// What it says beneath: the task whose timer runs, what a press would start, or why nothing
    /// would.
    let value: String
    /// What a press does, shown as it is pressed.
    let action: String
    /// Whether a press does anything.
    let enabled: Bool

    init(_ control: LumennaApp.TimerControl, name: String = Controls.name(Controls.timer)) {
        isOn = control.running
        title = name
        value = control.detail
        action = control.action
        enabled = control.enabled
    }

    /// What VoiceOver hears: the name, what a press does, then what it does it to, joined by the
    /// core (`timerDescribed`). A control draws one label whether or not the device is locked,
    /// so it is the whole one.
    var spoken: String {
        LumennaApp.timerDescribed(name: title, action: action, detail: value, locked: false)
    }
}

/// The watch's New Task complication: the core's launch shortcut, not a glance (so not one of
/// `glanceWidgets()`), opening the app on it as a widget does (`LaunchURL.of(launch)`).
enum LaunchComplication {
    /// Its WidgetKit kind: the launch's own, apart from the controls' and the glance widgets'.
    static let kind = "launch.new-task"
    /// The launch it opens.
    static let launch = "new-task"
    /// Its symbol: the core's icon for it is "add".
    static let symbol = "plus"

    /// The core's launch shortcut it stands for; `nil` should the core drop it, which
    /// `WidgetTests` finds.
    static var shortcut: LumennaApp.LaunchShortcut? {
        LumennaApp.launchShortcuts().first { $0.id == launch }
    }

    /// Its name in the gallery and to VoiceOver: the shortcut's title.
    static var name: String { shortcut?.title ?? launch }

    /// What it says it does in the gallery.
    static var description: String { shortcut?.description ?? "" }

    #if os(watchOS)
    /// Where it goes on a watch face: a circle, a corner with its name along the edge, and a line.
    static let families: [WidgetFamily] = [.accessoryCircular, .accessoryCorner, .accessoryInline]
    #endif
}
