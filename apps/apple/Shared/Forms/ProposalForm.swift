import SwiftUI

/// Replan from Now and Plan the Day: a proposal from the core, each change in it to accept or
/// leave, for every Apple app. Every word on the screen is the core's (`proposalWords`), and so
/// is every rule: what is proposed, and what is proposed again around what was left unchecked.
///
/// Applying some of a proposal can leave the day crowded, so the core proposes again (`next`)
/// keeping to what was turned down; the screen shows that until nothing more is proposed.
final class ProposalModel: ObservableObject, Identifiable {
    enum Kind {
        /// The rest of today.
        case replan
        /// A day, ISO.
        case planDay(String)
    }

    /// One change or sitting, as its row shows it.
    struct Item: Identifiable {
        let id: Int
        let sentence: String
        /// Why it is proposed: "Suggested because priority 1". Said as the row's description.
        let reason: String?
    }

    let kind: Kind
    let words: ProposalWords
    private let core: Core
    /// For tests: the moment the core takes as now, a local date and time.
    private let now: String?
    @Published private(set) var replan: Replan?
    @Published private(set) var plan: DayPlan?
    /// Whether each item is checked, by its `id`.
    @Published var checked: [Bool] = []
    /// Why applying was refused, shown on the screen above the proposal asked again.
    @Published var problem: String?
    /// Closes the screen; set by whoever presents it.
    var close: () -> Void = {}
    /// Told of each change made, after the store's change has been posted.
    var applied: (Change) -> Void = { _ in }

    init(core: Core, kind: Kind, now: String? = nil) throws {
        self.core = core
        self.kind = kind
        self.now = now
        switch kind {
        case .replan: words = proposalWords(kind: .replan)
        case .planDay: words = proposalWords(kind: .planDay)
        }
        try propose(replanPins: ReplanPins(keep: [], skip: [], sittings: []), dayPins: DayPins(exclude: [], declined: []))
    }

    /// The screen for an action of the day (`replan`, `planDay`), or nil for any other.
    convenience init?(core: Core, action: Action, now: String? = nil) throws {
        switch action.kind {
        case .replan: try self.init(core: core, kind: .replan, now: now)
        case .planDay: try self.init(core: core, kind: .planDay(action.target), now: now)
        default: return nil
        }
    }

    /// The whole proposal in one sentence: said when the screen opens.
    var announcement: String {
        replan?.announcement ?? plan?.announcement ?? ""
    }

    var items: [Item] {
        if let replan {
            return replan.changes.enumerated().map { Item(id: $0.offset, sentence: $0.element.sentence, reason: nil) }
        }
        return (plan?.sittings ?? []).enumerated().map { Item(id: $0.offset, sentence: $0.element.sentence, reason: $0.element.reason) }
    }

    /// What does not fit, under the heading `words.unfit`.
    var unfit: [String] {
        replan?.unplaceable ?? plan?.unfit ?? []
    }

    /// Asks the core for a proposal, everything in it checked.
    private func propose(replanPins: ReplanPins, dayPins: DayPins) throws {
        switch kind {
        case .replan:
            replan = try core.lumenna.replan(style: Clock.style, pins: replanPins, now: now)
            checked = Array(repeating: true, count: replan?.changes.count ?? 0)
        case let .planDay(date):
            plan = try core.lumenna.proposeDayPlan(date: date, style: Clock.style, pins: dayPins, now: now)
            checked = Array(repeating: true, count: plan?.sittings.count ?? 0)
        }
    }

