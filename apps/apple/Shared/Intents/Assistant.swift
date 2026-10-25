import AppIntents
import Foundation
import LumennaApp

/// The store Siri, Shortcuts, Spotlight and Apple Intelligence work on: the device of the
/// process the intent runs in, as each app (and each extension to come) gives it with
/// `Assistant.serve`. Every intent runs its operation through `Device.assist`, as `lum mcp`
/// does, so no assistant can do what another cannot.
final class AssistantStore: @unchecked Sendable {
    let device: LumennaApp.Device

    init(device: LumennaApp.Device) {
        self.device = device
    }

    /// Runs the assistant operation `id` with what its parameters were given, by name; a value
    /// left out is not given. A refusal is the core's sentence, for Siri to say.
    func assist(_ id: String, _ arguments: [String: LumennaApp.ArgumentValue?] = [:]) throws -> LumennaApp.Assisted {
        let given = arguments.compactMap { name, value in value.map { LumennaApp.Argument(name: name, value: $0) } }
        do {
            return try device.assist(operation: id, arguments: given)
        } catch {
            throw AssistantRefusal(error)
        }
    }

    /// One task whole, by its identifier.
    func task(_ id: String) throws -> LumennaApp.TaskDetail? {
        guard case let .task(shown) = try assist("show_task", ["id": .text(id)]) else { return nil }
        return shown.task
    }

    /// What a platform's search finds of each open task, by the task's identifier: its line
    /// and the words it is found by, the core's.
    func documents() -> [String: LumennaApp.SearchDocument] {
        let tasks = ((try? device.searchDocuments()) ?? []).filter { $0.kind == .task }
        return Dictionary(tasks.map { (String($0.id.dropFirst("task:".count)), $0) }) { first, _ in first }
    }

    /// A task as an entity, with what search finds of it.
    func entity(_ task: LumennaApp.TaskDetail?, _ documents: [String: LumennaApp.SearchDocument]? = nil) -> TaskEntity? {
        task.map { TaskEntity($0, found: (documents ?? self.documents())[$0.id]) }
    }

    /// Each task a listing names, whole, in the listing's order.
    func tasks(_ assisted: LumennaApp.Assisted) -> [TaskEntity] {
        guard case let .rows(rows) = assisted else { return [] }
        let documents = self.documents()
        return rows.rows.filter { $0.role == "task" }.compactMap { row in entity(try? task(row.id), documents) }
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

enum Assistant {
    /// Gives the intents this process's store: called by each app as it launches, before any
    /// intent can run, since an intent may be what launched it. Where Spotlight is, the open
    /// tasks are indexed for it, and again whenever the store changes.
    static func serve(_ device: LumennaApp.Device) {
        let store = AssistantStore(device: device)
        AppDependencyManager.shared.add(dependency: store)
        #if !os(watchOS)
        if #available(iOS 18.0, macOS 15.0, *) {
            TaskIndex.follow(store)
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
