import SwiftUI

/// The block editor: a new block, or a change to one — every occurrence, or one day,
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
    /// A rule the date grammar cannot say, shown beside an empty Repeats field; saving with
    /// the field still empty keeps it.
    let rule: String?
    /// Whether the block repeats, which is when it can have a last day.
    let repeats: Bool
    /// The fields as the form opened, which saving compares against (`blockEdit`).
    private let initial: BlockFields
    private let core: Core
    /// Told of the change once the form has saved and closed.
    var saved: (Change) -> Void
    /// Closes the form; set by whoever presents it, since SwiftUI's `dismiss` does not reach
    /// a UIKit or AppKit sheet reliably.
    var close: () -> Void = {}

    /// Every field but the start and length, as the core shapes them.
    @Published var fields: BlockFields
    @Published var start: Date
    @Published var minutes: Int
    @Published var day: Date
    @Published var failure: String?

    init(
        core: Core,
        purpose: Purpose,
        fields: BlockFields? = nil,
        start: String = "09:00",
        minutes: Int = 60,
        day: Date = .now,
        rule: String? = nil,
        repeats: Bool = false,
        saved: @escaping (Change) -> Void = { _ in }
    ) {
        self.core = core
        self.purpose = purpose
        self.rule = rule
        self.repeats = repeats
        self.saved = saved
        let defaults = blockDefaults(kind: "work")
        let fields = fields ?? BlockFields(
            title: "", start: start, minutes: String(minutes), kind: "work",
            acceptsTasks: defaults?.acceptsTasks ?? true, countsCapacity: defaults?.countsCapacity ?? true,
            anchored: defaults?.anchored ?? false, repeat: "", until: "", minMinutes: "", taskFilter: "",
            colour: "", notes: ""
        )
        initial = fields
        self.fields = fields
        self.start = Self.date(fields.start)
        self.minutes = Int(fields.minutes) ?? minutes
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
            fields: blockFields(block: shown),
            rule: shown.repetition == nil ? shown.rrule : nil,
            repeats: shown.repeats,
            saved: saved
        )
    }

    /// The form for one day of a series, from how that day stands.
    static func occurrence(core: Core, block: PlanBlock, day: String, saved: @escaping (Change) -> Void = { _ in }) -> BlockFormModel {
        BlockFormModel(core: core, purpose: .occurrence(block.series, day: day), fields: dayBlockFields(block: block), saved: saved)
    }

    var isAdding: Bool {
        if case .add = purpose { return true }
        return false
    }

    var isOneDay: Bool {
        if case .occurrence = purpose { return true }
        return false
    }

    /// The kind, which brings its own flags with it: a work block takes tasks, a break does
    /// not, an event is anchored. The flags can then be set apart from it.
    var kind: String {
        get { fields.kind }
        set {
            fields.kind = newValue
            if let defaults = blockDefaults(kind: newValue) {
                fields.acceptsTasks = defaults.acceptsTasks
                fields.countsCapacity = defaults.countsCapacity
                fields.anchored = defaults.anchored
            }
        }
    }

    /// What the repetition field means, which differs between adding and changing.
    var repetitionHelp: String {
        if isAdding {
            return "Such as \u{201C}every weekday\u{201D}. Empty for a block that happens once."
        }
        if let rule {
            return "It repeats by the rule \(rule), which this cannot show in words. Empty keeps it; \u{201C}none\u{201D} makes it happen once."
        }
        if initial.repeat.isEmpty {
            return "It happens once now. Such as \u{201C}every weekday\u{201D} to make it repeat."
        }
        return "Empty makes it happen once."
    }

    /// Whether it can have a last day: a block that repeats, or one being added to repeat.
    var asksUntil: Bool {
        !isOneDay && (repeats || (isAdding && !fields.repeat.trimmingCharacters(in: .whitespaces).isEmpty))
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

    /// The fields as edited, with the start and length the pickers hold.
    private var current: BlockFields {
        var current = fields
        current.start = Self.clock(start)
        current.minutes = String(minutes)
        return current
    }

    /// Saves, closes and tells `saved`; or says why not and stays open. What a block is made
    /// from, and which fields a change sends, are the core's (`newBlock`, `blockEdit`), as for
    /// every app: only what changed, so an edit made elsewhere to another field stands.
    func save() {
        guard !fields.title.trimmingCharacters(in: .whitespaces).isEmpty else {
            failure = "A block needs a name."
            return
        }
        do {
            let change: Change
            switch purpose {
            case .add:
                change = try core.lumenna.addBlock(block: newBlock(fields: current, date: Clock.isoDay(day)))
            case let .series(id), let .occurrence(id, _):
                guard let edit = try blockEdit(before: initial, after: current) else {
                    close()
                    Announcer.say("Nothing changed")
                    return
                }
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
                namedField("Name", text: $model.fields.title, example: "Deep work")
                if model.isAdding {
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
            if !model.isOneDay {
                Section {
                    namedField("Repeats", text: $model.fields.repeat, example: "every weekday")
                        #if os(iOS)
                        .textInputAutocapitalization(.never)
                        #endif
                } footer: {
                    FormParts.caption(model.repetitionHelp)
                }
            }
            Section {
                Labelled("Takes tasks") { Toggle("Takes tasks", isOn: $model.fields.acceptsTasks) }
                Labelled("Counts toward hours for work") { Toggle("Counts toward hours for work", isOn: $model.fields.countsCapacity) }
                Labelled("Anchored, never moved when the day slips") { Toggle("Anchored, never moved when the day slips", isOn: $model.fields.anchored) }
            } header: {
                FormParts.heading("What it does")
            } footer: {
                FormParts.caption("The kind sets these; change any of them to set it apart.")
            }
            if !model.isOneDay {
                Section {
                    if model.asksUntil {
                        namedField("Until", text: $model.fields.until, example: "31 January")
                    }
                    namedField("Shortest length, in minutes", text: $model.fields.minMinutes, example: "30")
                    namedField("Tasks from", text: $model.fields.taskFilter, example: "#Work")
                    namedField("Colour", text: $model.fields.colour, example: "teal")
                    namedField("Notes", text: $model.fields.notes, example: "Anything else", axis: .vertical)
                } header: {
                    FormParts.heading("More")
                } footer: {
                    FormParts.caption("Until is its last day. The shortest length is how far a slipping day may shorten it; empty for the kind's own. Tasks from is a filter for which tasks it is meant for.")
                }
                .autocorrectionDisabled()
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
        .frame(width: 480, height: 640)
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
                LengthStepper(minutes: $model.minutes)
                Text(Clock.length(UInt32(max(model.minutes, 0)))).foregroundStyle(Color.quietLabel)
            }
        }
        #endif
    }
}

#if os(macOS)
/// The length's stepper, telling VoiceOver the length it stands for. AppKit's says its number,
/// which VoiceOver reads as a percentage of its range, and the text beside it is not read
/// again when it changes; SwiftUI's own accessibility value on it is ignored.
private struct LengthStepper: NSViewRepresentable {
    @Binding var minutes: Int

    func makeNSView(context: Context) -> Spoken {
        let stepper = Spoken()
        stepper.minValue = 1
        stepper.maxValue = 720
        stepper.increment = 5
        stepper.valueWraps = false
        stepper.target = context.coordinator
        stepper.action = #selector(Coordinator.stepped(_:))
        stepper.setAccessibilityLabel("Length")
        return stepper
    }

    func updateNSView(_ stepper: Spoken, context: Context) {
        context.coordinator.minutes = $minutes
        stepper.integerValue = minutes
        stepper.spoken = Clock.length(UInt32(max(minutes, 0)))
    }

    func makeCoordinator() -> Coordinator { Coordinator(minutes: $minutes) }

    final class Coordinator: NSObject {
        var minutes: Binding<Int>
        init(minutes: Binding<Int>) { self.minutes = minutes }
        @objc func stepped(_ stepper: NSStepper) { minutes.wrappedValue = stepper.integerValue }
    }

    final class Spoken: NSStepper {
        var spoken = "" {
            didSet { if spoken != oldValue { NSAccessibility.post(element: self, notification: .valueChanged) } }
        }

        override func accessibilityValue() -> Any? { spoken }
    }
}
#endif
