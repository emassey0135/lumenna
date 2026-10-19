import AppKit
import LumennaApp

/// Adding a task the way it would be said: the app's quick-add line (`ViewModel.line`), which
/// starts with what the place shown adds (`#Work ` in a project's list), read back as it is
/// typed.
///
/// The readback under the field is what a sighted user gets from inline highlighting: what
/// will be saved, with the date resolved. It is not spoken as it changes — speech on every
/// keystroke buries the typing — but is what the app says when the task is added.
final class QuickAddViewController: NSViewController {
    private let core: Core
    /// Whether the line starts empty, whatever the place shown adds: from anywhere.
    private let fresh: Bool
    private let finished: () -> Void
    private lazy var field = CompletingField(core: core, syntax: .quickAdd, name: "New task")
    private let readback = NSTextField(wrappingLabelWithString: "")
    private let add = NSButton(title: "Add", target: nil, action: nil)
    private var observer: NSObjectProtocol?
    /// Whether the line has been added or put away, so it is put away once.
    private var closed = false

    init(core: Core, fresh: Bool = false, finished: @escaping () -> Void) {
        self.core = core
        self.fresh = fresh
        self.finished = finished
        super.init(nibName: nil, bundle: nil)
        title = "New Task"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    override func loadView() {
        field.placeholderString = "write the chapter tomorrow p1 #Work"
        field.changed = { [weak self] in self?.textChanged() }
        field.submitted = { [weak self] in self?.save() }
        readback.textColor = .quietLabel
        readback.font = .preferredFont(forTextStyle: .subheadline)
        // Read when VoiceOver reaches it, and changes without interrupting.
        readback.setAccessibilityLabel("Will add")

        add.target = self
        add.action = #selector(save)
        add.keyEquivalent = "\r"
        let cancel = NSButton(title: "Cancel", target: self, action: #selector(cancel))
        cancel.keyEquivalent = "\u{1b}"
        let buttons = NSStackView(views: [NSView(), cancel, add])

        let heading = NSTextField(labelWithString: "New Task")
        heading.font = .preferredFont(forTextStyle: .headline)
        let stack = NSStackView(views: [heading, field, readback, buttons])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        for view in [field, readback, buttons] {
            view.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -32).isActive = true
        }
        stack.widthAnchor.constraint(equalToConstant: 480).isActive = true
        view = stack
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.drawReadback()
        }
        // The line opens with what the place adds, unless it is from anywhere.
        core.app.send(.typing(.open))
        field.stringValue = fresh ? "" : core.app.current.view.line?.text ?? ""
        textChanged()
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(field)
        // The cursor after the prefix, so typing carries on from `#Work `.
        if let editor = field.currentEditor() {
            editor.selectedRange = NSRange(location: (field.stringValue as NSString).length, length: 0)
        }
    }

    private func textChanged() {
        let text = field.stringValue
        add.isEnabled = !text.trimmingCharacters(in: .whitespaces).isEmpty
        let cursor = field.currentEditor()?.selectedRange.location ?? (text as NSString).length
        core.app.send(.typing(.text(text: text, cursor: TextOffsets.bytes(cursor, in: text))))
        drawReadback()
    }

    /// What the line will add, the app's: everything worth saying before confirming, errors
    /// included, since there is no squiggle under the text, so this is the only channel.
    private func drawReadback() {
        readback.stringValue = add.isEnabled ? core.app.view.line?.readback ?? "" : ""
    }

    @objc private func save() {
        guard add.isEnabled else { return }
        // The app adds it, says what it added and puts it in hand in the list; a line it
        // refuses stays here with why.
        core.app.send(.quickAdd(field.stringValue))
        if let refused = core.app.current.view.addRefused {
            view.window?.showFailure(refused)
            return
        }
        closed = true
        finished()
    }

    @objc private func cancel() {
        close()
        finished()
    }

    /// Puts the line away in the app, unless it was added.
    func close() {
        guard !closed else { return }
        closed = true
        core.app.send(.typing(.close))
    }
}

/// Quick add as a sheet on the main window.
enum QuickAddSheet {
    static func present(on window: NSWindow, core: Core) {
        var sheet: NSWindow?
        let controller = QuickAddViewController(core: core) {
            if let sheet { window.endSheet(sheet) }
        }
        sheet = NSWindow(contentViewController: controller)
        window.beginSheet(sheet!)
    }
}

/// Quick add from anywhere: a floating panel the global shortcut opens over whatever
/// is in front, which takes one line and goes away.
final class QuickAddPanel: NSObject, NSWindowDelegate {
    private let core: Core
    private var panel: NSPanel?
    private var controller: QuickAddViewController?

    init(core: Core) {
        self.core = core
    }

    func show() {
        if let panel {
            panel.makeKeyAndOrderFront(nil)
            return
        }
        let controller = QuickAddViewController(core: core, fresh: true) { [weak self] in
            self?.panel?.close()
        }
        let panel = NSPanel(contentViewController: controller)
        panel.styleMask = [.titled, .closable, .nonactivatingPanel]
        panel.title = "Quick Add"
        panel.level = .floating
        panel.isReleasedWhenClosed = false
        panel.delegate = self
        panel.center()
        self.panel = panel
        self.controller = controller
        NSApp.activate(ignoringOtherApps: true)
        panel.makeKeyAndOrderFront(nil)
    }

    /// Closed by its close button too: the line is put away in the app.
    func windowWillClose(_ notification: Notification) {
        controller?.close()
        controller = nil
        panel = nil
    }
}
