import AppKit
import SwiftUI

/// The block editor (§16.1): a new block, or a change to one — every occurrence, or one day,
/// as the person already chose. Shown as a sheet.
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
    /// Whether it repeats by a rule the grammar cannot say, which the field then leaves alone.
    let unspeakableRule: Bool
    private let initial: (name: String, start: Date, minutes: Int, kind: String, repeat: String)
    private let core: Core
    var saved: (Change) -> Void = { _ in }
    var close: () -> Void = {}

    @Published var name: String
    @Published var start: Date
    @Published var minutes: Int
    @Published var kind: String
    @Published var repetition: String
    @Published var day: Date
    @Published var failure: String?

    init(
        core: Core, purpose: Purpose, name: String = "", start: String = "09:00", minutes: Int = 60,
        kind: String = "work", repetition: String = "", day: Date = .now, unspeakableRule: Bool = false
    ) {
        self.core = core
        self.purpose = purpose
        self.unspeakableRule = unspeakableRule
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
        case .series: heading = "Every Occurrence of \(name)"
        case let .occurrence(_, day): heading = "\(name), \(Clock.spokenDay(day)) Only"
        }
    }

    /// The form for every occurrence of a series, filled from the store.
    static func series(core: Core, id: String) throws -> BlockFormModel {
        let shown = try core.lumenna.showBlock(id: id)
        return BlockFormModel(
            core: core, purpose: .series(shown.id), name: shown.title, start: shown.start,
            minutes: Int(shown.minutes), kind: shown.kind, repetition: shown.repetition ?? "",
            unspeakableRule: shown.repeats && shown.repetition == nil
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

    var repetitionHelp: String {
        if case .add = purpose { return "Such as every weekday. Empty for a block that happens once." }
        if unspeakableRule { return "It repeats by a rule this cannot show in words. Empty keeps it; none makes it happen once." }
        if initial.repeat.isEmpty { return "It happens once now. Such as every weekday to make it repeat." }
        return "Empty makes it happen once."
    }

    private var repetitionChange: String? {
        let typed = repetition.trimmingCharacters(in: .whitespaces)
        guard asksRepetition, typed != initial.repeat else { return nil }
        if typed.isEmpty { return unspeakableRule ? nil : "none" }
        return typed
    }

    private static func date(_ clock: String) -> Date {
        let parts = clock.split(separator: ":").compactMap { Int($0) }
        return Calendar.current.date(bySettingHour: parts.first ?? 9, minute: parts.count > 1 ? parts[1] : 0, second: 0, of: .now) ?? .now
    }

    private static func clock(_ date: Date) -> String {
        let parts = Calendar.current.dateComponents([.hour, .minute], from: date)
        return String(format: "%02d:%02d", parts.hour ?? 0, parts.minute ?? 0)
    }

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
                // Only what changed, so a field edited on another device meanwhile is kept.
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

    /// Shows the form as a sheet on `window`.
    func present(on window: NSWindow, saved: @escaping (Change) -> Void) {
        self.saved = saved
        let sheet = NSWindow(contentViewController: NSHostingController(rootView: BlockForm(model: self)))
        sheet.title = heading
        close = { [weak window, weak sheet] in
            if let sheet { window?.endSheet(sheet) }
        }
        window.beginSheet(sheet)
    }
}

struct BlockForm: View {
    @ObservedObject var model: BlockFormModel

    var body: some View {
        Form {
            Section(model.heading) {
                TextField("Name", text: $model.name, prompt: Text("Deep work"))
                if model.asksDay {
                    DatePicker("Day", selection: $model.day, displayedComponents: .date)
                }
                DatePicker("Starts", selection: $model.start, displayedComponents: .hourAndMinute)
                LabeledContent("Lasts, in minutes") {
                    HStack {
                        TextField("Lasts, in minutes", value: $model.minutes, format: .number)
                            .labelsHidden()
                            .frame(width: 70)
                        Stepper("Lasts, in minutes", value: $model.minutes, in: 1...720, step: 5)
                            .labelsHidden()
                        Text(Clock.length(UInt32(max(model.minutes, 0)))).foregroundStyle(Color.quietLabel)
                    }
                }
                Picker("Kind", selection: $model.kind) {
                    Text("Work, takes tasks").tag("work")
                    Text("Break").tag("break")
                    Text("Event").tag("event")
                }
                .pickerStyle(.radioGroup)
                if model.asksRepetition {
                    TextField("Repeats", text: $model.repetition, prompt: Text("every weekday"))
                    Text(model.repetitionHelp).font(.footnote).foregroundStyle(Color.quietLabel)
                }
            }
            HStack {
                Spacer()
                Button("Cancel") { model.close() }
                    .keyboardShortcut(.cancelAction)
                Button("Save") { model.save() }
                    .keyboardShortcut(.defaultAction)
            }
        }
        .formStyle(.grouped)
        .tint(.lumennaTint)
        .frame(width: 440)
        .alert(
            "Could not do that",
            isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }),
            presenting: model.failure
        ) { _ in
            Button("OK") {}
        } message: { failure in
            Text(failure)
        }
    }
}
