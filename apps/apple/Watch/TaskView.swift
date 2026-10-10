import SwiftUI

/// One task, in the form the iPhone and the Mac edit it with (`Shared/Forms/TaskForm.swift`):
/// every field, saved as only what changed, then what can be done to it, the core's. What
/// only the watch does is ask their questions, as sheets.
struct TaskView: View {
    @EnvironmentObject private var core: WatchCore
    let id: String

    var body: some View {
        Hosted(core: core, id: id)
    }

    /// The model is made once the store is in hand, which a view's initialiser does not have.
    private struct Hosted: View {
        @StateObject private var model: TaskDetailModel
        @StateObject private var asker: WatchAsker
        @State private var host: TaskSheets
        @Environment(\.dismiss) private var dismiss

        init(core: WatchCore, id: String) {
            _model = StateObject(wrappedValue: TaskDetailModel(core: core, id: id))
            let asker = WatchAsker(core: core)
            _asker = StateObject(wrappedValue: asker)
            _host = State(initialValue: TaskSheets(asker: asker))
        }

        var body: some View {
            TaskDetailView(model: model)
                .navigationTitle("Task")
                .onAppear {
                    model.host = host
                    host.left = { dismiss() }
                }
                .sheet(item: $asker.asked) { asked in
                    NavigationStack { asked.view }
                }
        }
    }
}

/// The watch's answers to what the task form asks of its platform.
final class TaskSheets: TaskFormHost {
    let asker: ActionAsking
    var left: () -> Void = {}

    init(asker: WatchAsker) {
        self.asker = asker
    }

    func trashed() {
        left()
    }
}

/// A length in minutes, from the lengths a sitting usually takes, or none.
struct LengthChoice: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    let without: String?
    let chosen: (UInt32?) -> Void

    var body: some View {
        List {
            ForEach([15, 25, 30, 45, 60, 90, 120], id: \.self) { minutes in
                Button(Clock.length(UInt32(minutes))) {
                    dismiss()
                    chosen(UInt32(minutes))
                }
            }
            if let without {
                Button(without) {
                    dismiss()
                    chosen(nil)
                }
            }
        }
        .navigationTitle(title)
    }
}
