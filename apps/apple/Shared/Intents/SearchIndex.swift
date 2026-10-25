import AppIntents
import Foundation
import LumennaApp
import SwiftUI
#if !os(watchOS)
import CoreSpotlight
#endif

// What Spotlight, Siri and Apple Intelligence find of Lumenna's is the core's search
// (`records::search`), as on every platform: every search document indexed as its entity, its
// attributes the document's, and a result chosen opened by its launch.

#if !os(watchOS)
extension LumennaApp.SearchDocument {
    /// Spotlight's attributes as the document has them: its name, the line under it, the words
    /// it is found by, and when it is due.
    func attributes(over attributes: CSSearchableItemAttributeSet) -> CSSearchableItemAttributeSet {
        attributes.title = title
        attributes.contentDescription = line.isEmpty ? nil : line
        attributes.keywords = keywords
        if let due {
            attributes.dueDate = try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse(due)
        }
        return attributes
    }
}

@available(iOS 18.0, macOS 15.0, *)
extension TaskEntity: IndexedEntity {
    var attributeSet: CSSearchableItemAttributeSet {
        let attributes = defaultAttributeSet
        attributes.title = title
        attributes.contentDescription = line
        attributes.keywords = keywords
        attributes.dueDate = due
        return attributes
    }
}

@available(iOS 18.0, macOS 15.0, *)
extension ProjectEntity: IndexedEntity {
    var attributeSet: CSSearchableItemAttributeSet {
        document.map { $0.attributes(over: defaultAttributeSet) } ?? defaultAttributeSet
    }
}

@available(iOS 18.0, macOS 15.0, *)
extension LabelEntity: IndexedEntity {
    var attributeSet: CSSearchableItemAttributeSet {
        document.map { $0.attributes(over: defaultAttributeSet) } ?? defaultAttributeSet
    }
}

@available(iOS 18.0, macOS 15.0, *)
extension SavedFilterEntity: IndexedEntity {
    var attributeSet: CSSearchableItemAttributeSet {
        document.map { $0.attributes(over: defaultAttributeSet) } ?? defaultAttributeSet
    }
}

@available(iOS 18.0, macOS 15.0, *)
extension BlockSeriesEntity: IndexedEntity {
    var attributeSet: CSSearchableItemAttributeSet {
        document.attributes(over: defaultAttributeSet)
    }
}

/// The core's search documents, kept in Spotlight as the store changes: indexed as the app
/// launches, and again a few seconds after a change, however many changes came meanwhile. What
/// is no longer a document (a task done, a project archived) leaves the index. The watch has
/// no Spotlight.
@available(iOS 18.0, macOS 15.0, *)
enum SearchIndex {
    private static let queue = DispatchQueue(label: "Lumenna search index", qos: .utility)
    /// Whether an indexing is waiting to run; only touched on `queue`.
    private static var waiting = false
    /// The indexing running, or the last to run, which the next waits for; only touched on
    /// `queue`.
    private static var running: Task<Void, Never>?
    /// What this process has indexed of each kind of entity; none until its first indexing,
    /// which starts afresh. Only an indexing touches it, and one runs at a time.
    private static var indexed: [String: Set<String>] = [:]

    static func follow(_ store: AssistantStore) {
        index(store, after: 0)
        NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: nil) { _ in
            index(store, after: 5)
        }
    }

    private static func index(_ store: AssistantStore, after delay: TimeInterval) {
        queue.async {
            guard !waiting else { return }
            waiting = true
            queue.asyncAfter(deadline: .now() + delay) {
                waiting = false
                let documents = store.searchDocuments()
                let ofTasks = store.documents()
                let tasks = ofTasks.keys.compactMap { id in store.entity(try? store.task(id), ofTasks) }
                let projects = rows(store, "list_projects", "project").map { ProjectEntity($0, found: documents) }
                let labels = rows(store, "list_labels", "label").map { LabelEntity($0, found: documents) }
                var filters: [SavedFilterEntity] = []
                if case let .filters(listed) = try? store.assist("list_filters") {
                    filters = listed.filters.map { SavedFilterEntity($0, found: documents) }
                }
                let blocks = documents.filter { $0.kind == .block }.map(BlockSeriesEntity.init)
                let index = CSSearchableIndex.default()
                // Once the one before is done, however long Spotlight took over it: two at once
                // wrote `indexed` together, and the app crashed in its setter (an iPad on CI, its
                // index full from a suite's worth of tests, slow enough for the next to overlap).
                let before = running
                running = Task {
                    await before?.value
                    await replace(tasks, in: index)
                    // Only what the core indexes: the Inbox is a project no search offers.
                    await replace(projects.filter { $0.document != nil }, in: index)
                    await replace(labels.filter { $0.document != nil }, in: index)
                    await replace(filters.filter { $0.document != nil }, in: index)
                    await replace(blocks, in: index)
                }
                // Siri's phrases that name a task ("Complete … in Lumenna") learn the new ones.
                LumennaShortcuts.updateAppShortcutParameters()
            }
        }
    }

    private static func rows(_ store: AssistantStore, _ operation: String, _ role: String) -> [LumennaApp.RowView] {
        guard case let .rows(listed) = try? store.assist(operation) else { return [] }
        return listed.rows.filter { $0.role == role }
    }

    /// Indexes `entities` in place of what was indexed of their kind before.
    private static func replace<Entity: IndexedEntity>(_ entities: [Entity], in index: CSSearchableIndex) async where Entity.ID == String {
        let kind = String(describing: Entity.self)
        let now = Set(entities.map(\.id))
        if let before = indexed[kind] {
            let gone = Array(before.subtracting(now))
            if !gone.isEmpty { try? await index.deleteAppEntities(identifiedBy: gone, ofType: Entity.self) }
        } else {
            try? await index.deleteAppEntities(ofType: Entity.self)
        }
        indexed[kind] = now
        try? await index.indexAppEntities(entities)
    }
}
#endif

