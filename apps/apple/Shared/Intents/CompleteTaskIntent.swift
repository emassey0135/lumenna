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

    @AppDependency private var store: AssistantStore

    init() {}

    /// Completing `task`: a widget's Mark Done.
    init(task: TaskEntity) {
        id = task
    }

    func perform() async throws -> some IntentResult & ReturnsValue<TaskEntity?> & ProvidesDialog {
        #if LUMENNA_WIDGETS && os(macOS) && !APP_STORE
        // The direct download's widget: its store is outside the widget's sandbox, so the request
        // is handed to the app, resident in the menu bar, through the group (`WidgetRequests`).
        // The app takes it within a second, or at its next start, and the widget reads the glance
        // it writes then; nothing is said, since a widget's button says nothing.
        guard let container = AppGroup.container else { throw CocoaError(.fileNoSuchFile) }
        try WidgetRequests.hand(.init(operation: Self.persistentIdentifier, arguments: ["id": id.id]), into: container)
        return .result(value: id, dialog: IntentDialog(said: ""))
        #else
        let changed = try store.assist(Self.persistentIdentifier, ["id": .text(id.id)])
        return .result(value: store.entity(changed.task), dialog: IntentDialog(said: changed.said))
        #endif
    }
}
