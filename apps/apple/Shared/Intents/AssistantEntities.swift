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

/// `entities` in the order the core's search found them for `text` (`Device.searchResults`),
/// those it did not find left out: what matches is the core's, as on every platform. `key`
/// names an entity as a document of `kind` names it.
private func found<Entity>(
    _ entities: [Entity], _ store: AssistantStore, _ text: String, _ kind: LumennaApp.SearchKind,
    by key: (Entity) -> String, _ named: (LumennaApp.SearchDocument) -> String
) -> [Entity] {
    store.found(text).filter { $0.kind == kind }.flatMap { document in entities.filter { key($0) == named(document) } }
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
    @Property(title: "Estimate in Minutes") var estimate: Int?
    /// How it repeats, in the core's words ("every weekday").
    @Property(title: "Repeats") var repeats: String?
    /// Its states, as the filter language names them: overdue, blocked, recurring, …
    @Property(title: "States") var states: [String]
    @Property(title: "Added") var added: Date?
    /// What a search shows under its name, the core's: when it is due, its project and labels.
    /// A task done or in the trash has none.
    let line: String?
    /// The words a search finds it by beside its name: its notes, project and labels.
    let keywords: [String]
    /// What opening it hands the app (`ShellEvent.launch`), as its search document has it.
    let launch: String?

    init(_ task: LumennaApp.TaskDetail, found document: LumennaApp.SearchDocument? = nil) {
        id = task.id
        line = document.map(\.line).flatMap { $0.isEmpty ? nil : $0 }
        keywords = document?.keywords ?? []
        launch = document?.launch
        title = task.title
        due = task.due.flatMap { moment($0, task.dueTime) }
        project = task.project
        labels = task.labels
        priority = Int(task.priority)
        // The core's state keyword, as its filters read it.
        done = task.state.contains("completed")
        notes = task.notes
        estimate = task.estimateMins.map(Int.init)
        repeats = task.repetition
        states = task.state
        added = try? Date.ISO8601FormatStyle().parse(task.createdAt)
    }

    /// A task a glance names, by its identifier and title alone: what a widget's Mark Done hands
    /// Complete a Task, which gives the operation the identifier.
    init(glanced task: LumennaApp.GlanceTask) {
        self.init(id: task.id, title: task.title)
    }

    /// A task by its identifier and title alone.
    init(id: String, title: String) {
        self.id = id
        line = nil
        keywords = []
        launch = nil
        self.title = title
        due = nil
        project = nil
        labels = []
        priority = 0
        done = false
        notes = ""
        estimate = nil
        repeats = nil
        states = []
        added = nil
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(title)", subtitle: line.map { "\($0)" })
    }
}

/// Shortcuts' Find Tasks: each comparison it offers is a query in the filter language, so what
/// matches is the core's (`list_tasks`). A comparison the filter language has no form for is
/// not offered: a title alone (`search:` reads notes too), a priority above or below another,
/// a due date between two, an estimate, the states besides done. A due date or project left
/// empty is the filter language's `no date` or `no project`.
/// Names are quoted as the filter language reads a quoted name; a name holding a quotation mark
/// cannot be.
extension TaskQuery: EntityPropertyQuery {
    static let properties = QueryProperties {
        Property(\TaskEntity.$due) {
            LessThanComparator { "due before: \(Clock.isoDay($0))" }
            GreaterThanComparator { "due after: \(Clock.isoDay($0))" }
            EqualToComparator { $0.map { "due on: \(Clock.isoDay($0))" } ?? "no date" }
        }
        Property(\TaskEntity.$priority) {
            EqualToComparator { "p\($0)" }
            NotEqualToComparator { "!p\($0)" }
        }
        Property(\TaskEntity.$project) {
            EqualToComparator { $0.map { "#\"\($0)\"" } ?? "no project" }
            NotEqualToComparator { $0.map { "!#\"\($0)\"" } ?? "!no project" }
        }
        Property(\TaskEntity.$labels) {
            ContainsComparator { "@\"\($0)\"" }
        }
        Property(\TaskEntity.$done) {
            EqualToComparator { $0 ? "completed" : "!completed" }
        }
    }

    static let sortingOptions = SortingOptions {
        SortableBy(\TaskEntity.$due)
        SortableBy(\TaskEntity.$priority)
        SortableBy(\TaskEntity.$title)
    }

    func entities(
        matching comparators: [String],
        mode: ComparatorMode,
        sortedBy: [EntityQuerySort<TaskEntity>],
        limit: Int?
    ) async throws -> [TaskEntity] {
        let query = comparators.map { "(\($0))" }.joined(separator: mode == .and ? " & " : " | ")
        var tasks = store.tasks(try store.assist("list_tasks", ["query": .text(query)]))
        // The order Shortcuts asks for, else the core's (most urgent first).
        for sort in sortedBy.reversed() {
            tasks = order(tasks, by: sort)
        }
        return limit.map { Array(tasks.prefix($0)) } ?? tasks
    }

