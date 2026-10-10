import SwiftUI

/// A place's tasks, each row as the phone says it: the title, then what the core says of it.
/// Mark Done and Delete are its swipe actions, which VoiceOver offers as the row's actions;
/// in the trash, Restore and Delete from Trash. Tasks can be filtered, with the filter read
/// back; a project, label or filter's own actions are at the foot, as Browse offers them.
struct TaskListView: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    let place: Place
    @State private var adding = false
    @State private var erasing: RowView?
    @State private var filter = ""
    @State private var asked: Asked?
    @State private var folding = Folding()

    var body: some View {
        let listing = rows
        List {
            if place == .tasks {
                TextField("Filter", text: $filter, prompt: example("#Work & overdue"))
                    .textInputAutocapitalization(.never)
                    .accessibilityLabel("Filter")
                CompletionOffer(text: $filter, syntax: .filter)
                // The readback: a misread filter shows wrong results silently.
                if let query = listing?.query {
                    Text(query.description).font(.footnote)
                }
            }
            if !trash {
                Button("New Task") { adding = true }
            }
            if let listing, listing.rows.isEmpty {
                Text(listing.announcement.prefix(1).uppercased() + listing.announcement.dropFirst())
            }
            // Folded as on the phone, the level said against the row shown before.
            let shown = folding.shown(listing?.rows ?? [], depth: { Int($0.depth) }, key: \.id)
            ForEach(Array(shown.enumerated()), id: \.element.item.id) { index, row in
                taskRow(row, after: index == 0 ? nil : shown[index - 1].item.depth)
            }
            PlaceActions(place: place, asked: $asked, left: { dismiss() })
        }
        .navigationTitle(placeTitle(place: place))
        .navigationDestination(for: TaskOpened.self) { opened in
            TaskView(id: opened.id)
        }
        .sheet(isPresented: $adding) {
            QuickAddView(prefix: placeQuickAddPrefix(place: place))
        }
        .sheet(item: $asked) { asked in
            NavigationStack { asked.view }
        }
        // What cannot be undone asks first, and the answer that keeps it comes first.
        .confirmationDialog(
            "Delete \(erasing?.title ?? "") for good? This cannot be undone.",
            isPresented: Binding(get: { erasing != nil }, set: { if !$0 { erasing = nil } }),
            titleVisibility: .visible
        ) {
            Button("Cancel", role: .cancel) { erasing = nil }
            Button("Delete for Good", role: .destructive) {
                if let row = erasing { core.act { try core.lumenna.eraseTask(id: row.id) } }
                erasing = nil
            }
        }
    }

    private func taskRow(_ shown: Folding.Shown<RowView>, after previous: UInt32?) -> some View {
        let row = shown.item
        return NavigationLink(value: TaskOpened(id: row.id)) {
            // The row owns what VoiceOver says: the title as its name, then what the core
            // says of it as its value, as on the phone.
            VStack(alignment: .leading) {
                Text(row.title)
                if let detail = RowSpeech.details(row) {
                    Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(RowSpeech.label(row))
            .accessibilityValue(RowSpeech.value(row, previousDepth: previous, fold: shown.state))
        }
        .swipeActions(edge: .leading) {
            if !trash {
                Button(row.checked == true ? "Mark Not Done" : "Mark Done") { toggle(row) }
            }
        }
        .swipeActions(edge: .trailing) {
            if trash {
                Button("Restore") { core.act { try core.lumenna.restoreTask(id: row.id) } }
                Button("Delete from Trash", role: .destructive) { erasing = row }
            } else {
                Button("Delete", role: .destructive) { core.act { try core.lumenna.trashTask(id: row.id) } }
            }
            if let fold = Folding.action(for: shown) {
                Button(fold.title) {
                    folding.toggle(row.id)
                    Announcer.say(fold.said)
                }
            }
        }
    }

    private var trash: Bool { place == .trash }

    private var rows: Rows? {
        _ = core.generation
        let typed = filter.trimmingCharacters(in: .whitespaces)
        let query = place == .tasks && !typed.isEmpty ? typed : placeQuery(place: place)
        return core.read { try core.lumenna.listTasks(query: query) }
    }

    private func toggle(_ row: RowView) {
        if row.checked == true {
            core.act { try core.lumenna.uncompleteTask(id: row.id) }
        } else {
            core.act { try core.lumenna.completeTask(id: row.id) }
        }
    }
}

/// A task to open, by identifier.
struct TaskOpened: Hashable {
    let id: String
}

/// What can be done to a project, a label or a saved filter, as Browse and the sidebar offer
/// it on the phone: rename, reorder, nest, weigh, archive, merge, colour, change the query,
/// delete. At the foot of its list, as buttons, since a watch's row has room for few swipes.
private struct PlaceActions: View {
    @EnvironmentObject private var core: WatchCore
    let place: Place
    @Binding var asked: Asked?
    /// Leaves the place's list, once the place is gone or renamed.
    let left: () -> Void

    var body: some View {
        switch place {
        case let .project(name): project(name)
        case let .label(name): label(name)
        case let .filter(name, query): filter(name, query: query)
        default: EmptyView()
        }
    }

    private var lumenna: Lumenna { core.lumenna }

    private func project(_ name: String) -> some View {
        let projects = core.read { try lumenna.listProjects().rows } ?? []
        let archived = projects.first { $0.title == name }?.state.contains("archived") == true
        return Section {
            ForEach(PlaceAction.ofProject(archived: archived), id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) {
                    switch action {
                    case .rename:
                        asked = Asked(TextPrompt("Rename \(name)", initial: name) { renamed in leave { try lumenna.renameProject(name: name, to: renamed) } })
                    case .moveUp: core.act { try lumenna.reorderProject(name: name, direction: .up) }
                    case .moveDown: core.act { try lumenna.reorderProject(name: name, direction: .down) }
                    case .moveUnder:
                        let others = projects.map(\.title).filter { $0 != name }
                        asked = Asked(ChoicePrompt(title: "Move \(name) under", choices: [("Top Level", { core.act { try lumenna.moveProject(name: name, parent: nil) } })]
                            + others.map { other in (other, { core.act { try lumenna.moveProject(name: name, parent: other) } }) }))
                    case .addProjectInside:
                        asked = Asked(TextPrompt("New Project in \(name)", placeholder: "Name", action: "Add") { added in
                            core.act { try lumenna.addProject(name: added, parent: name) }
                        })
                    case .weight: asked = Asked(WeightPrompt(name: name))
                    case .archive, .unarchive: core.act { try lumenna.archiveProject(name: name) }
                    case .delete:
                        asked = Asked(ChoicePrompt(title: "Delete \(name)?", message: PlaceAction.deletingProject, choices: [
                            (PlaceAction.deleteAndTrash, { leave { try lumenna.deleteProject(name: name, keepTasks: false) } }),
                            (PlaceAction.deleteAndKeep, { leave { try lumenna.deleteProject(name: name, keepTasks: true) } }),
                        ]))
                    case .mergeInto, .colour, .changeQuery: break
                    }
                }
            }
        } header: {
            FormParts.heading("Project")
        }
    }

    private func label(_ name: String) -> some View {
        Section {
            ForEach(PlaceAction.ofLabel, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) {
                    switch action {
                    case .rename:
                        asked = Asked(TextPrompt("Rename \(name)", initial: name) { renamed in leave { try lumenna.renameLabel(name: name, to: renamed) } })
                    case .moveUp: core.act { try lumenna.reorderLabel(name: name, direction: .up) }
                    case .moveDown: core.act { try lumenna.reorderLabel(name: name, direction: .down) }
                    case .mergeInto:
                        // For when a typo made a near-duplicate: this one's tasks move to the other.
                        let others = (core.read { try lumenna.listLabels().rows } ?? []).map(\.title).filter { $0 != name }
                        asked = Asked(ChoicePrompt(title: "Merge \(name) into", choices: others.map { other in
                            (other, { leave { try lumenna.mergeLabels(from: name, into: other) } })
                        }))
                    case .colour:
                        asked = Asked(TextPrompt("Colour for \(name)", message: PlaceAction.colourHelp, placeholder: "teal") { colour in
                            core.act { try lumenna.recolourLabel(name: name, colour: colour.lowercased() == "none" ? nil : colour) }
                        })
                    case .delete:
                        asked = Asked(ChoicePrompt(title: "Delete \(name)?", message: PlaceAction.deletingLabel, choices: [
                            ("Delete", { leave { try lumenna.deleteLabel(name: name) } }),
                        ]))
                    default: break
                    }
                }
            }
        } header: {
            FormParts.heading("Label")
        }
    }

    private func filter(_ name: String, query: String) -> some View {
        Section {
            ForEach(PlaceAction.ofFilter, id: \.self) { action in
                Button(action.title, role: action.destructive ? .destructive : nil) {
                    switch action {
                    case .rename:
                        asked = Asked(TextPrompt("Rename \(name)", initial: name) { renamed in leave { try lumenna.editFilter(name: name, rename: renamed, query: nil) } })
                    case .changeQuery:
                        asked = Asked(TextPrompt("Query for \(name)", initial: query, syntax: .filter) { changed in
                            leave { try lumenna.editFilter(name: name, rename: nil, query: changed) }
                        })
                    case .moveUp: core.act { try lumenna.reorderFilter(name: name, direction: .up) }
                    case .moveDown: core.act { try lumenna.reorderFilter(name: name, direction: .down) }
                    case .delete: leave { try lumenna.deleteFilter(name: name) }
                    default: break
                    }
                }
            }
        } header: {
            FormParts.heading("Saved filter")
        }
    }

    /// A change after which this list stands for something gone or renamed: back to the places.
    private func leave(_ operation: () throws -> Change) {
        if core.act(operation) { left() }
    }
}

