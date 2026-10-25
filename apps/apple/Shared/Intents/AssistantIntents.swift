import AppIntents
import Foundation
import LumennaApp

// One intent per assistant operation (`lumenna_records::assistant`, `assistantOperations()`),
// each running it through `Device.assist`: what Siri, Shortcuts, Spotlight and Apple
// Intelligence can do is what `lum mcp` can, in the same words. App Intents' metadata is read
// at build time, so each identifier, title, description, category and keyword is the table's
// as a literal; `AssistantIntentTests` compares every one with the table at run time.
//
// A parameter taking a task, a project, labels, a block, a sitting or a choice takes the
// entity, and gives the operation its identifier (a project or label its name). An operation
// that asks first (`confirms`) puts the core's question to the person and goes ahead only on
// yes; one that applies a proposal proposes it afresh, says it, and asks before applying.

/// An intent that is one of the assistant's operations, its `persistentIdentifier` the
/// operation's `id`.
protocol OperationIntent: AppIntent {}

/// Every operation's intent, in the table's order: what the tests compare with the table.
let assistantIntents: [any OperationIntent.Type] = [
    ListTasksIntent.self, SearchTasksIntent.self, ShowTaskIntent.self, ShowDayIntent.self,
    ListBlocksIntent.self, ListProjectsIntent.self, ListLabelsIntent.self, ListSavedFiltersIntent.self,
    WhatShouldIWorkOnIntent.self, ProposeReplanIntent.self, ProposeDayPlanIntent.self, AddTaskIntent.self,
    EditTaskIntent.self, CompleteTaskIntent.self, UncompleteTaskIntent.self, TrashTaskIntent.self,
    EraseTaskPermanentlyIntent.self, EraseEverythingDeletedIntent.self, RestoreTaskIntent.self, MoveTaskIntent.self,
    AssignIntent.self, UnassignIntent.self, StartTimerIntent.self, PauseTimerIntent.self,
    StopTimerIntent.self, StartOnIntent.self, ApplyReplanIntent.self, ApplyDayPlanIntent.self,
    UndoIntent.self, RedoIntent.self,
]

/// What a write answers: the core's sentence.
private func said(_ assisted: LumennaApp.Assisted) -> some IntentResult & ProvidesDialog {
    .result(dialog: IntentDialog(said: assisted.said))
}

// MARK: - Tasks

struct ListTasksIntent: OperationIntent {
    static let persistentIdentifier = "list_tasks"
    static let title: LocalizedStringResource = "List Tasks"
    static let description = IntentDescription(
        "Lists tasks: by default every open task, most urgent first, subtasks under their task; or those a filter admits.",
        categoryName: "Tasks",
        searchKeywords: ["tasks", "to do", "overdue", "due"]
    )

    @Parameter(title: "Query", description: "A filter in Lumenna's filter language, such as `overdue`, `today & #Work`, `@calls`, `p1`, or `search: words`; empty for every open task.")
    var query: String?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[TaskEntity]> & ProvidesDialog {
        let listed = try store.assist(Self.persistentIdentifier, ["query": .text(query ?? "")])
        return .result(value: store.tasks(listed), dialog: IntentDialog(said: listed.said))
    }
}

struct SearchTasksIntent: OperationIntent {
    static let persistentIdentifier = "search_tasks"
    static let title: LocalizedStringResource = "Search Tasks"
    static let description = IntentDescription(
        "Searches task titles and notes for words.",
        categoryName: "Tasks",
        searchKeywords: ["find", "search"]
    )

    @Parameter(title: "Text", description: "Words to search titles and notes for.")
    var text: String

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[TaskEntity]> & ProvidesDialog {
        let found = try store.assist(Self.persistentIdentifier, ["text": .text(text)])
        return .result(value: store.tasks(found), dialog: IntentDialog(said: found.said))
    }
}

