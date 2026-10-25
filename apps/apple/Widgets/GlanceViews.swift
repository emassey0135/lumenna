import LumennaApp
import SwiftUI
import WidgetKit

/// A widget's face: the core's lines for the moment shown, as many of the widget's parts as fit,
/// in their order. VoiceOver hears the whole glance as one element, the core's `spoken`, with a
/// running timer as its value; only Today's Mark Done buttons and its New Task are elements of
/// their own.
struct GlanceView: View {
    let entry: GlanceTimelineEntry
    let parts: [LumennaApp.GlancePart]
    @Environment(\.widgetFamily) private var family

    var body: some View {
        Group {
            switch family {
            #if os(iOS) || os(watchOS)
            case .accessoryInline:
                Text(glance.headline.text)
                    .glanceElement(entry)
            case .accessoryCircular:
                circular.glanceElement(entry)
            case .accessoryRectangular:
                VStack(alignment: .leading, spacing: 0) {
                    Headline(entry: entry, compact: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .glanceElement(entry)
            #endif
            #if os(watchOS)
            case .accessoryCorner:
                corner.glanceElement(entry)
            #endif
            default:
                ViewThatFits(in: .vertical) {
                    ForEach(GlanceWidgets.fitting(parts), id: \.self) { shown in
                        Parts(entry: entry, parts: shown)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
        }
        .containerBackground(for: .widget) { Color.widgetBackground }
        // Tapped anywhere but a button: the app, on Today, the core's launch.
        .widgetURL(LaunchURL.of("today"))
    }

    private var glance: LumennaApp.GlanceEntry { entry.glance }

    #if os(iOS) || os(watchOS)
    /// The timer counting, else the block under way filling, else what is happening now.
    @ViewBuilder private var circular: some View {
        ZStack {
            AccessoryWidgetBackground()
            if let start = entry.timerStart {
                Text(start, style: .timer)
                    .font(.caption.monospacedDigit())
                    .multilineTextAlignment(.center)
                    .minimumScaleFactor(0.5)
                    .padding(4)
            } else if let block = entry.blockUnderWay {
                ProgressView(timerInterval: block, countsDown: false) {
                    Text(glance.headline.title)
                } currentValueLabel: {
                    Text(glance.headline.title)
                }
                .progressViewStyle(.circular)
            } else {
                Text(glance.headline.title)
                    .font(.caption2)
                    .multilineTextAlignment(.center)
                    .lineLimit(3)
                    .minimumScaleFactor(0.6)
                    .padding(4)
            }
        }
    }
    #endif

    #if os(watchOS)
    /// The timer or the block's progress in the corner, the headline along its curve.
    @ViewBuilder private var corner: some View {
        Group {
            if let start = entry.timerStart {
                Text(start, style: .timer).font(.caption.monospacedDigit()).minimumScaleFactor(0.5)
            } else if let block = entry.blockUnderWay {
                ProgressView(timerInterval: block, countsDown: false).progressViewStyle(.circular)
            } else {
                Image(systemName: "calendar.day.timeline.left")
            }
        }
        .widgetLabel { Text(glance.headline.text) }
    }
    #endif
}

/// The parts of the glance a size shows, each the core's line.
private struct Parts: View {
    let entry: GlanceTimelineEntry
    let parts: [LumennaApp.GlancePart]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(parts, id: \.self) { part in
                switch part {
                case .headline:
                    // Every widget's first part (the core's test), so it speaks for the widget.
                    VStack(alignment: .leading, spacing: 2) { Headline(entry: entry, compact: false) }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .glanceElement(entry)
                case .next:
                    if let next = entry.glance.next {
                        Text(next.text).font(.subheadline).lineLimit(2).accessibilityHidden(true)
                    }
                case .due:
                    Text(entry.glance.due.text)
                        .font(.subheadline)
                        .foregroundStyle(entry.glance.moment.overdue > 0 ? Color.warningLabel : Color.primary)
                        .lineLimit(1)
                        .accessibilityHidden(true)
                case .tasks:
                    Tasks(entry: entry)
                case .newTask:
                    NewTask()
                }
            }
        }
    }
}

/// What is happening now: the headline's title and the rest of its line, and the timer
/// counting by itself where one runs.
private struct Headline: View {
    let entry: GlanceTimelineEntry
    let compact: Bool

    var body: some View {
        let headline = entry.glance.headline
        Text(headline.title)
            .font(.headline)
            .lineLimit(2)
            .widgetAccentable()
        if !headline.detail.isEmpty {
            Text(headline.detail)
                .font(compact ? .caption : .subheadline)
                .foregroundStyle(Color.quietLabel)
                .lineLimit(2)
        }
        if let start = entry.timerStart {
            Text(start, style: .timer)
                .font(compact ? .body.monospacedDigit() : .title2.monospacedDigit())
                .widgetAccentable()
        }
    }
}

/// New Task: the core's launch shortcut, its name and the app opened on it (quick add), a link
/// of its own beside the widget's own tap, which opens the day.
private struct NewTask: View {
    var body: some View {
        #if !os(watchOS)
        if let shortcut = LumennaApp.launchShortcuts().first(where: { $0.id == "new-task" }) {
            Link(destination: LaunchURL.of(shortcut.id)) {
                Label(shortcut.title, systemImage: "plus")
                    .font(.subheadline)
                    .widgetAccentable()
            }
        }
        #endif
    }
}

/// The tasks due the glance names, each the core's line with the core's Mark Done beside it.
private struct Tasks: View {
    let entry: GlanceTimelineEntry

    var body: some View {
        #if !os(watchOS)
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(zip(entry.glance.moment.tasks, entry.glance.tasks).enumerated()), id: \.offset) { _, pair in
                let (task, line) = pair
                HStack(alignment: .center, spacing: 8) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(line.title).font(.subheadline).lineLimit(1)
                        if !line.detail.isEmpty {
                            Text(line.detail).font(.caption).foregroundStyle(task.overdue ? Color.warningLabel : Color.quietLabel).lineLimit(1)
                        }
                    }
                    // Its button says which task it marks.
                    .accessibilityHidden(true)
                    Spacer(minLength: 0)
                    MarkDone(entry: entry, task: task, line: line)
                }
            }
        }
        #endif
    }
}

