import LumennaApp
import SwiftUI

/// The words of completing a line, one place for every Apple app.
enum CompletionWords {
    /// The line's action and button that open the list.
    static let show = "Show Completions"
    /// The list's title.
    static let title = "Completions"
    /// What the list says when nothing fits.
    static let nothing = "Nothing completes here."
}

#if !os(watchOS)
/// A line in the quick-add or filter language, completing two ways (ROADMAP: both, on the phones
/// and the Mac alike): a strip of what fits beneath it, which changes as one types and is quick to
/// check, and a list with clear edges, which the field's Show Completions action, the button, or
/// Down on a keyboard opens. Nothing opens by itself. The core says what fits, at the end of
/// the line: a SwiftUI field does not say where its cursor is.
struct CompletingLine<Field: View>: View {
    let app: AppModel
    let syntax: LumennaApp.Syntax
    @Binding var text: String
    @ViewBuilder let field: () -> Field
    @State private var listing = false

    private var offered: LumennaApp.Completions? {
        text.isEmpty ? nil : try? app.complete(text, cursor: UInt32(text.utf8.count), syntax: syntax)
    }

    var body: some View {
        let offered = offered
        VStack(alignment: .leading, spacing: 6) {
            field()
                .accessibilityAction(named: CompletionWords.show) { listing = true }
                .onKeyPress(.downArrow) {
                    listing = true
                    return .handled
                }
            if let offered, !offered.candidates.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack {
                        ForEach(offered.candidates, id: \.text) { candidate in
                            Button(candidate.text) { insert(candidate, at: offered) }
                                // "Work, project", not "#Work": the sigil is punctuation speech may skip.
                                .accessibilityLabel(LumennaApp.candidateLine(candidate: candidate))
                        }
                        Button(CompletionWords.show) { listing = true }
                    }
                    .buttonStyle(.bordered)
                }
            }
        }
        #if os(macOS)
        // The Mac's own pick sheet, whose search field keeps focus while the arrows say each
        // choice: a SwiftUI sheet's hosting view is a nameless group the audit finds.
        .onChange(of: listing) { _, open in
            guard open else { return }
            listing = false
            guard let window = NSApp.keyWindow, let offered else { return }
            let items = offered.candidates.enumerated().map { PickerItem(key: String($0.offset), title: LumennaApp.candidateLine(candidate: $0.element)) }
            PickerSheet.present(on: window, title: CompletionWords.title, items: items, yes: "Choose", chosen: { item in
                if let at = Int(item.key), at < offered.candidates.count { insert(offered.candidates[at], at: offered) }
            })
        }
        #else
        .sheet(isPresented: $listing) {
            CompletionList(offered: offered) { candidate in
                listing = false
                if let candidate, let offered { insert(candidate, at: offered) }
            }
        }
        #endif
    }

    private func insert(_ candidate: LumennaApp.Candidate, at offered: LumennaApp.Completions) {
        text = CompletionText.insert(candidate.text, into: text, start: offered.start, end: offered.end).text
    }
}

/// What fits, as a list to choose from: each the core's line ("Work, project"), Cancel beside.
struct CompletionList: View {
    let offered: LumennaApp.Completions?
    /// The one chosen, or nil for Cancel.
    let chosen: (LumennaApp.Candidate?) -> Void

    var body: some View {
        NavigationStack {
            List {
                if let offered, !offered.candidates.isEmpty {
                    ForEach(offered.candidates, id: \.text) { candidate in
                        Button(LumennaApp.candidateLine(candidate: candidate)) { chosen(candidate) }
                    }
                } else {
                    Text(CompletionWords.nothing)
                }
            }
            // Named, or the audit finds a nameless list on the Mac.
            .accessibilityLabel(CompletionWords.title)
            .navigationTitle(CompletionWords.title)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { chosen(nil) } }
            }
        }
        #if os(macOS)
        .frame(minWidth: 320, minHeight: 240)
        // The sheet's group too, which the Mac's hosting view is.
        .accessibilityElement(children: .contain)
        .accessibilityLabel(CompletionWords.title)
        #endif
        .tint(Color.lumennaTint)
    }
}
#endif
