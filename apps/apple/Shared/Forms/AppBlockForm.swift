import LumennaApp
import SwiftUI

/// The block form the app has open (`ViewModel.blockForm`): a new block, a block changed for
/// every occurrence, or one day of it. Which fields it shows and in what words, what a kind
/// brings with it, a start that follows the day chosen until the person sets one, and what
/// saving sends — only what changed — are all the app's; each field goes to it as the person
/// changes it. The form closes when the app lets it go.
///
/// The surface's `BlockFormModel` stays for the screens not yet moved onto the app, and goes
/// with them.
final class AppBlockFormModel: ObservableObject {
    let core: Core
    /// Closes the form; set by whoever presents it, since SwiftUI's `dismiss` does not reach
    /// a UIKit or AppKit sheet reliably.
    var close: () -> Void = {}

    /// The form's fields, the app's, in order: what it shows.
    @Published private(set) var shown: [LumennaApp.FormField] = []
    @Published private(set) var heading = ""
    /// The button that saves: "Add" or "Save".
    @Published private(set) var yes = "Save"
    @Published var fields: LumennaApp.BlockFields {
        didSet { typed(from: oldValue) }
    }
    @Published var start: Date {
        didSet { if !following { send("start", Self.clock(start)) } }
    }
    @Published var minutes: Int {
        didSet { if !following { send("minutes", String(minutes)) } }
    }
    @Published var day: Date {
        didSet { if !following { send("date", Clock.isoDay(day)) } }
    }
    @Published var failure: String?

    /// Whether the fields are being set from the app's, not typed.
    private var following = false
    /// Whether the app's values are waiting for the app to be still.
    private var filling = false
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        let form = core.app.view.blockForm
        let values = form?.values ?? LumennaApp.BlockFields(
            title: "", start: "09:00", minutes: "60", kind: "work", acceptsTasks: true, countsCapacity: true,
            anchored: false, repeat: "", until: "", minMinutes: "", taskFilter: "", colour: "", notes: ""
        )
        fields = values
        start = Self.date(values.start)
        minutes = Int(values.minutes) ?? 60
        day = form.flatMap { Self.day($0.date) } ?? .now
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.follow()
        }
        follow()
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// The field `key` as the app words it, if the form shows it.
    func field(_ key: String) -> LumennaApp.FormField? {
        shown.first { $0.key == key }
    }

    func shows(_ key: String) -> Bool { field(key) != nil }

    /// The kind, which brings its own flags with it, the app's.
    var kind: String {
        get { fields.kind }
        set { fields.kind = newValue }
    }

    /// The app's form, into this one: its fields, and its values unless typing is still on its
    /// way. Gone, it closes.
    private func follow() {
        guard let form = core.app.view.blockForm else {
            close()
            return
        }
        shown = form.fields
        heading = form.heading
        yes = form.yes
        failure = form.refusal
        guard !filling else { return }
        filling = true
        core.app.whenStill { [weak self] in
            self?.filling = false
            self?.fill()
        }
    }

    /// The app's values, into the form's, once nothing is on its way.
    private func fill() {
        guard let form = core.app.view.blockForm else { return }
        following = true
        defer { following = false }
        if fields != form.values { fields = form.values }
        let start = Self.date(form.values.start)
        if Self.clock(self.start) != form.values.start { self.start = start }
        if let minutes = Int(form.values.minutes), minutes != self.minutes { self.minutes = minutes }
        if let day = Self.day(form.date), !Calendar.current.isDate(day, inSameDayAs: self.day) { self.day = day }
    }

    /// What changed in `fields`, each to the app by its key in the core's block form.
    private func typed(from old: LumennaApp.BlockFields) {
        guard !following else { return }
        let new = fields
        let changes: [(String, String, String)] = [
            ("title", old.title, new.title),
            ("kind", old.kind, new.kind),
            ("accepts_tasks", String(old.acceptsTasks), String(new.acceptsTasks)),
            ("counts_capacity", String(old.countsCapacity), String(new.countsCapacity)),
            ("anchored", String(old.anchored), String(new.anchored)),
            ("repeat", old.repeat, new.repeat),
            ("until", old.until, new.until),
            ("min_minutes", old.minMinutes, new.minMinutes),
            ("task_filter", old.taskFilter, new.taskFilter),
            ("colour", old.colour, new.colour),
            ("notes", old.notes, new.notes),
        ]
        for (key, before, after) in changes where before != after {
            send(key, after)
        }
    }

    private func send(_ key: String, _ value: String) {
        core.app.send(.block(.field(key: key, value: value)))
    }

    /// Saves what changed; the app closes the form, or says why not and keeps it open.
    func save() {
        core.app.send(.block(.save))
    }

    /// Puts the form away, making nothing.
    func cancel() {
        core.app.send(.block(.cancel))
    }

    private static func date(_ clock: String) -> Date {
        let parts = clock.split(separator: ":").compactMap { Int($0) }
        return Calendar.current.date(
            bySettingHour: parts.first ?? 0, minute: parts.count > 1 ? parts[1] : 0, second: 0, of: .now
        ) ?? .now
    }

    private static func clock(_ date: Date) -> String {
        let parts = Calendar.current.dateComponents([.hour, .minute], from: date)
        return String(format: "%02d:%02d", parts.hour ?? 0, parts.minute ?? 0)
    }

    private static func day(_ iso: String) -> Date? {
        iso.isEmpty ? nil : try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse(iso)
    }
}

