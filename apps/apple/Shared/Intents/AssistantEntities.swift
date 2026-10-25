import AppIntents
import CoreTransferable
import Foundation
import LumennaApp

// What Siri, Shortcuts and Apple Intelligence find and show: each thing an operation takes,
// built from the record the core answers with, and named in its words. An entity carries its
// record's identifier, which is what the operation is given back.

/// A day and time as the core sends them (`YYYY-MM-DD`, `HH:MM`), as a moment in this
/// device's time zone, for Shortcuts to compare and format.
private func moment(_ day: String, _ time: String? = nil) -> Date? {
    let date = day.split(separator: "-").compactMap { Int($0) }
    guard date.count == 3 else { return nil }
    var components = DateComponents(year: date[0], month: date[1], day: date[2])
    if let clock = time?.split(separator: ":").compactMap({ Int($0) }), clock.count == 2 {
        components.hour = clock[0]
        components.minute = clock[1]
    }
    return Calendar.current.date(from: components)
}

/// Whether `name` is what a person searching for `text` means: the platform's own match.
private func matches(_ name: String, _ text: String) -> Bool {
    name.localizedStandardContains(text)
}

// MARK: - Tasks

struct TaskEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Task"
    static let defaultQuery = TaskQuery()

    let id: String
    @Property(title: "Title") var title: String
    @Property(title: "Due") var due: Date?
    @Property(title: "Project") var project: String?
    @Property(title: "Labels") var labels: [String]
    @Property(title: "Priority") var priority: Int
    @Property(title: "Done") var done: Bool
    @Property(title: "Notes") var notes: String
    /// What a search shows under its name, the core's: when it is due, its project and labels.
    /// A task done or in the trash has none.
    let line: String?
    /// The words a search finds it by beside its name: its notes, project and labels.
    let keywords: [String]

    init(_ task: LumennaApp.TaskDetail, found document: LumennaApp.SearchDocument? = nil) {
        id = task.id
        line = document.map(\.line).flatMap { $0.isEmpty ? nil : $0 }
        keywords = document?.keywords ?? []
        title = task.title
        due = task.due.flatMap { moment($0, task.dueTime) }
        project = task.project
        labels = task.labels
        priority = Int(task.priority)
        // The core's state keyword, as its filters read it.
        done = task.state.contains("completed")
        notes = task.notes
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(title)", subtitle: line.map { "\($0)" })
    }
}

/// A task as text, for Shortcuts' Use Model action, sharing and anything that takes words:
/// its title, then its notes.
extension TaskEntity: Transferable {
    static var transferRepresentation: some TransferRepresentation {
        ProxyRepresentation { (task: TaskEntity) in
            task.notes.isEmpty ? task.title : task.title + "\n\n" + task.notes
        }
    }
}

struct TaskQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    func entities(for identifiers: [TaskEntity.ID]) async throws -> [TaskEntity] {
        let documents = store.documents()
        return identifiers.compactMap { id in store.entity(try? store.task(id), documents) }
    }

    func entities(matching string: String) async throws -> [TaskEntity] {
        store.tasks(try store.assist("search_tasks", ["text": .text(string)]))
    }

    /// The open tasks, most urgent first, as every list orders them.
    func suggestedEntities() async throws -> [TaskEntity] {
        store.tasks(try store.assist("list_tasks", ["query": .text("")]))
    }
}

// MARK: - Projects, labels and saved filters

/// A project, a label: each by the row the core lists it as.
private func rows(_ assisted: LumennaApp.Assisted, _ role: String) -> [LumennaApp.RowView] {
    guard case let .rows(rows) = assisted else { return [] }
    return rows.rows.filter { $0.role == role }
}

struct ProjectEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Project"
    static let defaultQuery = ProjectQuery()

    let id: String
    @Property(title: "Name") var name: String
    /// What the core says beside its name: how many tasks are in it.
    let value: String?

    init(_ row: LumennaApp.RowView) {
        id = row.id
        value = row.value
        name = row.title
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: value.map { "\($0)" })
    }
}

struct ProjectQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    private func all() throws -> [ProjectEntity] {
        rows(try store.assist("list_projects"), "project").map(ProjectEntity.init)
    }

    func entities(for identifiers: [ProjectEntity.ID]) async throws -> [ProjectEntity] {
        try all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [ProjectEntity] {
        try all().filter { matches($0.name, string) }
    }

    func suggestedEntities() async throws -> [ProjectEntity] {
        try all()
    }
}

struct LabelEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Label"
    static let defaultQuery = LabelQuery()

    let id: String
    @Property(title: "Name") var name: String
    let value: String?

    init(_ row: LumennaApp.RowView) {
        id = row.id
        value = row.value
        name = row.title
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: value.map { "\($0)" })
    }
}