struct ShowTaskIntent: OperationIntent {
    static let persistentIdentifier = "show_task"
    static let title: LocalizedStringResource = "Show a Task"
    static let description = IntentDescription(
        "One task whole: its fields, states, subtasks, what it waits for, its sittings and its history.",
        categoryName: "Tasks",
        searchKeywords: ["details", "notes"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<TaskEntity> & ProvidesDialog {
        let shown = try store.assist(Self.persistentIdentifier, ["id": .text(id.id)])
        return .result(value: store.entity(shown.task) ?? id, dialog: IntentDialog(said: shown.said))
    }
}

struct ListProjectsIntent: OperationIntent {
    static let persistentIdentifier = "list_projects"
    static let title: LocalizedStringResource = "List Projects"
    static let description = IntentDescription(
        "Every project, its subprojects under it.",
        categoryName: "Tasks",
        searchKeywords: ["projects"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[ProjectEntity]> & ProvidesDialog {
        let listed = try store.assist(Self.persistentIdentifier)
        guard case let .rows(rows) = listed else { return .result(value: [], dialog: IntentDialog(said: listed.said)) }
        return .result(value: rows.rows.filter { $0.role == "project" }.map(ProjectEntity.init), dialog: IntentDialog(said: listed.said))
    }
}

struct ListLabelsIntent: OperationIntent {
    static let persistentIdentifier = "list_labels"
    static let title: LocalizedStringResource = "List Labels"
    static let description = IntentDescription(
        "Every label.",
        categoryName: "Tasks",
        searchKeywords: ["labels", "tags"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[LabelEntity]> & ProvidesDialog {
        let listed = try store.assist(Self.persistentIdentifier)
        guard case let .rows(rows) = listed else { return .result(value: [], dialog: IntentDialog(said: listed.said)) }
        return .result(value: rows.rows.filter { $0.role == "label" }.map(LabelEntity.init), dialog: IntentDialog(said: listed.said))
    }
}

struct ListSavedFiltersIntent: OperationIntent {
    static let persistentIdentifier = "list_filters"
    static let title: LocalizedStringResource = "List Saved Filters"
    static let description = IntentDescription(
        "Every saved filter, with its query.",
        categoryName: "Tasks",
        searchKeywords: ["filters", "views"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[SavedFilterEntity]> & ProvidesDialog {
        let listed = try store.assist(Self.persistentIdentifier)
        guard case let .filters(filters) = listed else { return .result(value: [], dialog: IntentDialog(said: listed.said)) }
        return .result(value: filters.filters.map(SavedFilterEntity.init), dialog: IntentDialog(said: listed.said))
    }
}

struct AddTaskIntent: OperationIntent {
    static let persistentIdentifier = "add_task"
    static let title: LocalizedStringResource = "Add a Task"
    static let description = IntentDescription(
        "Adds a task (or, from a line starting with `block`, a time block) from one quick-add line, and says how it read it.",
        categoryName: "Tasks",
        searchKeywords: ["new", "create", "remember", "capture", "remind"]
    )

    @Parameter(title: "Text", description: "A quick-add line, read as a person's would be: the title, then any of a due date or time (\"tomorrow at 3pm\", \"every weekday\"), a priority (p1 to p4), labels (@calls), a project (#Work, #\"Home Office\"), an estimate (~30m). A line starting with `block` makes a time block instead (\"block Deep work 9-11am weekdays\"). The answer says how it was read.")
    var text: String

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<TaskEntity?> & ProvidesDialog {
        let added = try store.assist(Self.persistentIdentifier, ["text": .text(text)])
        return .result(value: store.entity(added.task), dialog: IntentDialog(said: added.said))
    }
}

struct EditTaskIntent: OperationIntent {
    static let persistentIdentifier = "edit_task"
    static let title: LocalizedStringResource = "Edit a Task"
    static let description = IntentDescription(
        "Changes a task's fields: only those given.",
        categoryName: "Tasks",
        searchKeywords: ["change", "rename", "reschedule", "due date"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity
    @Parameter(title: "Title", description: "A new title.")
    var title: String?
    @Parameter(title: "Due", description: "A date phrase (\"next monday\", \"in 3 days at 9am\"), or `none` to clear it.")
    var due: String?
    @Parameter(title: "Repeat", description: "A repetition (\"every monday\", \"every! 2 weeks\"), or `none` to stop it repeating.")
    var `repeat`: String?
    @Parameter(title: "Priority", description: "1 to 4, where 1 is highest.", inclusiveRange: (1, 4))
    var priority: Int?
    @Parameter(title: "Estimate", description: "How long it should take (\"45m\", \"1h30m\"), or `none`.")
    var estimate: String?
    @Parameter(title: "Notes", description: "Its notes, whole.")
    var notes: String?
    @Parameter(title: "Project", description: "The project to move it to, by name.")
    var project: ProjectEntity?
    @Parameter(title: "Labels", description: "Its labels, all of them, by name.")
    var labels: [LabelEntity]?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, [
            "id": .text(id.id),
            "title": title.map(ArgumentValue.text),
            "due": due.map(ArgumentValue.text),
            "repeat": `repeat`.map(ArgumentValue.text),
            "priority": priority.map { .number(Int64($0)) },
            "estimate": estimate.map(ArgumentValue.text),
            "notes": notes.map(ArgumentValue.text),
            "project": project.map { .text($0.name) },
            "labels": labels.map { .texts($0.map(\.name)) },
        ]))
    }
}

struct CompleteTaskIntent: OperationIntent {
    static let persistentIdentifier = "complete_task"
    static let title: LocalizedStringResource = "Complete a Task"
    static let description = IntentDescription(
        "Marks a task done. A repeating one moves on to its next date.",
        categoryName: "Tasks",
        searchKeywords: ["done", "finish", "check off", "tick"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["id": .text(id.id)]))
    }
}

struct UncompleteTaskIntent: OperationIntent {
    static let persistentIdentifier = "uncomplete_task"
    static let title: LocalizedStringResource = "Mark a Task Not Done"
    static let description = IntentDescription(
        "Marks a done task not done.",
        categoryName: "Tasks",
        searchKeywords: ["undone", "reopen"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["id": .text(id.id)]))
    }
}

struct MoveTaskIntent: OperationIntent {
    static let persistentIdentifier = "move_task"
    static let title: LocalizedStringResource = "Move a Task"
    static let description = IntentDescription(
        "Makes a task a subtask of another, moves it to a project, or makes it a task of its own.",
        categoryName: "Tasks",
        searchKeywords: ["subtask", "project", "move"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity
    @Parameter(title: "Parent", description: "A task to make it a subtask of, by identifier.")
    var parent: TaskEntity?
    @Parameter(title: "Project", description: "A project to move it to, by name.")
    var project: ProjectEntity?
    @Parameter(title: "Top", description: "True to make it a task of its own again, under no other.", default: false)
    var top: Bool

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, [
            "id": .text(id.id),
            "parent": parent.map { .text($0.id) },
            "project": project.map { .text($0.name) },
            "top": top ? .flag(true) : nil,
        ]))
    }
}

// MARK: - The trash

struct TrashTaskIntent: OperationIntent {
    static let persistentIdentifier = "trash_task"
    static let title: LocalizedStringResource = "Move a Task to the Trash"
    static let description = IntentDescription(
        "Moves a task to the trash, from which `restore_task` brings it back.",
        categoryName: "The Trash",
        searchKeywords: ["delete", "remove", "trash"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["id": .text(id.id)]))
    }
}

struct RestoreTaskIntent: OperationIntent {
    static let persistentIdentifier = "restore_task"
    static let title: LocalizedStringResource = "Restore a Task"
    static let description = IntentDescription(
        "Brings a task back from the trash.",
        categoryName: "The Trash",
        searchKeywords: ["undelete", "bring back", "restore"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["id": .text(id.id)]))
    }
}

extension OperationIntent {
    /// Runs an operation that asks first: puts `question` to the person (else the question the
    /// core refuses with, unconfirmed) and runs it with `confirm` only once they say yes.
    func confirmed(_ store: AssistantStore, _ arguments: [String: ArgumentValue?], asking question: String?) async throws -> LumennaApp.Assisted {
        var asking = question
        if asking == nil {
            do {
                return try store.assist(Self.persistentIdentifier, arguments)
            } catch let refusal as AssistantRefusal {
                asking = refusal.sentence
            }
        }
        if let asking { try await askFirst(asking) }
        return try store.assist(Self.persistentIdentifier, arguments.merging(["confirm": .flag(true)]) { $1 })
    }
}

struct EraseTaskPermanentlyIntent: OperationIntent {
    static let persistentIdentifier = "erase_task_permanently"
    static let title: LocalizedStringResource = "Erase a Task Permanently"
    static let description = IntentDescription(
        "Erases a task in the trash permanently: from the history every device keeps, and with it the history of everything deleted before. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.",
        categoryName: "The Trash",
        searchKeywords: ["erase", "permanently", "forever"]
    )

    @Parameter(title: "Task", description: "The identifier of a task in the trash.")
    var id: TaskEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        // The question every client asks before this action, the core's.
        let action = try store.task(id.id)?.actions.first { $0.kind == .erasePermanently }
        var question: String?
        if case let .confirm(title, message, _) = action?.question { question = "\(title) \(message)" }
        return said(try await confirmed(store, ["id": .text(id.id)], asking: question))
    }
}

struct EraseEverythingDeletedIntent: OperationIntent {
    static let persistentIdentifier = "erase_everything_deleted"
    static let title: LocalizedStringResource = "Erase Everything Deleted"
    static let description = IntentDescription(
        "Erases everything deleted permanently: every task in the trash, and the history of everything ever deleted, on every device. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.",
        categoryName: "The Trash",
        searchKeywords: ["erase", "empty the trash", "permanently"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try await confirmed(store, [:], asking: nil))
    }
}

// MARK: - The day

struct ShowDayIntent: OperationIntent {
    static let persistentIdentifier = "day"
    static let title: LocalizedStringResource = "Show the Day"
    static let description = IntentDescription(
        "A day as it is lived: its blocks in time order, each with the tasks in it (its sittings), the free time between them, and where now falls.",
        categoryName: "The Day",
        searchKeywords: ["today", "schedule", "agenda", "blocks", "plan"]
    )

