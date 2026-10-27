import AppIntents
import Foundation
import LumennaApp

/// The store Siri, Shortcuts, Spotlight and Apple Intelligence work on: the device of the
/// process the intent runs in, as each app (and each extension to come) gives it with
/// `Assistant.serve`. Every intent runs its operation through `Device.assist`, as `lum mcp`
/// does, so no assistant can do what another cannot.
final class AssistantStore: @unchecked Sendable {
    /// The app's store, which it holds open for the process's life; none in a widget.
    private let held: LumennaApp.Device?
    /// A widget's: the store opened afresh for each call.
    private let opening: (() throws -> LumennaApp.Device)?
    /// Hears of an operation that changed the store while it is still open: in a widget, which
    /// then writes its glance again (`GlanceFile`), the app perhaps not running.
    private let changed: ((LumennaApp.Device) -> Void)?

    /// The app's store, held open.
    init(device: LumennaApp.Device) {
        held = device
        opening = nil
        changed = nil
    }

    /// A store `open` opens for each call and lets go before the call returns: a widget's, which
    /// must hold no file in the group open while it is suspended, or iOS ends it (`0xdead10cc`).
    init(opening open: @escaping () throws -> LumennaApp.Device, changed: @escaping (LumennaApp.Device) -> Void) {
        held = nil
        opening = open
        self.changed = changed
    }

    /// `body` on the store: the app's own, or one opened for it alone, closed once nothing holds
    /// it, so `body` must keep nothing of it.
    private func using<T>(_ body: (LumennaApp.Device) throws -> T) throws -> T {
        if let held { return try body(held) }
        guard let opening else { throw CocoaError(.fileNoSuchFile) }
        return try body(try opening())
    }

    /// Runs the assistant operation `id` with what its parameters were given, by name; a value
    /// left out is not given. A refusal is the core's sentence, for Siri to say.
    func assist(_ id: String, _ arguments: [String: LumennaApp.ArgumentValue?] = [:]) throws -> LumennaApp.Assisted {
        let given = arguments.compactMap { name, value in value.map { LumennaApp.Argument(name: name, value: $0) } }
        do {
            return try using { device in
                // By the heads, which move for every write: what the operation did is the core's.
                let before = changed == nil ? nil : try? device.heads()
                let assisted = try device.assist(operation: id, arguments: given)
                if let changed, (try? device.heads()) != before { changed(device) }
                return assisted
            }
        } catch {
            throw AssistantRefusal(error)
        }
    }

    /// One task whole, by its identifier.
    func task(_ id: String) throws -> LumennaApp.TaskDetail? {
        guard case let .task(shown) = try assist("show_task", ["id": .text(id)]) else { return nil }
        return shown.task
    }

    /// Everything a platform's search indexes, the core's (`records::search`).
    func searchDocuments() -> [LumennaApp.SearchDocument] {
        (try? using { try $0.searchDocuments() }) ?? []
    }

    /// What the core's search finds for `text`, most fitting first.
    func found(_ text: String) -> [LumennaApp.SearchDocument] {
        (try? using { try $0.searchResults(text: text) }) ?? []
    }

    /// The tasks `ids` names as entities, in that order, done and trashed ones too, from their
    /// search documents in one call (`Device.taskDocuments`); an identifier naming no task is
    /// left out.
    func entities(_ ids: [String]) -> [TaskEntity] {
        guard !ids.isEmpty else { return [] }
        return ((try? using { try $0.taskDocuments(ids: ids) }) ?? []).compactMap(TaskEntity.init)
    }

    /// A task an operation answered with, as an entity.
    func entity(_ task: LumennaApp.TaskDetail?) -> TaskEntity? {
        task.flatMap { entities([$0.id]).first }
    }

    /// Each task a listing names, in the listing's order.
    func tasks(_ assisted: LumennaApp.Assisted) -> [TaskEntity] {
        guard case let .rows(rows) = assisted else { return [] }
        return entities(rows.rows.filter { $0.role == "task" }.map(\.id))
    }

    /// A day, as a person says it, or today.
    func day(_ date: String?) throws -> LumennaApp.Plan? {
        guard case let .plan(plan) = try assist("day", ["date": date.map(ArgumentValue.text)]) else { return nil }
        return plan
    }

