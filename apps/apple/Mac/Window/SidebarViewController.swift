import AppKit
import LumennaApp
import SwiftUI

/// The sidebar's entries as the window shows them (`ViewModel.sidebar`), and the place shown.
final class SidebarModel: ObservableObject {
    let core: Core
    @Published private(set) var entries: [LumennaApp.SidebarEntry] = []
    /// The entries the app has folded.
    @Published private(set) var folded: Set<String> = []
    /// What each row is named: its line, whether it is expanded, and its level where that
    /// changes from the row shown before (`AppModel.sidebarItems`, as the iPad says them), which
    /// the outline reports but VoiceOver, reading the row's text, does not say.
    @Published private(set) var names: [String: String] = [:]
    /// The key of the place the window shows, kept selected.
    @Published private(set) var shown: String?
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
        draw()
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Drawn again only when they changed, so a minute or a sync does not make VoiceOver read
    /// the row again.
    private func draw() {
        let view = core.app.view
        if entries != view.sidebar { entries = view.sidebar }
        let folded = Set(view.sidebarFolded)
        if self.folded != folded { self.folded = folded }
        var names: [String: String] = [:]
        for item in core.app.sidebarItems() {
            names[item.key] = [item.title, item.spoken].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: ", ")
        }
        if self.names != names { self.names = names }
        let shown = view.place.map { LumennaApp.sidebarKey(kind: .place($0)) }
        if self.shown != shown { self.shown = shown }
    }

    /// A place chosen: held in the sidebar's hand, and shown.
    func choose(_ key: String) {
        guard let entry = entries.first(where: { $0.key == key }), case let .place(place) = entry.kind,
              place != core.app.view.place else { return }
        core.app.send(.hold(field0: .sidebar, field1: key))
        core.app.send(.open(place))
        // Selected here at once: the app's turn that shows it has yet to come.
        shown = key
    }

    /// One of an entry's actions, the app's, held in the sidebar's hand first.
    func run(_ action: LumennaApp.Action, on entry: LumennaApp.SidebarEntry) {
        core.app.run(action, on: entry.key, side: .sidebar)
    }

    /// The heading's New, from the File menu.
    func runNew(under group: LumennaApp.SidebarGroup) {
        guard let heading = entries.first(where: { if case let .group(found) = $0.kind { found == group } else { false } }),
              let action = heading.actions.first(where: { $0.kind == .new }) else { return }
        run(action, on: heading)
    }
}

/// The places: Today, Tasks, the project tree, labels, saved filters, blocks, the trash.
///
/// A source list, as Mail's mailboxes are, in SwiftUI, which is AppKit's outline underneath, so
/// VoiceOver hears each row's level and expansion from it. The places, which of them are
/// folded, and what a project, label or filter can have done to it are the app's
/// (`ViewModel.sidebar`), as every sidebar has them; its actions are in its context menu, which
/// VoiceOver opens with VO-Shift-M, and a heading's is its New. Its rows are held in the
/// sidebar's hand, so a change made here lands here; the place selected is the one the window
/// shows. A heading is never selected: the arrow keys pass over it.
struct SidebarScreen: View {
    @ObservedObject var model: SidebarModel

    /// An entry and the entries beneath it.
    typealias Node = OutlineNode<LumennaApp.SidebarEntry>

    var body: some View {
        List(selection: Binding(get: { model.shown }, set: { if let key = $0 { model.choose(key) } })) {
            ForEach(Self.tree(model.entries)) { node in
                NodeView(node: node, model: model)
            }
        }
        .listStyle(.sidebar)
        .accessibilityLabel("Places")
    }

    /// The app's entries as a tree, nested by depth.
    static func tree(_ entries: [LumennaApp.SidebarEntry]) -> [Node] {
        Outline.tree(entries, id: \.key, depth: \.depth)
    }

    /// An entry, and inside a disclosure group the entries beneath it, folded by its own action.
    private struct NodeView: View {
        let node: Node
        @ObservedObject var model: SidebarModel

