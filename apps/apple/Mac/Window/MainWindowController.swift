import AppKit

/// The one main window (§16.5): a sidebar of places, the list for the place chosen, and the
/// chosen task's details — three panes in an `NSSplitViewController`, as Mail has them.
///
/// F6 and Shift-F6 move between the panes. Neither AppKit nor any other toolkit provides that
/// for free (§16.4), and a VoiceOver user otherwise reaches the detail pane by interacting
/// through everything in between.
final class MainWindowController: NSWindowController, NSWindowDelegate {
    let core: Core
    private let split = NSSplitViewController()
    private(set) var sidebar: SidebarViewController!
    private let content = ContainerViewController(placeholder: "", name: "List")
    private let detail = ContainerViewController(placeholder: "No task selected", name: "Task details")

    init(core: Core) {
        self.core = core
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1100, height: 700),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Lumenna"
        window.identifier = NSUserInterfaceItemIdentifier("main")
        window.setFrameAutosaveName("Main")
        window.tabbingMode = .disallowed
        super.init(window: window)
        window.delegate = self

        sidebar = SidebarViewController(core: core) { [weak self] place in self?.show(place) }
        let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebar)
        sidebarItem.minimumThickness = 180
        let contentItem = NSSplitViewItem(contentListWithViewController: content)
        contentItem.minimumThickness = 320
        let detailItem = NSSplitViewItem(viewController: detail)
        detailItem.minimumThickness = 300
        detailItem.canCollapse = true
        split.splitViewItems = [sidebarItem, contentItem, detailItem]
        split.splitView.autosaveName = "MainSplit"
        window.contentViewController = split
        // Setting the content shrinks the window to what the panes need at least, which with
        // lists that scroll is almost nothing; and a saved frame from such a window is no use.
        window.minSize = NSSize(width: 820, height: 460)
        if window.frame.height < window.minSize.height || window.frame.width < window.minSize.width {
            window.setContentSize(NSSize(width: 1100, height: 700))
            window.center()
        }
        sidebar.select(.today)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    // MARK: - Places

    /// Shows a place in the middle pane, and clears the detail pane until something is chosen.
    func show(_ place: Place) {
        let controller: NSViewController
        switch place {
        case .today:
            controller = DayViewController(core: core, window: self)
        case .tasks:
            controller = TaskListViewController(core: core, window: self, title: "Tasks")
        case let .project(name):
            controller = TaskListViewController(
                core: core, window: self, title: name,
                query: sigil("#", name), quickAddPrefix: sigil("#", name) + " "
            )
        case let .label(name):
            controller = TaskListViewController(
                core: core, window: self, title: name,
                query: sigil("@", name), quickAddPrefix: sigil("@", name) + " "
            )
        case let .filter(name, query):
            controller = TaskListViewController(core: core, window: self, title: name, query: query)
        case .blocks:
            controller = BlocksViewController(core: core, window: self)
        case .trash:
            controller = TaskListViewController(core: core, window: self, title: "Trash", query: "deleted", mode: .trash)
        }
        content.show(controller)
        window?.title = controller.title ?? "Lumenna"
        showTask(nil)
    }

    /// The task in the detail pane, or none.
    func showTask(_ id: String?) {
        guard let id else {
            detail.show(nil)
            return
        }
        // The same task stays as it is — it reads changes itself — so focus in the pane is
        // not thrown away when the list reloads around a change made there.
        if (detail.current as? TaskDetailViewController)?.taskID == id { return }
        detail.show(TaskDetailViewController(core: core, id: id, window: self))
    }

    /// The list in the middle pane, if it is a task list — what the Task menu acts on.
    var taskList: TaskListViewController? { content.current as? TaskListViewController }
    var day: DayViewController? { content.current as? DayViewController }

    // MARK: - Panes (F6)

    private var panes: [NSView] {
        [sidebar.outline, content.current?.preferredFirstResponder, detail.current?.preferredFirstResponder]
            .compactMap { $0 }
    }

    @objc func nextPane(_ sender: Any?) { movePane(by: 1) }
    @objc func previousPane(_ sender: Any?) { movePane(by: -1) }

    private func movePane(by step: Int) {
        guard let window else { return }
        let panes = self.panes
        guard !panes.isEmpty else { return }
        let current = panes.firstIndex { pane in
            guard let responder = window.firstResponder as? NSView else { return false }
            return responder === pane || responder.isDescendant(of: pane)
                || (window.fieldEditor(false, for: nil) === responder && (pane as? NSTextField)?.currentEditor() != nil)
        } ?? (step > 0 ? panes.count - 1 : 0)
        let next = panes[(current + step + panes.count) % panes.count]
        window.makeFirstResponder(next)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        // Closing hides it: the app stays resident, syncing (§16.2).
        sender.orderOut(nil)
        return false
    }
}

/// A pane whose content is swapped as the person moves around.
final class ContainerViewController: NSViewController {
    private let placeholder: String
    private let name: String
    private(set) var current: NSViewController?

    /// `name` is what VoiceOver calls the pane, as it does Mail's.
    init(placeholder: String, name: String) {
        self.placeholder = placeholder
        self.name = name
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        view = NSView()
        view.setAccessibilityRole(.group)
        view.setAccessibilityLabel(name)
        show(nil)
    }

    func show(_ controller: NSViewController?) {
        current?.view.removeFromSuperview()
        current?.removeFromParent()
        current = controller
        view.subviews.forEach { $0.removeFromSuperview() }
        // The pane is named once: by what fills it — the list, the outline, the form's scroll
        // area — or by itself while it holds nothing.
        view.setAccessibilityElement(controller == nil)
        let shown: NSView
        if let controller {
            addChild(controller)
            shown = controller.view
        } else {
            let label = NSTextField(labelWithString: placeholder)
            label.textColor = .quietLabel
            label.alignment = .center
            let centre = NSStackView(views: [label])
            centre.orientation = .vertical
            shown = centre
        }
        shown.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(shown)
        NSLayoutConstraint.activate([
            shown.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            shown.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            shown.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            shown.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }
}

extension NSViewController {
    /// Where focus goes when F6 reaches this pane.
    @objc var preferredFirstResponder: NSView? { view }
}
