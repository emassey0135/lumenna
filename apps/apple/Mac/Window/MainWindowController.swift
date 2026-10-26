import AppKit
import LumennaApp

/// The one main window: a sidebar of places, the list for the place shown, and the details of
/// the task in hand — three panes in an `NSSplitViewController`, as Mail has them.
///
/// The panes follow the app: the middle one shows the app's place (`ViewModel.place`), so a
/// place renamed, merged or deleted from the sidebar takes the window with it, and the right
/// one the task the app has in hand (`ViewModel.details`).
///
/// F6 and Shift-F6 move between the panes. Neither AppKit nor any other toolkit provides that
/// for free, and a VoiceOver user otherwise reaches the detail pane by interacting
/// through everything in between.
final class MainWindowController: NSWindowController, NSWindowDelegate {
    let core: Core
    private let split = NSSplitViewController()
    private(set) var sidebar: SidebarViewController!
    private let content = ContainerViewController(placeholder: "", name: "List")
    private let detail = ContainerViewController(placeholder: "No task selected", name: "Task details")
    private var questions: AppQuestions?
    /// What asks them, kept here: `AppQuestions` holds it weakly.
    private var asker: FrontAsker?
    private var sheets: AppSheets?
    private var observer: NSObjectProtocol?
    /// The place the middle pane shows.
    private var shown: LumennaApp.Place?

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

        sidebar = SidebarViewController(core: core)
        let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebar)
        sidebarItem.minimumThickness = 180
        let contentItem = NSSplitViewItem(contentListWithViewController: content)
        contentItem.minimumThickness = 320
        let detailItem = NSSplitViewItem(viewController: detail)
        detailItem.minimumThickness = 300
        detailItem.canCollapse = true
        split.splitViewItems = [sidebarItem, contentItem, detailItem]
        split.splitView.autosaveName = "MainSplit"
        // The panes, and under them a status line holding what the app last said.
        window.contentViewController = StatusLineController(content: split)
        // Setting the content shrinks the window to what the panes need at least, which with
        // lists that scroll is almost nothing; and a saved frame from such a window is no use.
        window.minSize = NSSize(width: 820, height: 460)
        if window.frame.height < window.minSize.height || window.frame.width < window.minSize.width {
            window.setContentSize(NSSize(width: 1100, height: 700))
            window.center()
        }
        // The app's questions, as sheets on this window, and the forms they open: a task's
        // details are the pane beside the list.
        let asker = FrontAsker(main: window)
        self.asker = asker
        questions = AppQuestions(app: core.app, asker: asker) { [weak self] action in
            switch action.kind {
            case .editTask, .edit where action.subject == .task: self?.showDetails()
            default: break
            }
        }
        questions?.answeredElsewhere = RemindersModel.answersItself
        // The screens the app opens by itself: the block form, a proposal, a reminder list.
        sheets = AppSheets(core: core, window: window)
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.follow()
        }
        // The window opens on the day, as it always has: the app's own first place is Tasks.
        core.app.send(.open(.today))
        follow()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    // MARK: - Places

    /// Goes to `place`, as choosing it in the sidebar does, then does `then` on its pane once the
    /// pane shows it. The pane changes only when the app's turn comes back (`follow`), so a
    /// command that goes somewhere and acts there acted on the pane it left.
    func go(_ place: LumennaApp.Place, then: (() -> Void)? = nil) {
        if core.app.current.view.place != place { core.app.send(.open(place)) }
        guard let then else { return }
        if shown == place, core.app.view.place == place { return then() }
        arriving.append((place, then))
    }

    /// What waits for a place to be shown in the middle pane, as `go` was asked.
    private var arriving: [(place: LumennaApp.Place, then: () -> Void)] = []

    /// The panes as the app has them: the place shown in the middle, the task in hand beside it.
    private func follow() {
        let view = core.app.view
        if let place = view.place, place != shown {
            shown = place
            let controller: NSViewController = switch place {
            case .today: DayViewController(core: core, window: self)
            default: TaskListViewController(core: core, window: self, place: place)
            }
            content.show(controller)
        }
        window?.title = view.place == .today ? core.app.lines.dayTitle : view.title
        showTask(view.details?.task.id)
        // What waited for the place now shown, once the pane has laid it out. What waits for a
        // place the app has since left is dropped.
        if let shown, !arriving.isEmpty {
            let ready = arriving.filter { $0.place == shown }.map(\.then)
            let now = core.app.current.view.place
            arriving.removeAll { $0.place == shown || $0.place != now }
            if !ready.isEmpty { DispatchQueue.main.async { ready.forEach { $0() } } }
        }
    }

    /// The task in the detail pane, or none.
    private func showTask(_ id: String?) {
        guard let id else {
            detail.show(nil)
            return
        }
        // The same task stays as it is — it reads changes itself — so focus in the pane is
        // not thrown away when the list redraws around a change made there.
        if (detail.current as? TaskDetailViewController)?.taskID == id { return }
        detail.show(TaskDetailViewController(core: core, id: id))
    }

    /// Goes to the details of the task in hand: Return on a row, its Edit Details.
    func showDetails() {
        follow()
        guard let pane = detail.current?.preferredFirstResponder else { return }
        window?.makeFirstResponder(pane)
    }

    /// The list in the middle pane, if it is a task list — what the Task menu acts on.
    var taskList: TaskListViewController? { content.current as? TaskListViewController }
    var day: DayViewController? { content.current as? DayViewController }

    /// Whether the window has been shown before: the first time, focus starts in the list.
    private var opened = false

    override func showWindow(_ sender: Any?) {
        super.showWindow(sender)
        guard !opened else { return }
        opened = true
        focusList(tries: 20)
    }

    /// Puts focus in the middle pane's list once SwiftUI has made it: left to AppKit, the
    /// window's first focus is the first list it finds, the sidebar's.
    private func focusList(tries: Int) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in
            guard let self, let window = self.window else { return }
            if let list = self.content.current?.preferredFirstResponder as? NSOutlineView {
                window.makeFirstResponder(list)
            } else if tries > 0 {
                self.focusList(tries: tries - 1)
            }
        }
    }

    // MARK: - Panes (F6)

    private var panes: [NSView] {
        [sidebar.preferredFirstResponder, content.current?.preferredFirstResponder, detail.current?.preferredFirstResponder]
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
        // Closing hides it: the app stays resident, syncing.
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
