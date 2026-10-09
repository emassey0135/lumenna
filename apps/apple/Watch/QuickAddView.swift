import SwiftUI

/// Quick add: one line, read back by the core before anything is saved, as on every app.
/// Dictation, Scribble and the keyboard all come with the text field.
struct QuickAddView: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    @State private var text: String

    /// `prefix` starts the line, as on the phone: the place's project or label, there to be
    /// seen and changed.
    init(prefix: String) {
        _text = State(initialValue: prefix)
    }

    var body: some View {
        let readback = preview
        ScrollView {
            VStack(alignment: .leading) {
                TextField("Task", text: $text)
                if let readback {
                    // What will be saved, before anything is: a misheard date is caught here.
                    Text(readback.announcement.prefix(1).uppercased() + readback.announcement.dropFirst())
                        .font(.footnote)
                    ForEach(readback.notices, id: \.self) { notice in
                        Text(notice).font(.footnote).foregroundStyle(.secondary)
                    }
                }
                Button("Add") {
                    if core.act({ try core.lumenna.addTask(text: line) }) { dismiss() }
                }
                .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .navigationTitle("New Task")
    }

    private var line: String { text }

    private var preview: Preview? {
        guard !text.trimmingCharacters(in: .whitespaces).isEmpty else { return nil }
        return try? core.lumenna.previewTask(text: line)
    }
}
