import LumennaApp
import SwiftUI

/// The day as lived, the app's day (`ViewModel.day`): its heading, then each block with the
/// sittings in it, the free time between, where now falls, and what is cancelled for the day,
/// each as the core words its line. Tapping a row offers its actions, the app's, as the
/// phone's swipe actions do: a watch's row has room for few. A block with sittings folds, by
/// its own Expand or Collapse. Going to another day, and a new block, are buttons at the foot.
struct DayView: View {
    @EnvironmentObject private var core: Core
    @EnvironmentObject private var asker: WatchAsker
    @StateObject private var model: DayModel
    @AccessibilityFocusState private var spoken: String?

    init(core: Core) {
        _model = StateObject(wrappedValue: DayModel(core: core))
    }

    var body: some View {
        ScrollViewReader { scroller in
            List {
                if !model.heading.isEmpty { heading }
                ForEach(model.shown) { row in
                    rowView(row.item, line: row.line)
                        .accessibilityFocused($spoken, equals: row.id)
                }
                Section {
                    Button("Add Block") { core.app.send(.block(.new(start: nil, minutes: nil))) }
                    Button("Previous Day") { model.move(.previous) }
                    if core.app.view.day?.today == false { Button("Today") { model.move(.now) } }
                    Button("Next Day") { model.move(.next) }
                    // The app's own question, asked with the watch's picker (`WatchAsker`).
                    Button("Go to Day") { model.move(.ask) }
                }
            }
            .navigationTitle(model.title)
            // Today opening on now, unless the app shows the day already.
            .onAppear { model.appear() }
            .onChange(of: model.landed?.count) { _, _ in
                // The app moved the row in hand: VoiceOver goes there, once the row is drawn.
                guard let key = model.landed?.key else { return }
                scroller.scrollTo(key, anchor: .center)
                DispatchQueue.main.async { spoken = key }
            }
            .onChange(of: spoken) { _, key in
                if let key { model.moved(to: key) }
            }
        }
    }

    /// The day's heading, as every app says it: "<day>. <summary>". Tapped, it offers the
    /// day's own actions, the app's, as a row offers its own.
    @ViewBuilder private var heading: some View {
        if model.actions.isEmpty {
            Text(model.heading).font(.footnote)
        } else {
            Button {
                // In the app's order, as every row's are offered.
                asker.show(ChoicePrompt(title: model.title, choices: model.actions.map { action in
                    (action.title, { [model] in model.perform(action) })
                }))
            } label: { Text(model.heading).font(.footnote) }
                .accessibilityAddTraits(.isHeader)
                .accessibilityIdentifier("day.heading")
        }
    }

    /// A row: its whole line shown; said as every Apple app's row is, the title as its name and
    /// the rest of the line, its folding and its level as its value.
    @ViewBuilder
    private func rowView(_ item: LumennaApp.DayItem, line: LumennaApp.Line?) -> some View {
        let text = line?.text ?? ""
        let title = line?.title ?? text
        let value = AppModel.spoken(detail: line?.detail, expanded: item.expanded, level: item.level)
        switch item.kind {
        case .summary:
            EmptyView()
        case .now:
            Text(text).font(.headline)
                .accessibilityLabel(title)
                .accessibilityValue(value)
        case .sitting:
            Button { offer(item, line: line) } label: { Text(text) }
                .accessibilityLabel(title)
                .accessibilityValue(value)
                .padding(.leading, 8)
        case .block:
            Button { offer(item, line: line) } label: {
                HStack {
                    SwatchMark(swatch: item.block?.swatch)
                    Text(text)
                }
            }
                .accessibilityAddTraits(.isHeader)
                .accessibilityLabel(title)
                .accessibilityValue(value)
        case .free, .cancelled:
            Button { offer(item, line: line) } label: { Text(text) }
                .accessibilityLabel(title)
                .accessibilityValue(value)
        }
    }

    /// A row's actions, offered when it is tapped: a sitting's Edit Task Details opens its task.
    private func offer(_ item: LumennaApp.DayItem, line: LumennaApp.Line?) {
        asker.offer(line?.title ?? "", item.offered, on: item.key)
    }
}
