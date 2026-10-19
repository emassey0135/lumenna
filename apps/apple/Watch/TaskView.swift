import SwiftUI

/// One task, in the form the iPhone and the Mac edit it with (`Shared/Forms/TaskForm.swift`):
/// every field, saved as only what changed, then what can be done to it, the core's, its
/// questions asked by the root as sheets.
struct TaskView: View {
    @EnvironmentObject private var core: WatchCore
    let id: String

    var body: some View {
        Hosted(core: core, id: id)
    }

    /// The model is made once the store is in hand, which a view's initialiser does not have.
    private struct Hosted: View {
        @StateObject private var model: TaskDetailModel
        @State private var host = TaskSheets()
        @Environment(\.dismiss) private var dismiss

        init(core: WatchCore, id: String) {
            _model = StateObject(wrappedValue: TaskDetailModel(core: core, id: id))
        }

        var body: some View {
            // The task's actions' questions are the root's sheets (`AppQuestions`).
            TaskDetailView(model: model)
                .navigationTitle("Task")
                .onAppear {
                    model.host = host
                    host.left = { dismiss() }
                    model.show()
                }
        }
    }
}

/// The watch's answers to what the task form asks of its platform.
final class TaskSheets: TaskFormHost {
    var left: () -> Void = {}

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