    /// The replan of the rest of today as the core proposes it now.
    func replan() throws -> LumennaApp.Replan? {
        guard case let .replan(replan) = try assist("propose_replan") else { return nil }
        return replan
    }

    /// The plan for a day as the core proposes it now.
    func dayPlan(_ date: String?) throws -> LumennaApp.DayPlan? {
        guard case let .dayPlan(plan) = try assist("propose_day_plan", ["date": date.map(ArgumentValue.text)]) else { return nil }
        return plan
    }

    /// What What Should I Work On? offers now.
    func choices() throws -> [LumennaApp.Choice] {
        guard case let .choices(choices) = try assist("what_should_i_work_on") else { return [] }
        return choices.choices
    }
}

/// How a search result chosen reaches the app: its `launch`, handed to the app as
/// `ShellEvent.launch`, which opens it.
final class AssistantLauncher: @unchecked Sendable {
    let open: @MainActor (String) -> Void

    init(open: @escaping @MainActor (String) -> Void) {
        self.open = open
    }
}

enum Assistant {
    /// Gives the intents this process's store, and `launch`, which hands the app a search
    /// result's launch: called by each app as it launches, before any intent can run, since an
    /// intent may be what launched it. Where Spotlight is, the core's search documents are
    /// indexed for it, and again whenever the store changes.
    static func serve(_ device: LumennaApp.Device, launch: @escaping @MainActor (String) -> Void) {
        let store = AssistantStore(device: device)
        AppDependencyManager.shared.add(dependency: store)
        AppDependencyManager.shared.add(dependency: AssistantLauncher(open: launch))
        #if !os(watchOS) && !LUMENNA_WIDGETS
        if #available(iOS 18.0, macOS 15.0, *) {
            SearchIndex.follow(store)
        }
        #endif
    }
}

/// What the core refused, in its own words, for Siri and Shortcuts to say.
struct AssistantRefusal: Error, CustomLocalizedStringResourceConvertible {
    let sentence: String

    init(_ error: Error) {
        sentence = (error as? AssistantRefusal)?.sentence ?? error.sentence
    }

    init(sentence: String) {
        self.sentence = sentence
    }

    var localizedStringResource: LocalizedStringResource { "\(sentence)" }
}

extension LumennaApp.Assisted {
    /// What it says: the core's sentence, then anything else worth saying.
    var said: String {
        let (announcement, notices): (String, [String]) = switch self {
        case let .change(x): (x.announcement, x.notices)
        case let .rows(x): (x.announcement, x.notices)
        case let .task(x): (x.announcement, x.notices)
        case let .plan(x): (x.announcement, x.notices)
        case let .filters(x): (x.announcement, x.notices)
        case let .choices(x): (x.announcement, x.notices)
        case let .replan(x): (x.announcement, x.notices)
        case let .dayPlan(x): (x.announcement, x.notices)
        case let .timer(x): (x.announcement, x.notices)
        case let .replanned(x): (x.announcement, x.notices)
        case let .dayPlanned(x): (x.announcement, x.notices)
        }
        return ([announcement] + notices).joined(separator: " ")
    }

    /// The task a change left, whole.
    var task: LumennaApp.TaskDetail? {
        switch self {
        case let .change(change): change.task
        case let .task(shown): shown.task
        default: nil
        }
    }
}

extension LumennaApp.Replan {
    /// The whole proposal as the core words it, each change its own line: for someone who
    /// cannot glance at a timeline, the description is the check.
    var said: String {
        ([announcement] + notices + changes.map(\.sentence) + unplaceable).joined(separator: "\n")
    }
}

extension LumennaApp.DayPlan {
    /// The whole plan as the core words it, each sitting and why on its own line.
    var said: String {
        ([announcement] + notices + sittings.map { [$0.sentence, $0.reason].compactMap { $0 }.joined(separator: ", ") } + unfit)
            .joined(separator: "\n")
    }
}

extension IntentDialog {
    /// The core's words, said as they are.
    init(said text: String) {
        self.init("\(text)")
    }
}

extension AppIntent {
    /// Asks the person `question` and goes on only once they say yes; no ends the intent.
    func askFirst(_ question: String) async throws {
        if #available(iOS 18.0, macOS 15.0, watchOS 11.0, *) {
            try await requestConfirmation(dialog: IntentDialog(said: question))
        } else {
            try await requestConfirmation(result: .result(dialog: IntentDialog(said: question)))
        }
    }
}