    @Parameter(title: "Date", description: "The day, as a person says it (\"tomorrow\", \"friday\", \"2026-11-03\"); today if left out.")
    var date: String?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[BlockEntity]> & ProvidesDialog {
        let day = try store.assist(Self.persistentIdentifier, ["date": date.map(ArgumentValue.text)])
        guard case let .plan(plan) = day else { return .result(value: [], dialog: IntentDialog(said: day.said)) }
        return .result(value: plan.blockEntities, dialog: IntentDialog(said: day.said))
    }
}

struct ListBlocksIntent: OperationIntent {
    static let persistentIdentifier = "list_blocks"
    static let title: LocalizedStringResource = "List Blocks"
    static let description = IntentDescription(
        "Every time block, each with when it happens and how often.",
        categoryName: "The Day",
        searchKeywords: ["blocks", "routine", "schedule"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier))
    }
}

struct AssignIntent: OperationIntent {
    static let persistentIdentifier = "assign"
    static let title: LocalizedStringResource = "Put a Task in a Block"
    static let description = IntentDescription(
        "Puts a task in a block, as a sitting: one session of work on it there.",
        categoryName: "The Day",
        searchKeywords: ["schedule", "block", "time block", "plan"]
    )

    @Parameter(title: "Task", description: "The task's identifier.")
    var task: TaskEntity
    @Parameter(title: "Block", description: "The block's identifier, as the day lists it.")
    var block: BlockEntity
    @Parameter(title: "Date", description: "The day, for a block that repeats; the block's own day otherwise.")
    var date: String?
    @Parameter(title: "Minutes", description: "How long the sitting is planned to take, in minutes.")
    var minutes: Int?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, [
            "task": .text(task.id),
            "block": .text(block.id),
            "date": date.map(ArgumentValue.text),
            "minutes": minutes.map { .number(Int64($0)) },
        ]))
    }
}