    private func order(_ tasks: [TaskEntity], by sort: EntityQuerySort<TaskEntity>) -> [TaskEntity] {
        let ascending = sort.order == .ascending
        func ordered<Value: Comparable>(_ value: (TaskEntity) -> Value?) -> [TaskEntity] {
            // Stable, what has no value last either way.
            tasks.enumerated().sorted { a, b in
                switch (value(a.element), value(b.element)) {
                case let (x?, y?) where x != y: ascending ? x < y : x > y
                case (nil, _?): false
                case (_?, nil): true
                default: a.offset < b.offset
                }
            }.map(\.element)
        }
        switch sort.by {
        case \TaskEntity.$due: return ordered { $0.due }
        case \TaskEntity.$priority: return ordered { $0.priority }
        case \TaskEntity.$title: return ordered { $0.title.lowercased() }
        default: return tasks
        }
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
        #if LUMENNA_WIDGETS && os(macOS) && !APP_STORE
        // The direct download's widget reaches no store: its Mark Done hands the identifier on
        // to the app (`WidgetRequests`), which is all it needs of the task.
        return identifiers.map { TaskEntity(id: $0, title: "") }
        #else
        let documents = store.documents()
        return identifiers.compactMap { id in store.entity(try? store.task(id), documents) }
        #endif
    }

    func entities(matching string: String) async throws -> [TaskEntity] {
        let documents = store.documents()
        return store.found(string).filter { $0.kind == .task }.compactMap { document in
            store.entity(try? store.task(String(document.id.dropFirst("task:".count))), documents)
        }
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
    /// What a platform's search has of it, the core's.
    let document: LumennaApp.SearchDocument?

    init(_ row: LumennaApp.RowView, found documents: [LumennaApp.SearchDocument] = []) {
        id = row.id
        value = row.value
        document = documents.first { $0.kind == .project && $0.title == row.title }
        name = row.title
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: value.map { "\($0)" })
    }
}