    /// Makes what is checked. Says what was made and what is proposed now; shows the next
    /// proposal while there is one, else closes. A refusal (the day moved since) is shown, and
    /// the proposal is asked for again.
    func apply() {
        problem = nil
        do {
            let change: Change
            let said: (String, [String])
            if let replan {
                let accepted = replan.changes.enumerated().filter { isChecked($0.offset) }.map(\.element)
                let done = try core.lumenna.applyReplan(proposal: replan, accepted: accepted, now: now)
                change = done.change
                said = (done.announcement, done.notices)
                self.replan = done.next
                checked = Array(repeating: true, count: done.next.changes.count)
            } else if let plan {
                let accepted = plan.sittings.enumerated().filter { isChecked($0.offset) }.map(\.element)
                let done = try core.lumenna.applyDayPlan(plan: plan, accepted: accepted, now: now)
                change = done.change
                said = (done.announcement, done.notices)
                self.plan = done.next
                checked = Array(repeating: true, count: done.next.sittings.count)
            } else {
                return
            }
            NotificationCenter.default.post(name: Core.changed, object: nil)
            applied(change)
            Announcer.say(said.0, notices: said.1)
            if items.isEmpty { close() }
        } catch {
            let refused = error.sentence
            try? propose(replanPins: replan?.pins ?? ReplanPins(keep: [], skip: [], sittings: []), dayPins: plan?.pins ?? DayPins(exclude: [], declined: []))
            problem = refused
            Announcer.say(refused)
        }
    }

    private func isChecked(_ index: Int) -> Bool {
        index < checked.count && checked[index]
    }

    /// Whether the item at `index` is checked, for its toggle.
    func binding(_ index: Int) -> Binding<Bool> {
        Binding(get: { [weak self] in self?.isChecked(index) ?? false }, set: { [weak self] value in
            guard let self, index < self.checked.count else { return }
            self.checked[index] = value
        })
    }

    /// What the screen says on opening, once.
    private var opened = false
    func sayOpening() {
        guard !opened else { return }
        opened = true
        // Behind the screen's own focus change, so neither cuts the other off.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            guard let self else { return }
            Announcer.say(self.announcement)
        }
    }
}

/// The proposal's screen: each change a check box, its sentence the name and its reason the
/// description; what does not fit under its heading; the hint beside the button that applies.
/// With nothing proposed, the core's sentence for that and no button.
struct ProposalForm: View {
    @ObservedObject var model: ProposalModel

    var body: some View {
        Form {
            #if os(macOS)
            // A sheet has no title bar to say what it is for.
            Section { FormParts.heading(model.words.title) }
            #endif
            if let problem = model.problem {
                Section {
                    Text(problem).foregroundStyle(Color.warningLabel)
                }
            }
            if model.items.isEmpty {
                Section {
                    Text(model.words.nothing)
                }
            } else {
                Section {
                    ForEach(model.items) { item in
                        Toggle(isOn: model.binding(item.id)) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(item.sentence)
                                if let reason = item.reason {
                                    FormParts.caption(reason).accessibilityHidden(true)
                                }
                            }
                        }
                        #if os(macOS)
                        .toggleStyle(.checkbox)
                        #endif
                        .accessibilityLabel(item.sentence)
                        .modifier(FieldHint([item.reason, model.words.item].compactMap { $0 }.joined(separator: ". ")))
                    }
                }
            }
            if !model.unfit.isEmpty {
                Section {
                    ForEach(Array(model.unfit.enumerated()), id: \.offset) { _, line in
                        Text(line)
                    }
                } header: {
                    FormParts.heading(model.words.unfit)
                }
            }
            Section {
                if !model.items.isEmpty {
                    Button(model.words.apply) { model.apply() }
                        #if os(macOS)
                        .keyboardShortcut(.defaultAction)
                        #endif
                        .accessibilityIdentifier("proposal.apply")
                }
                #if !os(iOS)
                // On the phone Cancel is in the navigation bar.
                Button(model.words.cancel, role: .cancel) { model.close() }
                    #if os(macOS)
                    .keyboardShortcut(.cancelAction)
                    #endif
                #endif
            } footer: {
                if !model.items.isEmpty { FormParts.caption(model.words.hint) }
            }
        }
        #if os(macOS)
        .formStyle(.grouped)
        .frame(minWidth: 460, minHeight: 320)
        #endif
        .tint(Color.lumennaTint)
        .onAppear { model.sayOpening() }
    }
}
