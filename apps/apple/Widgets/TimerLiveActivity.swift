#if os(iOS)
import ActivityKit
import AppIntents
import SwiftUI
import WidgetKit

/// The running timer on the Lock Screen and in the Dynamic Island, as the app writes it
/// (`TimerActivities`, from the core's `TimerControl.timer`): the task and its block, counting
/// down to the end of its planned length where it has one, else up from what it logged, and the
/// core's Pause Timer and Stop Timer. A paired Apple Watch shows it in its Smart Stack, as watchOS
/// takes it from the phone. Its text counts by itself; the core's words that time is up are shown
/// once it has run out, the one moment written ahead (`upWords`).
struct TimerLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: TimerActivityAttributes.self) { context in
            TimerActivityLockScreen(state: context.state, stale: context.isStale)
                .activityBackgroundTint(nil)
                .widgetURL(LaunchURL.of("today"))
        } dynamicIsland: { context in
            let state = context.state
            let stale = context.isStale
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    TimerActivityTask(state: state)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    TimerActivityCount(state: state, stale: stale)
                        .font(.title2.monospacedDigit())
                }
                DynamicIslandExpandedRegion(.bottom) {
                    TimerActivityButtons(state: state)
                }
            } compactLeading: {
                Image(systemName: "timer")
                    .accessibilityLabel(state.task)
            } compactTrailing: {
                TimerActivityCount(state: state, stale: stale)
                    .monospacedDigit()
                    .frame(maxWidth: 56)
            } minimal: {
                TimerActivityCount(state: state, stale: stale)
                    .monospacedDigit()
                    .font(.caption2)
            }
            .widgetURL(LaunchURL.of("today"))
        }
    }
}

/// The Lock Screen's: the task and its count, the words that time is up once it is, then the two
/// buttons.
private struct TimerActivityLockScreen: View {
    let state: TimerActivityAttributes.ContentState
    let stale: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                TimerActivityTask(state: state)
                Spacer(minLength: 8)
                TimerActivityCount(state: state, stale: stale)
                    .font(.title.monospacedDigit())
                    .widgetAccentable()
            }
            TimerActivityButtons(state: state)
        }
        .padding()
    }
}

/// The task, and beneath it the block it is in: one element to VoiceOver.
private struct TimerActivityTask: View {
    let state: TimerActivityAttributes.ContentState

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(state.task)
                .font(.headline)
                .lineLimit(2)
            if !state.block.isEmpty {
                Text(state.block)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// The count, which runs by itself: down to the end of the planned length while some is left,
/// else up from everything logged, with the core's words that time is up beside it.
private struct TimerActivityCount: View {
    let state: TimerActivityAttributes.ContentState
    let stale: Bool

    var body: some View {
        if !stale, let down = TimerActivityState.countsDown(state, at: Date()) {
            Text(timerInterval: down, countsDown: true)
                .multilineTextAlignment(.trailing)
        } else {
            VStack(alignment: .trailing, spacing: 0) {
                Text(state.countingFrom, style: .timer)
                    .multilineTextAlignment(.trailing)
                if state.runsOutAt != nil, let up = state.upWords {
                    Text(up)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .accessibilityElement(children: .combine)
        }
    }
}

/// The core's Pause Timer and Stop Timer, run on the sitting shown, in the app.
private struct TimerActivityButtons: View {
    let state: TimerActivityAttributes.ContentState

    var body: some View {
        HStack(spacing: 12) {
            Button(intent: PauseTimerActivityIntent(sitting: state.sitting)) {
                Label(state.pause, systemImage: "pause.fill")
                    .frame(maxWidth: .infinity)
            }
            .accessibilityLabel(state.pause)
            Button(intent: StopTimerActivityIntent(sitting: state.sitting)) {
                Label(state.stop, systemImage: "stop.fill")
                    .frame(maxWidth: .infinity)
            }
            .accessibilityLabel(state.stop)
        }
        .buttonStyle(.bordered)
        .tint(Color.lumennaTint)
        .font(.subheadline.weight(.semibold))
    }
}
#endif
