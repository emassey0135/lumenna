import AppIntents
import Foundation
import LumennaApp
import SwiftUI
import WidgetKit

// Control Center's controls, the core's (`controls()`): the timer, a toggle showing the core's
// `TimerControl` whose press is `Device.toggleTimer`, and New Task, a button opening the app on
// the core's launch. iOS 18, macOS 26 and watchOS 26 are the first with controls.

/// The timer: on while a timer runs, saying what runs or what a press would start.
@available(iOS 18.0, macOS 26.0, watchOS 26.0, *)
struct TimerControlWidget: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: Controls.timer, provider: TimerControlProvider()) { control in
            let state = TimerControlState(control)
            ControlWidgetToggle(isOn: state.isOn, action: TimerControlIntent()) {
                Label(state.title, systemImage: "timer")
                    .accessibilityLabel(state.spoken)
            } valueLabel: { _ in
                Text(state.value)
                    .controlWidgetActionHint(Text(state.action))
            }
            .disabled(!state.enabled)
        }
        .displayName("\(Controls.name(Controls.timer))")
        .description("\(Controls.control(Controls.timer)?.description ?? "")")
    }
}

/// What the timer's control shows, read afresh each time the system asks.
@available(iOS 18.0, macOS 26.0, watchOS 26.0, *)
struct TimerControlProvider: ControlValueProvider {
    var previewValue: LumennaApp.TimerControl { Controls.unknown() }

    func currentValue() async throws -> LumennaApp.TimerControl {
        TimerControlSource.current()
    }
}

/// A press of the timer's control. The value the system hands it is ignored: a press is always the
/// core's toggle, which stops the timer running or starts on what to work on now.
@available(iOS 18.0, macOS 26.0, watchOS 26.0, *)
struct TimerControlIntent: SetValueIntent {
    static let title: LocalizedStringResource = "Timer"
    /// The control's alone: Shortcuts and Siri offer Start a Timer and Stop a Timer.
    static let isDiscoverable = false

    @Parameter(title: "Running")
    var value: Bool

    init() {}

    func perform() async throws -> some IntentResult {
        try TimerControlSource.press()
        return .result()
    }
}

/// New Task: opens the app on the core's launch, quick add (`NewTaskControlIntent`).
@available(iOS 18.0, macOS 26.0, watchOS 26.0, *)
struct NewTaskControlWidget: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: Controls.newTask) {
            ControlWidgetButton(action: NewTaskControlIntent()) {
                Label(Controls.name(Controls.newTask), systemImage: LaunchComplication.symbol)
            }
        }
        .displayName("\(Controls.name(Controls.newTask))")
        .description("\(Controls.control(Controls.newTask)?.description ?? "")")
    }
}

/// Where the timer's control finds the store, as the widgets find it (apps/apple/CLAUDE.md, "The
/// controls").
enum TimerControlSource {
    /// What the control shows now: the core's, from the store in the group, opened and closed
    /// again for it alone; in the Mac's direct download, whose store the sandbox keeps the
    /// control from, the file its app writes (`TimerControlFile`).
    static func current() -> LumennaApp.TimerControl {
        #if os(macOS) && !APP_STORE
        return TimerControlFile.read() ?? Controls.unknown()
        #else
        do {
            return try GlanceStore.visit(AppGroup.store) { try $0.timerControl() } ?? Controls.unknown()
        } catch {
            return Controls.unknown(reason: error.sentence)
        }
        #endif
    }

    /// The control pressed: the core's toggle, on the store in the group, then the glance written
    /// again while it is still open, since the app may not be running, and every widget and
    /// control told. In the Mac's direct download it is handed to the app (`WidgetRequests`).
    static func press() throws {
        #if os(macOS) && !APP_STORE
        guard let container = AppGroup.container else { throw CocoaError(.fileNoSuchFile) }
        try WidgetRequests.hand(.init(operation: WidgetRequests.toggleTimer, arguments: [:]), into: container)
        #else
        let pressed: Void? = try GlanceStore.visit(AppGroup.store) { device in
            _ = try device.toggleTimer()
            try? GlanceFile(device: device)?.writeNow()
        }
        guard pressed != nil else { throw CocoaError(.fileNoSuchFile) }
        #endif
        GlanceFile.reload()
    }
}
