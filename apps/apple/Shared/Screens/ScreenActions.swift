import Foundation
import LumennaApp
import SwiftUI

/// What a screen offers of its own beside its rows, the core's (`ViewModel.screenActions`): New
/// Task, the filter, the day's moves, Undo and Redo, in the core's order and words, already in
/// Title Case. Every Apple app draws them from here, its own way (the iPhone's and iPad's
/// navigation bar and the day's buttons, the Mac's row of buttons over its pane, the watch's
/// buttons at the foot of its list), and runs them here (`AppModel.perform`): one that is the
/// shell's own (`shell`: the filter field) by the app itself, every other as the core's key.
extension LumennaApp.ScreenAction {
    /// Its name on a button: the core's title, with "…" after it on the Mac where it asks
    /// something first, as the Mac marks a button and a menu item; the iPhone, iPad and watch
    /// mark none, as their buttons and their rows' menus do not.
    var named: String {
        #if os(macOS)
        asks ? title + "…" : title
        #else
        title
        #endif
    }
}

extension AppModel {
    /// Runs one of the screen's own actions: one that is the shell's own through `shell` (by its
    /// id: "filter" puts the filter field in hand), every other as the core's key (`Key(id)`),
    /// whose question the window's `AppQuestions` asks and whose form `AppSheets` opens. New Task
    /// opens the app's quick-add line: `lineOpened` then brings the platform's quick add up over
    /// it, once the app is still. One not enabled does nothing.
    func perform(
        _ action: LumennaApp.ScreenAction, shell: (String) -> Void = { _ in }, lineOpened: @escaping () -> Void = {}
    ) {
        guard action.enabled else { return }
        if action.shell { return shell(action.id) }
        let closed = view.line == nil
        key(action.id)
        guard closed else { return }
        whenStill { [weak self] in
            if self?.view.line != nil { lineOpened() }
        }
    }
}

/// One of the screen's own actions as a button: its name, dimmed while it does nothing.
struct ScreenActionButton: View {
    let action: LumennaApp.ScreenAction
    let run: (LumennaApp.ScreenAction) -> Void

    var body: some View {
        Button(action.named) { run(action) }
            .disabled(!action.enabled)
    }
}

/// The screen's own actions as a row of buttons, in the core's order, with `trailing` after them
/// (the day's Day Actions): side by side while they fit, then on two lines, and one above another
/// only once the text is too large for that. `spread` sets them across the width, as the iPhone's
/// day has them; else they keep together at the leading edge, as the Mac's do.
struct ScreenActionRow<Trailing: View>: View {
    let actions: [LumennaApp.ScreenAction]
    var spread = false
    let run: (LumennaApp.ScreenAction) -> Void
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        let half = (actions.count + 1) / 2
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) { buttons(actions[...], spread: spread); trailing() }
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) { buttons(actions[..<half], spread: spread) }
                HStack(spacing: 8) { buttons(actions[half...], spread: spread); trailing() }
            }
            // One above another: a spacer between them would stretch the column.
            VStack(alignment: .leading, spacing: 8) { buttons(actions[...], spread: false); trailing() }
        }
    }

    @ViewBuilder private func buttons(_ shown: ArraySlice<LumennaApp.ScreenAction>, spread: Bool) -> some View {
        ForEach(Array(shown.enumerated()), id: \.element.id) { index, action in
            if spread, index > 0 { Spacer(minLength: 0) }
            ScreenActionButton(action: action, run: run)
        }
    }
}

extension ScreenActionRow where Trailing == EmptyView {
    init(actions: [LumennaApp.ScreenAction], spread: Bool = false, run: @escaping (LumennaApp.ScreenAction) -> Void) {
        self.init(actions: actions, spread: spread, run: run) { EmptyView() }
    }
}