struct UnassignIntent: OperationIntent {
    static let persistentIdentifier = "unassign"
    static let title: LocalizedStringResource = "Take a Task out of a Block"
    static let description = IntentDescription(
        "Takes a sitting out of its block; the task itself stays.",
        categoryName: "The Day",
        searchKeywords: ["unschedule", "remove from block"]
    )

    @Parameter(title: "Sitting", description: "The sitting's identifier: a task's place in a block, as the day lists it.")
    var assignment: SittingEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["assignment": .text(assignment.id)]))
    }
}

// MARK: - Timers

struct StartTimerIntent: OperationIntent {
    static let persistentIdentifier = "start_timer"
    static let title: LocalizedStringResource = "Start a Timer"
    static let description = IntentDescription(
        "Starts, or resumes, a sitting's timer.",
        categoryName: "Timers",
        searchKeywords: ["start", "begin", "work on", "resume"]
    )

    @Parameter(title: "Sitting", description: "The sitting's identifier: a task's place in a block, as the day lists it.")
    var assignment: SittingEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["assignment": .text(assignment.id)]))
    }
}

struct PauseTimerIntent: OperationIntent {
    static let persistentIdentifier = "pause_timer"
    static let title: LocalizedStringResource = "Pause a Timer"
    static let description = IntentDescription(
        "Pauses a sitting's timer, keeping what it has logged.",
        categoryName: "Timers",
        searchKeywords: ["pause", "break"]
    )