#if !os(watchOS)
/// A task's Mark Done: the core's Complete a Task (`CompleteTaskIntent`), run on the store in the
/// group, or in the Mac's direct download handed to the app (`apps/apple/CLAUDE.md`, "The
/// widgets").
private struct MarkDone: View {
    let entry: GlanceTimelineEntry
    let task: LumennaApp.GlanceTask
    let line: LumennaApp.Line

    var body: some View {
        Button(intent: CompleteTaskIntent(task: TaskEntity(glanced: task))) {
            Text(entry.glance.markDone).font(.caption.weight(.semibold))
        }
        .buttonStyle(.bordered)
        .tint(Color.lumennaTint)
        .accessibilityLabel(GlanceWidgets.markDone(entry.glance, line))
    }
}
#endif

private extension View {
    /// One element for the whole glance, the core's sentence, a running timer as its value.
    @ViewBuilder func glanceElement(_ entry: GlanceTimelineEntry) -> some View {
        let element = accessibilityElement(children: .ignore).accessibilityLabel(entry.glance.spoken)
        if let start = entry.timerStart {
            element.accessibilityValue(Text(start, style: .timer))
        } else {
            element
        }
    }
}

private extension Color {
    /// The system's own background, which the text colours clear contrast on.
    static var widgetBackground: Color {
        #if os(iOS)
        Color(uiColor: .systemBackground)
        #elseif os(macOS)
        Color(nsColor: .windowBackgroundColor)
        #else
        Color.black
        #endif
    }
}
