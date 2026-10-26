import Foundation
import LumennaApp

/// The day as the app has it (`ViewModel.day`), for a SwiftUI day: every row with its line,
/// those folded away marked, the heading, the day's own actions, the row in hand, and each time
/// the app moves it.
///
/// The day shown, moving between days, what is in hand and folded, and every row's actions and
/// their forms are the app's; this follows it and reports the person's own moves
/// (`moved(to:)`). The day opens as its screen comes to the front, unless the app shows it
/// already, today opening on now.
final class DayModel: ObservableObject {
    /// A row as the day draws it: the app's, and its line in this device's words.
    struct Row: Identifiable, Equatable {
        let item: LumennaApp.DayItem
        let line: LumennaApp.Line?
        var id: String { item.key }
    }

    let core: Core
    /// Every row but the heading, those folded away included (`hidden`): a tree draws them
    /// inside their collapsed block, a flat list leaves them out.
    @Published private(set) var rows: [Row] = []
    /// The day's heading, as every app says it: "<day>. <summary>".
    @Published private(set) var heading = ""
    /// "Today", "Tomorrow", or the date: what the screen is called.
    @Published private(set) var title = "Today"
    /// The day's own actions (`DayView.actions`): Plan the Day, Replan from Now, End the Day.
    @Published private(set) var actions: [LumennaApp.Offered] = []
    /// The row in hand, the app's.
    @Published private(set) var focus: String?
    /// The row the app moved focus to, each time it moves it: a new `count` is a new move.
    @Published private(set) var landed: (key: String, count: Int)?
    /// The day shown, ISO.
    @Published private(set) var date: String?

    /// The app's landing count at the last look.
    private var landing: UInt32?
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Whether the app is showing the day: only then is its day this screen's.
    var here: Bool { core.app.view.place == .today }

    /// Makes the day the app's place, unless it is already. Coming back to it finds the row in
    /// hand it was left with: the app keeps it. Opening is the screen coming to the front, which
    /// VoiceOver says itself: no landing.
    func appear() {
        // As the app holds it now, ahead of any turn on its way: opened again after a reminder's
        // block was shown (`ShowBlock`), the day would go back to today.
        if core.app.current.view.place != .today {
            core.app.send(.open(.today))
            landing = nil
        }
        draw()
    }

    /// The app's day, while it is shown.
    private func draw() {
        let view = core.app.view
        guard view.place == .today, let day = view.day else { return }
        let lines = core.app.lines
        let worded = day.items.enumerated().map { index, item in
            Row(item: item, line: index < lines.day.count ? lines.day[index] : nil)
        }
        let heading = worded.first { $0.item.kind == .summary }?.line?.text ?? lines.dayTitle
        let rows = worded.filter { $0.item.kind != .summary }
        if rows != self.rows { self.rows = rows }
        if heading != self.heading { self.heading = heading }
        if title != lines.dayTitle { title = lines.dayTitle }
        if actions != day.offered { actions = day.offered }
        if date != day.date { date = day.date }
        if focus != view.focus { focus = view.focus }
        let moved = landing.map { view.landing > $0 } ?? false
        landing = view.landing
        if moved, let key = view.focus { landed = (key, (landed?.count ?? 0) + 1) }
    }

    /// The rows a flat list shows: those not folded away.
    var shown: [Row] { rows.filter { !$0.item.hidden } }

    /// The person moved to a row: the app's row in hand, a sitting's task shown beside it.
    func moved(to key: String) {
        guard here, key != core.app.view.focus else { return }
        core.app.send(.focus(key))
        // In hand here at once: a key pressed straight after the move acts on this row, not on
        // the one the app's next turn has yet to replace.
        focus = key
    }

    /// Runs one of a row's actions, the app's: the row in hand, then the action, whose question
    /// and form the window asks and opens.
    func run(_ action: LumennaApp.Offered, on row: LumennaApp.DayItem) {
        core.app.run(action, on: row.key)
    }

    /// The core's command `id` on `row` ("open": a block's Edit, free time's Add Block), or the
    /// app says why the row has none.
    func key(_ id: String, on row: LumennaApp.DayItem) {
        core.app.key(id, on: row.key)
    }

    /// One of the day's own commands by its id ("plan-day"), on the day shown, or why it has none.
    func key(_ id: String) {
        appear()
        core.app.key(id)
    }

    /// One of the day's own actions.
    func perform(_ action: LumennaApp.Offered) {
        core.app.act(action, on: .day)
    }

    /// Moves through the days: the app says the day newly shown, and puts now in hand for Now.
    func move(_ step: LumennaApp.DayMove) {
        appear()
        core.app.send(.day(step))
    }

    /// The row in hand, if it is shown.
    var inHand: LumennaApp.DayItem? {
        focus.flatMap { focus in rows.first { $0.id == focus }?.item }
    }
}