struct LabelQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    private func all() throws -> [LabelEntity] {
        rows(try store.assist("list_labels"), "label").map(LabelEntity.init)
    }

    func entities(for identifiers: [LabelEntity.ID]) async throws -> [LabelEntity] {
        try all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [LabelEntity] {
        try all().filter { matches($0.name, string) }
    }

    func suggestedEntities() async throws -> [LabelEntity] {
        try all()
    }
}

struct SavedFilterEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Saved Filter"
    static let defaultQuery = SavedFilterQuery()

    let id: String
    @Property(title: "Name") var name: String
    /// Its query in the filter language, which List Tasks takes.
    @Property(title: "Query") var query: String

    init(_ filter: LumennaApp.FilterView) {
        id = filter.id
        name = filter.name
        query = filter.query
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: "\(query)")
    }
}

struct SavedFilterQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    private func all() throws -> [SavedFilterEntity] {
        guard case let .filters(filters) = try store.assist("list_filters") else { return [] }
        return filters.filters.map(SavedFilterEntity.init)
    }

    func entities(for identifiers: [SavedFilterEntity.ID]) async throws -> [SavedFilterEntity] {
        try all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [SavedFilterEntity] {
        try all().filter { matches($0.name, string) }
    }

    func suggestedEntities() async throws -> [SavedFilterEntity] {
        try all()
    }
}

// MARK: - The day

/// A block on a day, as the day lists it: its identifier is `<series>@<date>`.
struct BlockEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Block"
    static let defaultQuery = BlockQuery()

    let id: String
    @Property(title: "Title") var title: String
    @Property(title: "Starts") var start: Date?
    @Property(title: "Ends") var end: Date?
    /// Its times as the core sends them, and the core's details: its length, its kind, now,
    /// the tasks in it.
    let times: (start: String, end: String)
    let details: [String]

    init(_ block: LumennaApp.PlanBlock, on date: String) {
        id = block.id
        times = (block.start, block.end)
        details = block.details
        title = block.title
        let start = moment(date, block.start)
        self.start = start
        // Its end is its start plus its minutes, across a change of clocks too.
        end = start.map { $0.addingTimeInterval(TimeInterval(block.durationMins) * 60) }
    }

    var displayRepresentation: DisplayRepresentation {
        let when = "\(Clock.time(times.start)) – \(Clock.time(times.end))"
        return DisplayRepresentation(title: "\(title)", subtitle: "\(([when] + details).joined(separator: ", "))")
    }
}

extension LumennaApp.Plan {
    /// Its blocks, as entities.
    var blockEntities: [BlockEntity] {
        blocks.map { BlockEntity($0, on: date) }
    }

    /// Every sitting in its blocks, as entities, in the day's order.
    var sittingEntities: [SittingEntity] {
        blocks.flatMap { block in block.assignments.map { SittingEntity($0, in: block) } }
    }
}

struct BlockQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore
    /// The day Put a Task in a Block names, whose blocks it offers.
    @IntentParameterDependency<AssignIntent>(\.$date) private var assigning

    private func offered() throws -> [BlockEntity] {
        try store.day(assigning?.date)?.blockEntities ?? []
    }

    func entities(for identifiers: [BlockEntity.ID]) async throws -> [BlockEntity] {
        // Each on its own day, which its identifier names after the `@`.
        let days = Set(identifiers.compactMap { $0.split(separator: "@").last.map(String.init) })
        let found = days.flatMap { day in (try? store.day(day))?.blockEntities ?? [] }
        return identifiers.compactMap { id in found.first { $0.id == id } }
    }

    func entities(matching string: String) async throws -> [BlockEntity] {
        try offered().filter { matches($0.title, string) }
    }

    func suggestedEntities() async throws -> [BlockEntity] {
        try offered()
    }
}

/// A sitting: a task's place in a block, one session of work on it there.
struct SittingEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Sitting"
    static let defaultQuery = SittingQuery()

    let id: String
    @Property(title: "Task") var task: String
    @Property(title: "Block") var block: String
    @Property(title: "Running") var running: Bool
    @Property(title: "Minutes Worked") var minutes: Int
    /// The core's details: its status, its time, a capped timer.
    let details: [String]

    init(_ sitting: LumennaApp.PlanAssignment, in block: LumennaApp.PlanBlock) {
        id = sitting.id
        details = sitting.details
        task = sitting.title
        self.block = block.title
        running = sitting.running
        minutes = Int(sitting.minutes)
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(task)", subtitle: "\(([block] + details).joined(separator: ", "))")
    }
}

struct SittingQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    func entities(for identifiers: [SittingEntity.ID]) async throws -> [SittingEntity] {
        // Today's first, where a timer is started, paused and stopped; then the days beside it.
        var found: [SittingEntity] = []
        for day in [nil, "yesterday", "tomorrow"] where !identifiers.allSatisfy({ id in found.contains { $0.id == id } }) {
            found += (try? store.day(day))?.sittingEntities ?? []
        }
        return identifiers.compactMap { id in found.first { $0.id == id } }
    }

    func entities(matching string: String) async throws -> [SittingEntity] {
        try await suggestedEntities().filter { matches($0.task, string) }
    }

    /// Today's sittings.
    func suggestedEntities() async throws -> [SittingEntity] {
        try store.day(nil)?.sittingEntities ?? []
    }
}