        var body: some View {
            if node.children.isEmpty, node.group {
                // A heading with nothing under it yet (no labels, no saved filters).
                Row(node: node, model: model).selectionDisabled()
            } else if node.children.isEmpty {
                Row(node: node, model: model).tag(node.id)
            } else if node.group {
                // A heading stays one: Projects, Labels and Saved Filters are what VoiceOver's
                // heading keys move between.
                group
            } else {
                // A project with subprojects is a place like any other.
                group.accessibilityRemoveTraits(.isHeader).tag(node.id)
            }
        }

        private var group: some View {
            DisclosureGroup(isExpanded: Binding(get: { !model.folded.contains(node.id) }, set: { open in
                let kind: LumennaApp.ActionKind = open ? .expand : .collapse
                if let fold = node.row.actions.first(where: { $0.kind == kind }) { model.run(fold, on: node.row) }
            })) {
                ForEach(node.children) { child in
                    AnyView(NodeView(node: child, model: model))
                }
            } label: {
                // A heading is never selected, so the arrows pass over it; only its label: set
                // on the group, the places under it could not be chosen either.
                Row(node: node, model: model).selectionDisabled(node.group)
            }
        }
    }

    /// One entry: its title, and its count or state for VoiceOver, since a source list is too
    /// narrow for a second line; its actions its context menu. No tooltip: VoiceOver read one
    /// as the row's help, saying the count twice.
    private struct Row: View {
        let node: Node
        @ObservedObject var model: SidebarModel

        var body: some View {
            let entry = node.row
            let name = model.names[entry.key] ?? entry.text
            HStack {
                SwatchMark(swatch: entry.swatch)
                Text(entry.title)
                    .font(node.group ? .subheadline.weight(.semibold) : .body)
                    .foregroundStyle(Color.primary)
                    .lineLimit(1)
                    // VoiceOver reads a row's one text, not the row's name: the text says the
                    // whole line. Hidden instead, the row was said with "group" after it.
                    .accessibilityRepresentation { Text(name) }
                Spacer(minLength: 0)
            }
                // A group named by its whole line ("Work, 3 tasks, expanded, level 2"), as the task
                // list's rows are: as a name and a value, VoiceOver said the value alone, and as
                // static text, nothing of the outline. Found by its title.
                .accessibilityElement(children: .contain)
                .accessibilityLabel(name)
                .accessibilityIdentifier(entry.title)
                .contextMenu {
                    ForEach(entry.actions, id: \.self) { action in
                        Button(action.asksFirst ? action.title + "…" : action.title) { model.run(action, on: entry) }
                    }
                }
        }
    }
}

/// The SwiftUI sidebar in the window's first pane, hosted by a view that steps out of
/// VoiceOver's tree (`FlatHostingView`).
final class SidebarViewController: NSViewController {
    private let model: SidebarModel

    init(core: Core) {
        model = SidebarModel(core: core)
        super.init(nibName: nil, bundle: nil)
        title = "Places"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        view = FlatHostingView(rootView: SidebarScreen(model: model))
    }

    /// The list's own outline, AppKit's under SwiftUI's `List`: where F6 goes.
    var outline: NSView? {
        func find(_ view: NSView) -> NSOutlineView? {
            (view as? NSOutlineView) ?? view.subviews.lazy.compactMap(find).first
        }
        return isViewLoaded ? find(view) : nil
    }

    override var preferredFirstResponder: NSView? { outline ?? view }

    @objc func newProject() { model.runNew(under: .projects) }
    @objc func newLabel() { model.runNew(under: .labels) }
    @objc func newSavedFilter() { model.runNew(under: .filters) }
}

/// A menu item that runs a closure, for menus built from a list of actions.
final class ClosureMenuItem: NSMenuItem {
    private let run: () -> Void

    init(title: String, keyEquivalent: String = "", action run: @escaping () -> Void) {
        self.run = run
        super.init(title: title, action: #selector(runAction), keyEquivalent: keyEquivalent)
        target = self
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("not used") }

    @objc private func runAction() { run() }
}

extension OutlineNode where Row == LumennaApp.SidebarEntry {
    /// Whether it is a heading (Projects, Labels, Saved Filters), never a place.
    var group: Bool { if case .group = row.kind { true } else { false } }
}
