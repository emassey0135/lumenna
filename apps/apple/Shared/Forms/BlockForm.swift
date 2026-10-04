import SwiftUI

/// The block editor (§16.1): a new block, or a change to one — every occurrence, or one day,
/// as the person already chose. The model and the form both Apple apps share; each shows it
/// its own way, iOS as a navigation sheet and macOS as a window sheet.
final class BlockFormModel: ObservableObject {
    enum Purpose {
        case add
        /// Every occurrence of the series with this identifier.
        case series(String)
        /// One day of it.
        case occurrence(String, day: String)
    }

    let purpose: Purpose
    let heading: String
    /// Whether it repeats by a rule the date grammar cannot say, which the repetition field
    /// then leaves alone unless something is typed into it.
    let unspeakableRule: Bool
    private let initial: (name: String, start: Date, minutes: Int, kind: String, repeat: String)
    private let core: Core
    /// Told of the change once the form has saved and closed.
    var saved: (Change) -> Void
    /// Closes the form; set by whoever presents it, since SwiftUI's `dismiss` does not reach
    /// a UIKit or AppKit sheet reliably.
    var close: () -> Void = {}

    @Published var name: String
    @Published var start: Date
    @Published var minutes: Int
    @Published var kind: String
    @Published var repetition: String
    @Published var day: Date
    @Published var failure: String?

    init(
        core: Core,
        purpose: Purpose,
        name: String = "",
        start: String = "09:00",
        minutes: Int = 60,
        kind: String = "work",
        repetition: String = "",
        day: Date = .now,
        unspeakableRule: Bool = false,
        saved: @escaping (Change) -> Void = { _ in }
    ) {
        self.core = core
        self.purpose = purpose
        self.unspeakableRule = unspeakableRule
        self.saved = saved
        let startDate = Self.date(start)
        initial = (name, startDate, minutes, kind, repetition)
        self.name = name
        self.start = startDate
        self.minutes = minutes
        self.kind = kind
        self.repetition = repetition
        self.day = day
        switch purpose {
        case .add: heading = "New Block"
        case .series: heading = "Every Occurrence"
        case let .occurrence(_, day): heading = Clock.spokenDay(day) + " Only"
        }
    }

    /// The form for every occurrence of a series, filled from the store.
    static func series(core: Core, id: String, saved: @escaping (Change) -> Void = { _ in }) throws -> BlockFormModel {
        let shown = try core.lumenna.showBlock(id: id)
        return BlockFormModel(
            core: core,
            purpose: .series(shown.id),
            name: shown.title,
            start: shown.start,
            minutes: Int(shown.minutes),
            kind: shown.kind,
            repetition: shown.repetition ?? "",
            unspeakableRule: shown.repeats && shown.repetition == nil,
            saved: saved
        )
    }

    var asksRepetition: Bool {
        if case .occurrence = purpose { return false }
        return true
    }

    var asksDay: Bool {
        if case .add = purpose { return true }
        return false
    }

    /// What the repetition field means, which differs between adding and changing.
    var repetitionHelp: String {
        if case .add = purpose {
            return "Such as \u{201C}every weekday\u{201D}. Empty for a block that happens once."
        }
        if unspeakableRule {
            return "It repeats by a rule this cannot show in words. Empty keeps it; \u{201C}none\u{201D} makes it happen once."
        }
        if initial.repeat.isEmpty {
            return "It happens once now. Such as \u{201C}every weekday\u{201D} to make it repeat."
        }
        return "Empty makes it happen once."
    }

    /// The repetition to send: nothing if it is as it was, `none` if it was cleared.
    private var repetitionChange: String? {
        let typed = repetition.trimmingCharacters(in: .whitespaces)
        guard asksRepetition, typed != initial.repeat else { return nil }
        if typed.isEmpty {
            return unspeakableRule ? nil : "none"
        }
        return typed
    }

    private static func date(_ clock: String) -> Date {
        let parts = clock.split(separator: ":").compactMap { Int($0) }
        return Calendar.current.date(
            bySettingHour: parts.first ?? 9, minute: parts.count > 1 ? parts[1] : 0, second: 0, of: .now
        ) ?? .now
    }