struct ProjectQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    private func all() throws -> [ProjectEntity] {
        let documents = store.searchDocuments()
        return rows(try store.assist("list_projects"), "project").map { ProjectEntity($0, found: documents) }
    }

    func entities(for identifiers: [ProjectEntity.ID]) async throws -> [ProjectEntity] {
        try all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [ProjectEntity] {
        found(try all(), store, string, .project, by: \.name, \.title)
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
    /// What a platform's search has of it, the core's.
    let document: LumennaApp.SearchDocument?

    init(_ row: LumennaApp.RowView, found documents: [LumennaApp.SearchDocument] = []) {
        id = row.id
        value = row.value
        document = documents.first { $0.kind == .label && $0.title == row.title }
        name = row.title
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(name)", subtitle: value.map { "\($0)" })
    }
}

struct LabelQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    private func all() throws -> [LabelEntity] {
        let documents = store.searchDocuments()
        return rows(try store.assist("list_labels"), "label").map { LabelEntity($0, found: documents) }
    }

    func entities(for identifiers: [LabelEntity.ID]) async throws -> [LabelEntity] {
        try all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [LabelEntity] {
        found(try all(), store, string, .label, by: \.name, \.title)
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
    /// What a platform's search has of it, the core's.
    let document: LumennaApp.SearchDocument?

    init(_ filter: LumennaApp.FilterView, found documents: [LumennaApp.SearchDocument] = []) {
        id = filter.id
        document = documents.first { $0.kind == .filter && $0.title == filter.name }
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
        let documents = store.searchDocuments()
        return filters.filters.map { SavedFilterEntity($0, found: documents) }
    }

    func entities(for identifiers: [SavedFilterEntity.ID]) async throws -> [SavedFilterEntity] {
        try all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [SavedFilterEntity] {
        found(try all(), store, string, .filter, by: \.name, \.title)
    }

    func suggestedEntities() async throws -> [SavedFilterEntity] {
        try all()
    }
}

/// A block as the Blocks list has it, every day of it: what a platform's search indexes of
/// blocks, built from its search document alone.
struct BlockSeriesEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Block"
    static let defaultQuery = BlockSeriesQuery()

    /// Its search document's: `block:<series>`.
    let id: String
    @Property(title: "Title") var title: String
    let document: LumennaApp.SearchDocument

    init(_ document: LumennaApp.SearchDocument) {
        id = document.id
        self.document = document
        title = document.title
    }

    var displayRepresentation: DisplayRepresentation {
        DisplayRepresentation(title: "\(title)", subtitle: document.line.isEmpty ? nil : "\(document.line)")
    }
}

struct BlockSeriesQuery: EntityStringQuery {
    @AppDependency private var store: AssistantStore

    private func all() -> [BlockSeriesEntity] {
        store.searchDocuments().filter { $0.kind == .block }.map(BlockSeriesEntity.init)
    }

    func entities(for identifiers: [BlockSeriesEntity.ID]) async throws -> [BlockSeriesEntity] {
        all().filter { identifiers.contains($0.id) }
    }

    func entities(matching string: String) async throws -> [BlockSeriesEntity] {
        store.found(string).filter { $0.kind == .block }.map(BlockSeriesEntity.init)
    }

    func suggestedEntities() async throws -> [BlockSeriesEntity] {
        all()
    }
}

// MARK: - The day

/// A block on a day, as the day lists it: its identifier is `<series>@<date>`.
struct BlockEntity: AppEntity {
    static let typeDisplayRepresentation: TypeDisplayRepresentation = "Block on a Day"
    static let defaultQuery = BlockQuery()

    let id: String
    /// The block (series) it is a day of.
    let series: String
    @Property(title: "Title") var title: String
    @Property(title: "Starts") var start: Date?
    @Property(title: "Ends") var end: Date?
    /// Its times as the core sends them, and the core's details: its length, its kind, now,
    /// the tasks in it.
    let times: (start: String, end: String)
    let details: [String]

    init(_ block: LumennaApp.PlanBlock, on date: String) {
        id = block.id
        series = block.series
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
    #if LUMENNA_WIDGETS
    // A widget has no Put a Task in a Block (it compiles Complete a Task alone): today's blocks.
    private let day: String? = nil
    #else
    /// The day Put a Task in a Block names, whose blocks it offers.
    @IntentParameterDependency<AssignIntent>(\.$date) private var assigning
    private var day: String? { assigning?.date }
    #endif

    private func offered() throws -> [BlockEntity] {
        try store.day(day)?.blockEntities ?? []
    }

    func entities(for identifiers: [BlockEntity.ID]) async throws -> [BlockEntity] {
        // Each on its own day, which its identifier names after the `@`.
        let days = Set(identifiers.compactMap { $0.split(separator: "@").last.map(String.init) })
        let found = days.flatMap { day in (try? store.day(day))?.blockEntities ?? [] }
        return identifiers.compactMap { id in found.first { $0.id == id } }
    }

    func entities(matching string: String) async throws -> [BlockEntity] {
        found(try offered(), store, string, .block, by: { "block:\($0.series)" }, \.id)
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
    /// The task it is a sitting of.
    let taskID: String
    @Property(title: "Task") var task: String
    @Property(title: "Block") var block: String
    @Property(title: "Running") var running: Bool
    @Property(title: "Minutes Worked") var minutes: Int
    /// The core's details: its status, its time, a capped timer.
    let details: [String]

    init(_ sitting: LumennaApp.PlanAssignment, in block: LumennaApp.PlanBlock) {
        id = sitting.id
        taskID = sitting.task
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

    /// Today's sittings of the tasks the core's search finds.
    func entities(matching string: String) async throws -> [SittingEntity] {
        found(try await suggestedEntities(), store, string, .task, by: { "task:\($0.taskID)" }, \.id)
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

/// Offered, not searched: the core's search covers no choice.
struct ChoiceQuery: EntityQuery {
    @AppDependency private var store: AssistantStore

    func entities(for identifiers: [ChoiceEntity.ID]) async throws -> [ChoiceEntity] {
        let offered = try store.choices().map(ChoiceEntity.init)
        return identifiers.compactMap { id in offered.first { $0.id == id } }
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
    #if LUMENNA_WIDGETS
    // A widget has no Apply a Day Plan: today's plan.
    private let day: String? = nil
    #else
    /// The day Apply a Day Plan names, whose plan it offers.
    @IntentParameterDependency<ApplyDayPlanIntent>(\.$date) private var applying
    private var day: String? { applying?.date }
    #endif

    func entities(for identifiers: [ProposedSittingEntity.ID]) async throws -> [ProposedSittingEntity] {
        // Each day's plan proposed again, the day its block's identifier names.
        let days = Set(identifiers.compactMap { $0.split(separator: "|").dropFirst().first?.split(separator: "@").last.map(String.init) })
        let proposed = days.flatMap { day in (try? store.dayPlan(day))?.sittings.map(ProposedSittingEntity.init) ?? [] }
        return identifiers.compactMap { id in proposed.first { $0.id == id } }
    }

    /// The sittings proposed now for the day named, or today.
    func suggestedEntities() async throws -> [ProposedSittingEntity] {
        try store.dayPlan(day)?.sittings.map(ProposedSittingEntity.init) ?? []
    }
}
