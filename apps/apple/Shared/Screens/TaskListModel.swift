import Foundation
import LumennaApp

/// A place's tasks, or every block series, as the app lists them (`ViewModel.list`), for a SwiftUI list: every row with
/// its line, those folded away marked, the row in hand, and each time the app moves it.
///
/// The app decides what is listed and folded, which row is in hand after a change and what each
/// row offers; this follows it and reports the person's own moves (`moved(to:)`). The list opens
/// its place as it comes to the front, unless the app shows it already.
final class TaskListModel: ObservableObject {
    /// A row as the list draws it: the app's, and its line in this device's words.
    struct Row: Identifiable, Equatable {
        let item: LumennaApp.Item
        let line: LumennaApp.Line?
        var id: String { item.row.id }
        var row: LumennaApp.RowView { item.row }
    }

    let core: Core
    /// The place this list shows.
    let place: LumennaApp.Place
    var trash: Bool { place == .trash }
    /// Whether this is the Blocks list, every block series, rather than tasks.
    var blocks: Bool { place == .blocks }
    /// Whether the list has a filter above it: the trash and the blocks have none.
    var filters: Bool { !trash && !blocks }
    /// What a row here is, for saying why it has no action of a kind.
    var subject: LumennaApp.Subject { blocks ? .block : .task }
    /// Every row, those folded away included (`hidden`): a tree draws them inside their
    /// collapsed parents, a flat list leaves them out.
    @Published private(set) var rows: [Row] = []
    /// How the query was understood and how many it found, or what is empty.
    @Published private(set) var readback = ""
    @Published private(set) var title: String
    /// The row in hand, the app's.
    @Published private(set) var focus: String?
    /// The row the app moved focus to, each time it moves it: a new `count` is a new move.
    @Published private(set) var landed: (key: String, count: Int)?
    /// Whether the filter is wanted in hand (Filter Tasks), until the field has taken focus.
    @Published var filterWanted = false
    /// The filter as typed, sent to the app as it changes.
    @Published var filter = "" {
        didSet { if !following, filter != oldValue { core.app.send(.filter(filter)) } }
    }

    /// Whether the filter is being set from the app's, not typed.
    private var following = false
    /// The app's landing count at the last look.
    private var landing: UInt32?
    private var observer: NSObjectProtocol?

    init(core: Core, place: LumennaApp.Place) {
        self.core = core
        self.place = place
        title = LumennaApp.placeTitle(place: place)
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Whether the app is showing this list's place: only then is its list this one's.
    var here: Bool { core.app.view.place == place }

    /// Makes this list's place the app's, unless it is already. Coming back to it, from a
    /// task's details or another place, finds the row in hand and the filter it was left with:
    /// the app keeps them. Opening is the list coming to the front, which VoiceOver says itself:
    /// no landing.
    func appear() {
        if !here {
            core.app.send(.open(place))
            landing = nil
        }
        draw()
    }

    /// The app's list, while it shows this place.
    private func draw() {
        let view = core.app.view
        guard view.place == place, let list = view.list else { return }
        let lines = core.app.lines
        let rows = list.items.enumerated().map { index, item in
            Row(item: item, line: index < lines.list.count ? lines.list[index] : nil)
        }
        if rows != self.rows { self.rows = rows }
        if readback != lines.readback { readback = lines.readback }
        if title != view.title { title = view.title }
        if focus != view.focus { focus = view.focus }
        if filters, filter != view.query, core.app.isStill {
            following = true
            filter = view.query
            following = false
        }
        let moved = landing.map { view.landing > $0 } ?? false
        landing = view.landing
        if moved, let key = view.focus { landed = (key, (landed?.count ?? 0) + 1) }
    }

    /// The rows a flat list shows: those not folded away.
    var shown: [Row] { rows.filter { !$0.item.hidden } }

    /// The person moved to a row: the app's row in hand, whose details show beside it.
    func moved(to id: String) {
        guard here, id != core.app.view.focus else { return }
        core.app.send(.focus(id))
        // In hand here at once: a key pressed straight after the move acts on this row, not on
        // the one the app's next turn has yet to replace.
        focus = id
    }

    /// Runs one of a row's actions, the app's: the row in hand, then the action, whose question
    /// the window's `AppQuestions` asks.
    func run(_ action: LumennaApp.Action, on row: LumennaApp.RowView) {
        core.app.run(action, on: row.id)
    }

    /// The row's action of one of `kinds`, or why it has none, in the core's words.
    func run(_ kinds: [LumennaApp.ActionKind], on row: LumennaApp.RowView, saying kind: LumennaApp.ActionKind) {
        if let action = row.actions.first(where: { kinds.contains($0.kind) }) {
            run(action, on: row)
        } else {
            Announcer.say(LumennaApp.notOffered(kind: kind, subject: subject, thisDevice: false))
        }
    }

    /// The row in hand, if it is listed.
    var inHand: LumennaApp.RowView? {
        focus.flatMap { focus in rows.first { $0.id == focus }?.row }
    }
}