/// A project's weight, asked again with what was typed when it is not one: a typo must not
/// quietly become "inherit" (the core reads it, `parseWeight`).
private struct WeightPrompt: View {
    @EnvironmentObject private var core: WatchCore
    @Environment(\.dismiss) private var dismiss
    let name: String
    @State private var typed = ""
    @State private var problem: String?

    var body: some View {
        List {
            Text((problem.map { "\($0) " } ?? "") + PlaceAction.weightHelp)
                .font(.footnote)
            TextField("Weight", text: $typed, prompt: example("1.0"))
            Button("Save") {
                do {
                    let weight = try parseWeight(text: typed)
                    if core.act({ try core.lumenna.weighProject(name: name, weight: weight) }) { dismiss() }
                } catch {
                    problem = error.sentence
                }
            }
        }
        .navigationTitle("Weight of \(name)")
    }
}

/// Every block series, each with when it happens, as Browse lists them, each opening the
/// form for every occurrence.
struct BlockListView: View {
    @EnvironmentObject private var core: WatchCore
    @State private var editing: BlockFormModel?

    var body: some View {
        let listing: Rows? = {
            _ = core.generation
            return core.read { try core.lumenna.listBlocks() }
        }()
        List {
            if let listing, listing.rows.isEmpty {
                Text("No blocks")
            }
            ForEach(listing?.rows ?? [], id: \.id) { row in
                Button {
                    editing = core.read { try BlockFormModel.series(core: core, id: row.id) { change in
                        core.changed()
                        Announcer.say(change.announcement, notices: change.notices)
                    } }
                } label: {
                    VStack(alignment: .leading) {
                        Text(row.title)
                        if let value = row.value {
                            Text(value).font(.footnote).foregroundStyle(Color.quietLabel)
                        }
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(row.title)
                    .accessibilityValue(row.value ?? "")
                }
            }
        }
        .navigationTitle("Blocks")
        .sheet(item: $editing) { model in
            BlockSheet(model: model)
        }
    }
}

extension BlockFormModel: Identifiable {}

/// The block form (`Shared/Forms/BlockForm.swift`) as a watch sheet, closing itself on save.
struct BlockSheet: View {
    @ObservedObject var model: BlockFormModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            BlockForm(model: model)
                .navigationTitle(model.heading)
        }
        .onAppear { model.close = { dismiss() } }
    }
}