struct AppBlockForm: View {
    @ObservedObject var model: AppBlockFormModel

    var body: some View {
        Form {
            Section {
                if let title = model.field("title") {
                    namedField(title.label, text: $model.fields.title, example: title.example).modifier(FieldHint(title.hint))
                }
                if let date = model.field("date") {
                    NamedDatePicker(date.label, selection: $model.day, displayedComponents: .date, hint: date.hint)
                }
                if let start = model.field("start") {
                    NamedDatePicker(start.label, selection: $model.start, displayedComponents: .hourAndMinute)
                }
                if model.shows("minutes") { lasts }
            } header: {
                #if os(macOS)
                // A sheet has no title bar to say what it is for.
                FormParts.heading(model.heading)
                #endif
            }
            if let kind = model.field("kind") {
                ChoiceSection(
                    kind.label,
                    selection: $model.kind,
                    choices: kind.options.map { ($0.title, $0.id) },
                    footer: kind.hint
                )
            }
            if let repetition = model.field("repeat") {
                Section {
                    namedField(repetition.label, text: $model.fields.repeat, example: repetition.example)
                        #if os(iOS) || os(watchOS)
                        .textInputAutocapitalization(.never)
                        #endif
                } footer: {
                    // The core's hint, with the rule the words cannot say where there is one.
                    FormParts.caption(repetition.hint)
                }
            }
            Section {
                toggle("accepts_tasks", $model.fields.acceptsTasks)
                toggle("counts_capacity", $model.fields.countsCapacity)
                toggle("anchored", $model.fields.anchored)
            }
            if more.contains(where: model.shows) {
                Section {
                    text("until", $model.fields.until)
                    text("min_minutes", $model.fields.minMinutes)
                    text("task_filter", $model.fields.taskFilter)
                    text("colour", $model.fields.colour)
                    text("notes", $model.fields.notes, axis: .vertical)
                } header: {
                    FormParts.heading("More")
                } footer: {
                    // What the fields take, seen as well as heard: the core's sentences, in
                    // the fields' order.
                    FormParts.caption(
                        ["until", "min_minutes", "task_filter"].compactMap { model.field($0)?.hint }.joined(separator: " ")
                    )
                }
                .autocorrectionDisabled()
            }
            #if os(macOS)
            HStack {
                Spacer()
                Button("Cancel") { model.cancel() }
                    .keyboardShortcut(.cancelAction)
                Button(model.yes) { model.save() }
                    .keyboardShortcut(.defaultAction)
            }
            #elseif os(watchOS)
            // A watch's sheet has its own close button, and no bar button for Save.
            Button(model.yes) { model.save() }
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

    /// A text field for one of the app's fields, if the form shows it.
    @ViewBuilder private func text(_ key: String, _ value: Binding<String>, axis: Axis = .horizontal) -> some View {
        if let field = model.field(key) {
            namedField(field.label, text: value, example: field.example, axis: axis).modifier(FieldHint(field.hint))
        }
    }

    /// A toggle for one of the app's on-or-off fields, if the form shows it.
    @ViewBuilder private func toggle(_ key: String, _ isOn: Binding<Bool>) -> some View {
        if let field = model.field(key) {
            Labelled(field.label) { Toggle(field.label, isOn: isOn) }
        }
    }

    /// How long it lasts: a stepper in fives on the phone and the watch, where typing a number
    /// is the slow way; minutes to type, with a stepper beside them, on the Mac.
    @ViewBuilder private var lasts: some View {
        #if os(iOS) || os(watchOS)
        // The stepper says what it stands for, the length in words, rather than the core's
        // name for a field of minutes to type.
        Stepper(value: $model.minutes, in: 5...720, step: 5) {
            Text("Lasts \(Clock.length(UInt32(model.minutes)))")
        }
        #else
        if let field = model.field("minutes") {
            Named(field.label) {
                HStack {
                    TextField(field.label, value: $model.minutes, format: .number)
                        .frame(width: 70)
                    LengthStepper(minutes: $model.minutes)
                    Text(Clock.length(UInt32(max(model.minutes, 0)))).foregroundStyle(Color.quietLabel)
                }
            }
        }
        #endif
    }
}