    @Parameter(title: "Sitting", description: "The sitting's identifier: a task's place in a block, as the day lists it.", optionsProvider: RunningSitting())
    var assignment: SittingEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["assignment": .text(assignment.id)]))
    }
}

struct StopTimerIntent: OperationIntent {
    static let persistentIdentifier = "stop_timer"
    static let title: LocalizedStringResource = "Stop a Timer"
    static let description = IntentDescription(
        "Stops a sitting's timer and logs its time.",
        categoryName: "Timers",
        searchKeywords: ["stop", "finish", "log time"]
    )

    @Parameter(title: "Sitting", description: "The sitting's identifier: a task's place in a block, as the day lists it.", optionsProvider: RunningSitting())
    var assignment: SittingEntity
    @Parameter(title: "Minutes", description: "The minutes to log instead of the timer's, when it ran on by mistake.")
    var minutes: Int?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, [
            "assignment": .text(assignment.id),
            "minutes": minutes.map { .number(Int64($0)) },
        ]))
    }
}

// MARK: - Planning

struct WhatShouldIWorkOnIntent: OperationIntent {
    static let persistentIdentifier = "what_should_i_work_on"
    static let title: LocalizedStringResource = "What Should I Work On?"
    static let description = IntentDescription(
        "What to work on now, ranked, each choice with the reason it is offered: in a block under way its own unfinished sittings, then its suggestions; in free time, what fits before the next block. Starting on one is `start_on`.",
        categoryName: "Planning",
        searchKeywords: ["next", "focus", "now", "suggest"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[ChoiceEntity]> & ProvidesDialog {
        let offered = try store.assist(Self.persistentIdentifier)
        guard case let .choices(choices) = offered else { return .result(value: [], dialog: IntentDialog(said: offered.said)) }
        return .result(value: choices.choices.map(ChoiceEntity.init), dialog: IntentDialog(said: offered.said))
    }
}

struct StartOnIntent: OperationIntent {
    static let persistentIdentifier = "start_on"
    static let title: LocalizedStringResource = "Start on a Choice"
    static let description = IntentDescription(
        "Starts on one of What Should I Work On?'s choices at once, its timer running: in the block under way, or in a block of its own in free time.",
        categoryName: "Planning",
        searchKeywords: ["start", "work on", "focus"]
    )

    @Parameter(title: "Choice", description: "The `id` of one of the choices What Should I Work On? offered.")
    var choice: ChoiceEntity

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier, ["choice": .text(choice.id)]))
    }
}