/// Today's sittings, the one whose timer runs first in hand: what Pause and Stop act on.
struct RunningSitting: DynamicOptionsProvider {
    @AppDependency private var store: AssistantStore

    func results() async throws -> [SittingEntity] {
        try store.day(nil)?.sittingEntities ?? []
    }

    func defaultResult() async -> SittingEntity? {
        (try? store.day(nil))?.sittingEntities.first { $0.running }
    }
}

// MARK: - Planning

/// One of What Should I Work On?'s choices, with the reason it is offered.
struct ChoiceEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Choice"
    static let defaultQuery = ChoiceQuery()

    let id: String
    @Property(title: "Title") var title: String
    @Property(title: "Reason") var reason: String?
    let detail: String?

    init(_ choice: LumennaApp.Choice) {
        id = choice.id
        detail = choice.detail
        title = choice.title
        reason = choice.reason
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(title)", subtitle: (reason ?? detail).map { "\($0)" })
    }
}

struct ChoiceQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    func entities(for identifiers: [ChoiceEntity.ID]) async throws -> [ChoiceEntity] {
        let offered = try store.choices().map(ChoiceEntity.init)
        return identifiers.compactMap { id in offered.first { $0.id == id } }
    }

    func entities(matching string: String) async throws -> [ChoiceEntity] {
        try await suggestedEntities().filter { matches($0.title, string) }
    }

    /// What it offers now, in its order.
    func suggestedEntities() async throws -> [ChoiceEntity] {
        try store.choices().map(ChoiceEntity.init)
    }
}

/// One change a replan of today proposes, named by its sentence. A replan is proposed afresh
/// whenever one is asked for, so a change is known again by what it does.
struct ReplanChangeEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Replan Change"
    static let defaultQuery = ReplanChangeQuery()

    let id: String
    @Property(title: "Change") var sentence: String
    let change: LumennaApp.ReplanChange

    init(_ change: LumennaApp.ReplanChange) {
        id = [change.kind, change.target, "\(change.fromStart)", "\(change.toStart)", "\(change.fromMins)", "\(change.toMins)", change.destination]
            .joined(separator: "|")
        self.change = change
        sentence = change.sentence
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(sentence)")
    }
}

struct ReplanChangeQuery: EntityQuery {
    @AppDependency private var store: AssistantStore

    func entities(for identifiers: [ReplanChangeEntity.ID]) async throws -> [ReplanChangeEntity] {
        let proposed = try await suggestedEntities()
        return identifiers.compactMap { id in proposed.first { $0.id == id } }
    }

    /// The changes proposed now.
    func suggestedEntities() async throws -> [ReplanChangeEntity] {
        try store.replan()?.changes.map(ReplanChangeEntity.init) ?? []
    }
}

/// One sitting a day plan proposes, named by its sentence and why.
struct ProposedSittingEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Proposed Sitting"
    static let defaultQuery = ProposedSittingQuery()

    let id: String
    @Property(title: "Sitting") var sentence: String
    @Property(title: "Reason") var reason: String?
    let sitting: LumennaApp.ProposedSitting

    init(_ sitting: LumennaApp.ProposedSitting) {
        id = [sitting.task, sitting.block, "\(sitting.minutes)"].joined(separator: "|")
        self.sitting = sitting
        sentence = sitting.sentence
        reason = sitting.reason
    }

    /// The day it is proposed for, which its block's identifier names after the `@`.
    var day: String? {
        sitting.block.split(separator: "@").last.map(String.init)
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(sentence)", subtitle: reason.map { "\($0)" })
    }
}

struct ProposedSittingQuery: EntityQuery {
    @AppDependency private var store: AssistantStore
    /// The day Apply a Day Plan names, whose plan it offers.
    @IntentParameterDependency<ApplyDayPlanIntent>(\.$date) private var applying

    func entities(for identifiers: [ProposedSittingEntity.ID]) async throws -> [ProposedSittingEntity] {
        // Each day's plan proposed again, the day its block's identifier names.
        let days = Set(identifiers.compactMap { $0.split(separator: "|").dropFirst().first?.split(separator: "@").last.map(String.init) })
        let proposed = days.flatMap { day in (try? store.dayPlan(day))?.sittings.map(ProposedSittingEntity.init) ?? [] }
        return identifiers.compactMap { id in proposed.first { $0.id == id } }
    }

    /// The sittings proposed now for the day named, or today.
    func suggestedEntities() async throws -> [ProposedSittingEntity] {
        try store.dayPlan(applying?.date)?.sittings.map(ProposedSittingEntity.init) ?? []
    }
}
