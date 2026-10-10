import SwiftUI

/// The block editor: a new block, or a change to one — every occurrence, or one day,
/// as the person already chose. The model and the form the Apple apps share; each shows it
/// its own way, iOS as a navigation sheet, macOS as a window sheet, watchOS as a sheet.
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

    /// What the repetition field means: the core's hint, unless the block repeats by a rule
    /// the field cannot show, which is said with what leaving it empty does.
    var repetitionHelp: String {
        if !isAdding, let rule {
            return "It repeats by the rule \(rule), which this cannot show in words. Empty keeps it; \u{201C}none\u{201D} makes it happen once."
        }
        return FormWords.block["repeat"].hint
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
    /// Each field's name, hint, example and options, the core's, as in every app.
    private let words = FormWords.block

    var body: some View {
        Form {
            Section {
                namedField(words["title"], text: $model.fields.title)
                if model.isAdding {
                    Labelled(words["date"].label) {
                        DatePicker(words["date"].label, selection: $model.day, displayedComponents: .date)
                            .modifier(FieldHint(words["date"].hint))
                    }
                }
                Labelled(words["start"].label) {
                    DatePicker(words["start"].label, selection: $model.start, displayedComponents: .hourAndMinute)
                }
                lasts
            } header: {
                #if os(macOS)
                // A sheet has no title bar to say what it is for.
                FormParts.heading(model.heading)
                #endif
            }
            ChoiceSection(
                words["kind"].label,
                selection: $model.kind,
                choices: words["kind"].options.map { ($0.title, $0.id) },
                footer: words["kind"].hint
            )
            if shows("repeat") {
                Section {
                    namedField(words["repeat"].label, text: $model.fields.repeat, example: words["repeat"].example)
                        #if os(iOS) || os(watchOS)
                        .textInputAutocapitalization(.never)
                        #endif
                } footer: {
                    FormParts.caption(model.repetitionHelp)
                }
            }
            Section {
                toggle("accepts_tasks", $model.fields.acceptsTasks)
                toggle("counts_capacity", $model.fields.countsCapacity)
                toggle("anchored", $model.fields.anchored)
            }
            if more.contains(where: shows) {
                Section {
                    if model.asksUntil, shows("until") {
                        namedField(words["until"], text: $model.fields.until)
                    }
                    if shows("min_minutes") { namedField(words["min_minutes"], text: $model.fields.minMinutes) }
                    if shows("task_filter") { namedField(words["task_filter"], text: $model.fields.taskFilter) }
                    if shows("colour") { namedField(words["colour"], text: $model.fields.colour) }
                    if shows("notes") { namedField(words["notes"], text: $model.fields.notes, axis: .vertical) }
                } header: {
                    FormParts.heading("More")
                } footer: {
                    // What the fields take, seen as well as heard: the core's sentences, in
                    // the fields' order.
                    FormParts.caption(
                        ["until", "min_minutes", "task_filter"]
                            .filter { ($0 != "until" || model.asksUntil) && shows($0) }
                            .map { words[$0].hint }
                            .joined(separator: " ")
                    )
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
            #elseif os(watchOS)
            // A watch's sheet has its own close button, and no bar button for Save.
            Button("Save") { model.save() }
            #endif
        }
        #if os(macOS)
        .formStyle(.grouped)
        // A grouped form scrolls, so in a sheet it would otherwise ask for no height at all.
        .frame(width: 480, height: 640)
        #endif
        .modifier(FailureAlert(failure: $model.failure))
    }

    /// The fields under More.
    private let more = ["until", "min_minutes", "task_filter", "colour", "notes"]

    /// Whether the field `key` is shown: every field for a series or a new block; for one day
    /// of a repeating block, only those the core marks as one day's own.
    private func shows(_ key: String) -> Bool {
        !model.isOneDay || words[key].oneDay
    }

    /// A toggle for one of the core's on-or-off fields, by its key.
    private func toggle(_ key: String, _ isOn: Binding<Bool>) -> some View {
        Labelled(words[key].label) { Toggle(words[key].label, isOn: isOn) }
    }

    /// How long it lasts: a stepper in fives on the phone, where typing a number is the slow
    /// way; minutes to type, with a stepper beside them, on the Mac.
    @ViewBuilder private var lasts: some View {
        #if os(iOS) || os(watchOS)
        // The stepper says what it stands for, the length in words, rather than the core's
        // name for a field of minutes to type.
        Stepper(value: $model.minutes, in: 5...720, step: 5) {
            Text("Lasts \(Clock.length(UInt32(model.minutes)))")
        }
        #else
        Named(words["minutes"].label) {
            HStack {
                TextField(words["minutes"].label, value: $model.minutes, format: .number)
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