// MARK: - Opening a result

// Each opens what was chosen (in Spotlight, Siri or Shortcuts) by its search document's launch,
// handed to the app (`ShellEvent.launch`), which shows it.

struct OpenTaskIntent: OpenIntent {
    static let title: LocalizedStringResource = "Open Task"
    static let openAppWhenRun = true

    @Parameter(title: "Task")
    var target: TaskEntity

    @AppDependency private var launcher: AssistantLauncher

    @MainActor
    func perform() async throws -> some IntentResult {
        if let launch = target.launch { launcher.open(launch) }
        return .result()
    }
}

struct OpenProjectIntent: OpenIntent {
    static let title: LocalizedStringResource = "Open Project"
    static let openAppWhenRun = true

    @Parameter(title: "Project")
    var target: ProjectEntity

    @AppDependency private var launcher: AssistantLauncher

    @MainActor
    func perform() async throws -> some IntentResult {
        if let launch = target.document?.launch { launcher.open(launch) }
        return .result()
    }
}

struct OpenLabelIntent: OpenIntent {
    static let title: LocalizedStringResource = "Open Label"
    static let openAppWhenRun = true

    @Parameter(title: "Label")
    var target: LabelEntity

    @AppDependency private var launcher: AssistantLauncher

    @MainActor
    func perform() async throws -> some IntentResult {
        if let launch = target.document?.launch { launcher.open(launch) }
        return .result()
    }
}

struct OpenSavedFilterIntent: OpenIntent {
    static let title: LocalizedStringResource = "Open Saved Filter"
    static let openAppWhenRun = true

    @Parameter(title: "Saved Filter")
    var target: SavedFilterEntity

    @AppDependency private var launcher: AssistantLauncher

    @MainActor
    func perform() async throws -> some IntentResult {
        if let launch = target.document?.launch { launcher.open(launch) }
        return .result()
    }
}

struct OpenBlockIntent: OpenIntent {
    static let title: LocalizedStringResource = "Open Block"
    static let openAppWhenRun = true

    @Parameter(title: "Block")
    var target: BlockSeriesEntity

    @AppDependency private var launcher: AssistantLauncher

    @MainActor
    func perform() async throws -> some IntentResult {
        launcher.open(target.document.launch)
        return .result()
    }
}

// MARK: - On the screen

/// Tells Siri and Apple Intelligence which task is on the screen, so "this task" means it: the
/// current user activity, carrying the task's entity, while its details show.
private struct ShowsTask: ViewModifier {
    let id: String
    let title: String
    @State private var activity: NSUserActivity?

    func body(content: Content) -> some View {
        content
            .onAppear(perform: show)
            .onChange(of: id, show)
            .onDisappear {
                activity?.resignCurrent()
                activity = nil
            }
    }

    private func show() {
        let activity = self.activity ?? NSUserActivity(activityType: "dev.emassey0135.lumenna.task")
        activity.title = title
        if #available(iOS 18.2, macOS 15.2, watchOS 11.2, *) {
            activity.appEntityIdentifier = EntityIdentifier(for: TaskEntity.self, identifier: id)
        }
        activity.becomeCurrent()
        self.activity = activity
    }
}

extension View {
    /// Says this view shows the task `id`, for Siri and Apple Intelligence.
    func showingTask(_ id: String, titled title: String) -> some View {
        modifier(ShowsTask(id: id, title: title))
    }
}
