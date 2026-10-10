import SwiftUI

/// What could go at the end of a line the core reads — a project, a label, a date word —
/// offered once the line is entered, as the BTSpeak app offers it: on a watch the line is
/// typed or dictated on the system's own input screen, which takes no suggestions. Choosing
/// one puts it in place of what it completes.
struct CompletionOffer: View {
    @EnvironmentObject private var core: WatchCore
    @Binding var text: String
    let syntax: Syntax

    var body: some View {
        if let offered, !offered.candidates.isEmpty {
            Section {
                ForEach(offered.candidates, id: \.text) { candidate in
                    // "project Work", not "#Work": the sigil is punctuation VoiceOver may skip.
                    Button(candidate.text) { text = Self.insert(candidate, into: text, at: offered) }
                        .accessibilityLabel(candidate.label)
                }
            } header: {
                FormParts.heading(offered.announcement.prefix(1).uppercased() + offered.announcement.dropFirst())
            }
        }
    }

    private var offered: Completions? {
        guard !text.isEmpty else { return nil }
        return try? core.lumenna.completeText(text: text, cursor: UInt32(text.utf8.count), syntax: syntax)
    }

    /// The candidate in place of the bytes it replaces, with a space after it so the next
    /// word can follow.
    static func insert(_ candidate: Candidate, into text: String, at offered: Completions) -> String {
        let bytes = Array(text.utf8)
        let start = min(Int(offered.start), bytes.count)
        let end = min(max(Int(offered.end), start), bytes.count)
        let before = String(decoding: bytes[..<start], as: UTF8.self)
        let after = String(decoding: bytes[end...], as: UTF8.self)
        return before + candidate.text + (after.hasPrefix(" ") ? "" : " ") + after
    }
}

/// A line of text asked for, as the phone's prompts ask: what for, then the field, then
/// Save. With `syntax`, the core's completions follow the field.
struct TextPrompt: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    var message: String?
    var placeholder = ""
    var action = "Save"
    var syntax: Syntax?
    let done: (String) -> Void
    @State private var text: String

    init(
        _ title: String, message: String? = nil, initial: String = "", placeholder: String = "",
        action: String = "Save", syntax: Syntax? = nil, done: @escaping (String) -> Void
    ) {
        self.title = title
        self.message = message
        self.placeholder = placeholder
        self.action = action
        self.syntax = syntax
        self.done = done
        _text = State(initialValue: initial)
    }

    var body: some View {
        List {
            if let message {
                Text(message).font(.footnote)
            }
            // Named explicitly: with text in it, a field's title gives way to the text.
            TextField(title, text: $text, prompt: placeholder.isEmpty ? nil : example(placeholder))
                .textInputAutocapitalization(.never)
                .accessibilityLabel(title)
            if let syntax {
                CompletionOffer(text: $text, syntax: syntax)
            }
            Button(action) {
                dismiss()
                done(text.trimmingCharacters(in: .whitespaces))
            }
            .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
        }
        .navigationTitle(title)
    }
}

/// A choice among several, each a button, as the phone's action sheets offer them.
struct ChoicePrompt: View {
    @Environment(\.dismiss) private var dismiss
    let title: String
    var message: String?
    let choices: [(String, () -> Void)]

    var body: some View {
        List {
            if let message {
                Text(message).font(.footnote)
            }
            ForEach(Array(choices.enumerated()), id: \.offset) { _, choice in
                Button(choice.0) {
                    dismiss()
                    choice.1()
                }
            }
        }
        .navigationTitle(title)
    }
}

/// A sheet to show: a prompt or a choice, built when it is asked for.
struct Asked: Identifiable {
    let id = UUID()
    let view: AnyView

    init(_ view: some View) {
        self.view = AnyView(view)
    }
}
