import SwiftUI
import UIKit

/// The block editor (§16.1): a form for a new block, or for changing one — every occurrence,
/// or one day, as the person already chose.
final class BlockFormViewController: UIHostingController<BlockForm> {
    init(model: BlockFormModel) {
        super.init(rootView: BlockForm(model: model))
        title = model.title
        // UIKit buttons, Save always enabled: a disabled one failed contrast, and saving a
        // block with no name says why it cannot.
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { _ in model.close() }
        )
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: "Save", primaryAction: UIAction { _ in if model.save() { model.close() } }
        )
        navigationItem.rightBarButtonItem?.style = .done
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }
}

final class BlockFormModel: ObservableObject {
    enum Purpose {
        case add
        /// Every occurrence of the block with this series identifier.
        case series(String)
        /// One day of it.
        case occurrence(String, day: String)
    }

    let purpose: Purpose
    let title: String
    /// How the block repeats now, as an RFC 5545 rule, when editing one that does.
    let currentRule: String?
    private let initial: (name: String, start: Date, minutes: Int, kind: String, repeat: String)
    private let saved: (Change) -> Void
    private let core: Core

    @Published var name: String
    @Published var start: Date
    @Published var minutes: Int
    @Published var kind: String
    @Published var repetition: String
    @Published var day: Date
    @Published var failure: String?
    /// Closes the form; set by whoever presents it, since SwiftUI's `dismiss` does not reach
    /// a UIKit sheet reliably.
    var close: () -> Void = {}

    init(
        core: Core,
        purpose: Purpose,
        name: String = "",
        start: String = "09:00",
        minutes: Int = 60,
        kind: String = "work",
        repetition: String = "",
        day: Date = .now,
        currentRule: String? = nil,
        saved: @escaping (Change) -> Void
    ) {
        self.core = core
        self.currentRule = currentRule
        self.purpose = purpose
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
        case .add: title = "New Block"
        case .series: title = "Every Occurrence"
        case let .occurrence(_, day): title = Clock.spokenDay(day) + " Only"
        }
    }

    /// What the repetition field means, which differs between adding and changing.
    var repetitionHelp: String {
        switch (purpose, currentRule) {
        case (.add, _):
            return "Such as \u{201C}every weekday\u{201D}. Empty for a block that happens once."
        case let (_, rule?):
            return "It repeats now as \(rule). Empty keeps that; \u{201C}none\u{201D} makes it happen once."
        default:
            return "It happens once now. Such as \u{201C}every weekday\u{201D} to make it repeat."
        }
    }

    var asksRepetition: Bool {
        if case .occurrence = purpose { return false }
        return true
    }

    var asksDay: Bool {
        if case .add = purpose { return true }
        return false
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

    func save() -> Bool {
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty else {
            failure = "A block needs a name."
            return false
        }
        let at = Self.clock(start)
        do {
            let change: Change
            switch purpose {
            case .add:
                change = try core.lumenna.addBlock(block: NewBlock(
                    title: name,
                    at: at,
                    minutes: UInt32(minutes),
                    date: Clock.isoDay(day),
                    kind: kind,
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
                    // Empty keeps how it repeats; "none" makes it happen once.
                    repeat: asksRepetition && !repetition.isEmpty ? repetition : nil
                )
                let scope: BlockScope
                if case let .occurrence(_, day) = purpose {
                    scope = .occurrence(date: day)
                } else {
                    scope = .series
                }
                change = try core.lumenna.editBlock(id: id, edit: edit, scope: scope)
            }
            saved(change)
            return true
        } catch {
            failure = error.sentence
            return false
        }
    }
}

struct BlockForm: View {
    @ObservedObject var model: BlockFormModel

    var body: some View {
        Form {
            Section {
                NamedRow(name: "Name") {
                    TextField("", text: $model.name, prompt: example("Deep work"))
                        .multilineTextAlignment(.trailing)
                }
                if model.asksDay {
                    DatePicker("Day", selection: $model.day, displayedComponents: .date)
                }
                DatePicker("Starts", selection: $model.start, displayedComponents: .hourAndMinute)
                Stepper(value: $model.minutes, in: 5...720, step: 5) {
                    Text("Lasts \(Clock.length(UInt32(model.minutes)))")
                }
            }
            Section {
                Picker("Kind", selection: $model.kind) {
                    Text("Work, takes tasks").tag("work")
                    Text("Break").tag("break")
                    Text("Event").tag("event")
                }
                .pickerStyle(.inline)
                .labelsHidden()
            } header: {
                FormParts.caption("Kind")
            }
            if model.asksRepetition {
                Section {
                    NamedRow(name: "Repeats") {
                        TextField("", text: $model.repetition, prompt: example("every weekday"))
                            .multilineTextAlignment(.trailing)
                            .textInputAutocapitalization(.never)
                    }
                } footer: {
                    FormParts.caption(model.repetitionHelp)
                }
            }
        }
        // SwiftUI's own accent, which the window's UIKit tint does not reach.
        .tint(Color(uiColor: .lumennaTint))
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
