import LumennaApp
import SwiftUI

/// Quick add: the app's quick-add line (`ViewModel.line`), read back by the core before
/// anything is saved, as on every app. Dictation, Scribble and the keyboard all come with the
/// text field.
struct QuickAddView: View {
    @EnvironmentObject private var core: Core
    @Environment(\.dismiss) private var dismiss
    /// Whether the line starts empty, whatever the place shown adds: from the places.
    var fresh = false
    @State private var text = ""
    /// Whether the line was added or put away, so it is put away once.
    @State private var closed = false
    @State private var refused: String?

    var body: some View {
        _ = core.generation
        let line = core.app.view.line
        return List {
            // Named explicitly: with text in it, a field's title gives way to the text.
            TextField(QuickAddText.label, text: $text)
                .accessibilityLabel(QuickAddText.label)
                .onChange(of: text) { _, text in typed(text) }
            // After the line is entered, what could finish its last word, beside it.
            CompletionOffer(text: $text, syntax: .quickAdd)
            if let refused {
                Text(refused).font(.footnote).foregroundStyle(Color.warningLabel)
            } else if let readback = line?.readback, !blank {
                // What will be saved, before anything is, the errors with it: a misheard date
                // is caught here.
                Text(readback).font(.footnote)
            }
            Button(QuickAddText.add) { add() }
                .disabled(blank)
        }
        .navigationTitle(QuickAddText.title)
        .onAppear {
            // The line opens with what the place shown adds, there to be seen and changed.
            core.app.send(.typing(.open))
            text = fresh ? "" : core.app.current.view.line?.text ?? ""
            typed(text)
        }
        .onDisappear {
            guard !closed else { return }
            closed = true
            core.app.send(.typing(.close))
        }
    }

    private var blank: Bool { text.trimmingCharacters(in: .whitespaces).isEmpty }

    private func typed(_ text: String) {
        refused = nil
        core.app.send(.typing(.text(text: text, cursor: UInt32(text.utf8.count))))
    }

    /// The app adds it and says what it added; a line it refuses stays here with why.
    private func add() {
        core.app.send(.quickAdd(text))
        if let refusal = core.app.current.view.addRefused {
            refused = refusal
            Announcer.say(refusal)
            return
        }
        closed = true
        dismiss()
    }
}