struct ProposeReplanIntent: OperationIntent {
    static let persistentIdentifier = "propose_replan"
    static let title: LocalizedStringResource = "Propose a Replan of Today"
    static let description = IntentDescription(
        "Proposes how to replan the rest of today, when the day has slipped: every change as a whole sentence. Nothing changes until `apply_replan` is given the ones the person accepted; say them to the person first.",
        categoryName: "Planning",
        searchKeywords: ["replan", "behind", "reschedule", "running late"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[ReplanChangeEntity]> & ProvidesDialog {
        let proposed = try store.assist(Self.persistentIdentifier)
        guard case let .replan(replan) = proposed else { return .result(value: [], dialog: IntentDialog(said: proposed.said)) }
        return .result(value: replan.changes.map(ReplanChangeEntity.init), dialog: IntentDialog(said: replan.said))
    }
}

struct ProposeDayPlanIntent: OperationIntent {
    static let persistentIdentifier = "propose_day_plan"
    static let title: LocalizedStringResource = "Propose a Plan for a Day"
    static let description = IntentDescription(
        "Proposes which tasks to put in each of a day's work blocks still to come, each with why. Nothing changes until `apply_day_plan` is given the sittings the person accepted.",
        categoryName: "Planning",
        searchKeywords: ["plan the day", "fill", "schedule"]
    )

    @Parameter(title: "Date", description: "The day, as a person says it (\"tomorrow\", \"friday\", \"2026-11-03\"); today if left out.")
    var date: String?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ReturnsValue<[ProposedSittingEntity]> & ProvidesDialog {
        let proposed = try store.assist(Self.persistentIdentifier, ["date": date.map(ArgumentValue.text)])
        guard case let .dayPlan(plan) = proposed else { return .result(value: [], dialog: IntentDialog(said: proposed.said)) }
        return .result(value: plan.sittings.map(ProposedSittingEntity.init), dialog: IntentDialog(said: plan.said))
    }
}

struct ApplyReplanIntent: OperationIntent {
    static let persistentIdentifier = "apply_replan"
    static let title: LocalizedStringResource = "Apply a Replan"
    static let description = IntentDescription(
        "Makes the replan changes the person accepted, as one change they can undo, and proposes again around what they turned down.",
        categoryName: "Planning",
        searchKeywords: ["replan", "accept"]
    )

    /// Left out, every change proposed. The proposal itself is the one the core makes now,
    /// which a person hears before anything changes.
    @Parameter(title: "Accepted", description: "Those of its `changes` the person accepted, each as it was proposed.")
    var accepted: [ReplanChangeEntity]?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        let proposed = try store.assist("propose_replan")
        guard case let .replan(proposal) = proposed else { return .result(dialog: IntentDialog(said: proposed.said)) }
        let changes = accepted.map { $0.map(\.change) } ?? proposal.changes
        guard !changes.isEmpty else { return .result(dialog: IntentDialog(said: proposal.said)) }
        try await askFirst(changes == proposal.changes ? proposal.said : changes.map(\.sentence).joined(separator: "\n"))
        let applied = try store.assist(Self.persistentIdentifier, ["proposal": .replan(proposal), "accepted": .replanChanges(changes)])
        return .result(dialog: IntentDialog(said: applied.said))
    }
}

struct ApplyDayPlanIntent: OperationIntent {
    static let persistentIdentifier = "apply_day_plan"
    static let title: LocalizedStringResource = "Apply a Day Plan"
    static let description = IntentDescription(
        "Puts the sittings the person accepted into their blocks, as one change they can undo, and plans again around what they turned down.",
        categoryName: "Planning",
        searchKeywords: ["plan", "accept"]
    )

    /// The day whose plan the core proposes now, as Propose a Plan for a Day takes it.
    @Parameter(title: "Date", description: "The day, as a person says it (\"tomorrow\", \"friday\", \"2026-11-03\"); today if left out.")
    var date: String?
    /// Left out, every sitting proposed.
    @Parameter(title: "Accepted", description: "Those of its `sittings` the person accepted, each as it was proposed.")
    var accepted: [ProposedSittingEntity]?

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        let proposed = try store.assist("propose_day_plan", ["date": (date ?? accepted?.first?.day).map(ArgumentValue.text)])
        guard case let .dayPlan(proposal) = proposed else { return .result(dialog: IntentDialog(said: proposed.said)) }
        let sittings = accepted.map { $0.map(\.sitting) } ?? proposal.sittings
        guard !sittings.isEmpty else { return .result(dialog: IntentDialog(said: proposal.said)) }
        var asking = proposal
        asking.sittings = sittings
        try await askFirst(sittings == proposal.sittings ? proposal.said : asking.said)
        let applied = try store.assist(Self.persistentIdentifier, ["proposal": .dayPlan(proposal), "accepted": .dayPlanSittings(sittings)])
        return .result(dialog: IntentDialog(said: applied.said))
    }
}

// MARK: - Undo

struct UndoIntent: OperationIntent {
    static let persistentIdentifier = "undo"
    static let title: LocalizedStringResource = "Undo"
    static let description = IntentDescription(
        "Undoes the last change made on this device, whoever made it: an app, `lum`, or an assistant.",
        categoryName: "Undo",
        searchKeywords: ["undo", "take back"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier))
    }
}

struct RedoIntent: OperationIntent {
    static let persistentIdentifier = "redo"
    static let title: LocalizedStringResource = "Redo"
    static let description = IntentDescription(
        "Redoes what was last undone.",
        categoryName: "Undo",
        searchKeywords: ["redo"]
    )

    @AppDependency private var store: AssistantStore

    func perform() async throws -> some IntentResult & ProvidesDialog {
        said(try store.assist(Self.persistentIdentifier))
    }
}
