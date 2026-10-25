import AppIntents
import Foundation
import LumennaApp

// Complete a Task, apart from the other operations' intents (`AssistantIntents.swift`) since the
// widgets compile it too, for Today's Mark Done (`Widgets/`), and nothing else of them.

/// An intent that is one of the assistant's operations, its `persistentIdentifier` the
/// operation's `id`.
protocol OperationIntent: AppIntent {}

struct CompleteTaskIntent: OperationIntent {
    static let persistentIdentifier = "complete_task"
    static let title: LocalizedStringResource = "Complete a Task"
    static let description: IntentDescription? = IntentDescription(
        "Marks a task done. A repeating one moves on to its next date.",
        categoryName: "Tasks",
        searchKeywords: ["done", "finish", "check off", "tick"]
    )

    @Parameter(title: "Task", description: "The task's identifier: a whole UUID, or a prefix that names one task.")
    var id: TaskEntity

    #if LUMENNA_WIDGETS
    /// The widget's copy, which runs its Mark Done: Shortcuts and Siri list the app's.
    static let isDiscoverable = false
    #endif

    #if os(macOS) && !APP_STORE
    /// In the direct download the app, never the widget: its store is outside the sandbox the
    /// widget runs in, and the app, resident in the menu bar, holds it.
    @available(macOS 27.0, *)
    static var allowedExecutionTargets: ExecutionTargets { .main }
    #endif

    @AppDependency private var store: AssistantStore

    init() {}

    /// Completing `task`: a widget's Mark Done.
    init(task: TaskEntity) {
        id = task
    }

    func perform() async throws -> some IntentResult & ReturnsValue<TaskEntity?> & ProvidesDialog {
        let changed = try store.assist(Self.persistentIdentifier, ["id": .text(id.id)])
        return .result(value: store.entity(changed.task), dialog: IntentDialog(said: changed.said))
    }
}