    private static func clock(_ date: Date) -> String {
        let parts = Calendar.current.dateComponents([.hour, .minute], from: date)
        return String(format: "%02d:%02d", parts.hour ?? 0, parts.minute ?? 0)
    }

    /// Saves, closes and tells `saved`; or says why not and stays open.
    func save() {
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty else {
            failure = "A block needs a name."
            return
        }
        guard minutes > 0 else {
            failure = "A block has to last at least a minute."
            return
        }
        let at = Self.clock(start)
        do {
            let change: Change
            switch purpose {
            case .add:
                change = try core.lumenna.addBlock(block: NewBlock(
                    title: name, at: at, minutes: UInt32(minutes), date: Clock.isoDay(day), kind: kind,
                    repeat: repetition.isEmpty ? nil : repetition
                ))
            case let .series(id), let .occurrence(id, _):
                // Only what changed, so a field edited on another device meanwhile is not
                // overwritten with what this form happened to show.
                let edit = BlockEdit(
                    title: name != initial.name ? name : nil,
                    at: at != Self.clock(initial.start) ? at : nil,
                    minutes: minutes != initial.minutes ? UInt32(minutes) : nil,
                    kind: kind != initial.kind ? kind : nil,
                    repeat: repetitionChange
                )
                let scope: BlockScope = if case let .occurrence(_, day) = purpose { .occurrence(date: day) } else { .series }
                change = try core.lumenna.editBlock(id: id, edit: edit, scope: scope)
            }
            close()
            saved(change)
        } catch {
            failure = error.sentence
        }
    }
}

struct BlockForm: View {
    @ObservedObject var model: BlockFormModel

    var body: some View {
        Form {
            Section {
                namedField("Name", text: $model.name, example: "Deep work")
                if model.asksDay {
                    Labelled("Day") { DatePicker("Day", selection: $model.day, displayedComponents: .date) }
                }
                Labelled("Starts") { DatePicker("Starts", selection: $model.start, displayedComponents: .hourAndMinute) }
                lasts
            } header: {
                #if os(macOS)
                // A sheet has no title bar to say what it is for.
                FormParts.heading(model.heading)
                #endif
            }
            ChoiceSection("Kind", selection: $model.kind, choices: [
                ("Work, takes tasks", "work"), ("Break", "break"), ("Event", "event"),
            ])
            if model.asksRepetition {
                Section {
                    namedField("Repeats", text: $model.repetition, example: "every weekday")
                        #if os(iOS)
                        .textInputAutocapitalization(.never)
                        #endif
                } footer: {
                    FormParts.caption(model.repetitionHelp)
                }
            }
            #if os(macOS)
            HStack {
                Spacer()
                Button("Cancel") { model.close() }
                    .keyboardShortcut(.cancelAction)
                Button("Save") { model.save() }
                    .keyboardShortcut(.defaultAction)
            }
            #endif
        }
        #if os(macOS)
        .formStyle(.grouped)
        // A grouped form scrolls, so in a sheet it would otherwise ask for no height at all.
        .frame(width: 460, height: 500)
        #endif
        .modifier(FailureAlert(failure: $model.failure))
    }

    /// How long it lasts: a stepper in fives on the phone, where typing a number is the slow
    /// way; minutes to type, with a stepper beside them, on the Mac.
    @ViewBuilder private var lasts: some View {
        #if os(iOS)
        Stepper(value: $model.minutes, in: 5...720, step: 5) {
            Text("Lasts \(Clock.length(UInt32(model.minutes)))")
        }
        #else
        Named("Lasts, in minutes") {
            HStack {
                TextField("Lasts, in minutes", value: $model.minutes, format: .number)
                    .frame(width: 70)
                Stepper("Lasts, in minutes", value: $model.minutes, in: 1...720, step: 5)
                    .labelsHidden()
                Text(Clock.length(UInt32(max(model.minutes, 0)))).foregroundStyle(Color.quietLabel)
            }
        }
        #endif
    }
}
