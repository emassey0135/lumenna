import AppIntents
import Foundation
import SwiftUI
#if !os(watchOS)
import CoreSpotlight
#endif

#if !os(watchOS)
/// A task in Spotlight, where Siri and Apple Intelligence find it too, as the core's search
/// document has it (`Device.searchDocuments`): its name, the line under it, the words it is
/// found by, and when it is due.
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

/// The open tasks, the core's search documents, kept in Spotlight as the store changes: indexed as the app launches, and
/// again a few seconds after a change, however many changes came meanwhile. What is no longer
/// open leaves the index. The watch has no Spotlight.
@available(iOS 18.0, macOS 15.0, *)
enum TaskIndex {
    private static let queue = DispatchQueue(label: "Lumenna task index", qos: .utility)
    /// Whether an indexing is waiting to run; only touched on `queue`.
    private static var waiting = false
    /// What this process has indexed; nothing until the first indexing, which starts afresh.
    private static var indexed: Set<String>?

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
                let documents = store.documents()
                let tasks = documents.keys.compactMap { id in store.entity(try? store.task(id), documents) }
                let before = indexed
                indexed = Set(tasks.map(\.id))
                let index = CSSearchableIndex.default()
                Task {
                    if let before {
                        let gone = Array(before.subtracting(tasks.map(\.id)))
                        if !gone.isEmpty { try? await index.deleteAppEntities(identifiedBy: gone, ofType: TaskEntity.self) }
                    } else {
                        try? await index.deleteAppEntities(ofType: TaskEntity.self)
                    }
                    try? await index.indexAppEntities(tasks)
                }
                // Siri's phrases that name a task ("Complete … in Lumenna") learn the new ones.
                LumennaShortcuts.updateAppShortcutParameters()
            }
        }
    }
}
#endif

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
