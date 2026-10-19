import LumennaApp
import SwiftUI

/// Replan from Now and Plan the Day as the app has them open (`ViewModel.proposal`): each change
/// to check or leave, what fits nowhere, the other options. Every word is the core's, and every
/// rule the app's: what is proposed, the checks, what applying makes and what it proposes next,
/// and what to say. The screen closes when the app lets it go.
///
/// The surface's `ProposalModel` stays for the screens not yet moved onto the app.
final class AppProposalModel: ObservableObject {
    let core: Core
    /// Closes the screen; set by whoever presents it.
    var close: () -> Void = {}
    @Published private(set) var proposal: LumennaApp.ProposalView?
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        proposal = core.app.view.proposal
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            guard let self else { return }
            self.proposal = self.core.app.view.proposal
            if self.proposal == nil { self.close() }
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    var title: String { proposal?.words.title ?? "" }

    /// Whether the change at `index` is checked, for its toggle.
    func binding(_ index: Int) -> Binding<Bool> {
        Binding(get: { [weak self] in
            guard let changes = self?.proposal?.changes, index < changes.count else { return false }
            return changes[index].checked
        }, set: { [weak self] on in
            self?.send(.check(index: UInt32(index), on: on))
        })
    }

    func apply() { send(.apply) }

    func take(_ index: Int) { send(.take(UInt32(index))) }

    /// Puts the proposal away, making nothing.
    func cancel() { send(.close) }

    private func send(_ event: LumennaApp.ProposalEvent) {
        core.app.send(.proposal(event))
    }
}

/// The proposal's screen: each change a check box, its sentence the name and its reason the
/// description; what does not fit under its heading; the other options; the hint beside the
/// button that applies. With nothing proposed, the core's sentence for that and no button.
struct AppProposalForm: View {
    @ObservedObject var model: AppProposalModel

    var body: some View {
        if let proposal = model.proposal {
            form(proposal)
        }
    }

    private func form(_ proposal: LumennaApp.ProposalView) -> some View {
        let words = proposal.words
        return Form {
            #if os(macOS)
            // A sheet has no title bar to say what it is for.
            Section { FormParts.heading(words.title) }
            #endif
            if let refusal = proposal.refusal {
                Section {
                    Text(refusal).foregroundStyle(Color.warningLabel)
                }
            }
            if proposal.changes.isEmpty {
                Section {
                    Text(proposal.ready ? words.nothing : proposal.announcement)
                }
            } else {
                Section {
                    ForEach(Array(proposal.changes.enumerated()), id: \.offset) { index, change in
                        Toggle(isOn: model.binding(index)) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(change.sentence)
                                if let reason = change.reason {
                                    FormParts.caption(reason).accessibilityHidden(true)
                                }
                            }
                        }
                        #if os(macOS)
                        .toggleStyle(.checkbox)
                        #endif
                        .accessibilityLabel(change.sentence)
                        .modifier(FieldHint([change.reason, words.item].compactMap { $0 }.joined(separator: ". ")))
                    }
                }
            }
            if !proposal.unfit.isEmpty {
                Section {
                    ForEach(Array(proposal.unfit.enumerated()), id: \.offset) { _, line in
                        Text(line)
                    }
                } header: {
                    FormParts.heading(words.unfit)
                }
            }
            if !proposal.others.isEmpty {
                Section {
                    ForEach(Array(proposal.others.enumerated()), id: \.offset) { index, other in
                        Button(other) { model.take(index) }
                    }
                } header: {
                    FormParts.heading(words.others)
                }
            }
            Section {
                if !proposal.changes.isEmpty {
                    Button(words.apply) { model.apply() }
                        #if os(macOS)
                        .keyboardShortcut(.defaultAction)
                        #endif
                        .accessibilityIdentifier("proposal.apply")
                }
                #if !os(iOS)
                // On the phone Cancel is in the navigation bar.
                Button(words.cancel, role: .cancel) { model.cancel() }
                    #if os(macOS)
                    .keyboardShortcut(.cancelAction)
                    #endif
                #endif
            } footer: {
                if !proposal.changes.isEmpty { FormParts.caption(words.hint) }
            }
        }
        #if os(macOS)
        .formStyle(.grouped)
        .frame(minWidth: 460, minHeight: 320)
        #endif
        .tint(Color.lumennaTint)
    }
}
