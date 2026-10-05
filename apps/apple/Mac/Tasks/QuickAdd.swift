import AppKit

/// Adding a task the way it would be said: one line, read back as it is typed.
///
/// The readback under the field is what a sighted user gets from inline highlighting: what
/// will be saved, with the date resolved. It is not spoken as it changes — speech on every
/// keystroke buries the typing — but is the announcement when the task is added.
final class QuickAddViewController: NSViewController {
    private let core: Core
    private let initial: String
    private let finished: (Change?) -> Void
    private lazy var field = CompletingField(core: core, syntax: .quickAdd, name: "New task")
    private let readback = NSTextField(wrappingLabelWithString: "")
    private let add = NSButton(title: "Add", target: nil, action: nil)

    init(core: Core, initial: String, finished: @escaping (Change?) -> Void) {
        self.core = core
        self.initial = initial
        self.finished = finished
        super.init(nibName: nil, bundle: nil)
        title = "New Task"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        field.stringValue = initial
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
        let empty = text.trimmingCharacters(in: .whitespaces).isEmpty
        add.isEnabled = !empty
        guard !empty, let preview = try? core.lumenna.previewTask(text: text) else {
            readback.stringValue = ""
            return
        }
        // Everything worth saying before confirming, errors included: there is no squiggle
        // under the text, so this is the only channel.
        readback.stringValue = ([preview.announcement] + preview.diagnostics.map(\.message)).joined(separator: ". ")
    }

    @objc private func save() {
        guard add.isEnabled else { return }
        do {
            let change = try core.lumenna.addTask(text: field.stringValue)
            NotificationCenter.default.post(name: Core.changed, object: nil)
            finished(change)
        } catch {
            view.window?.showFailure(error.sentence)
        }
    }

    @objc private func cancel() {
        finished(nil)
    }
}

/// Quick add as a sheet on the main window.
enum QuickAddSheet {
    static func present(on window: NSWindow, core: Core, initial: String = "", added: @escaping (Change) -> Void) {
        var sheet: NSWindow?
        let controller = QuickAddViewController(core: core, initial: initial) { change in
            if let sheet { window.endSheet(sheet) }
            if let change { added(change) }
        }
        sheet = NSWindow(contentViewController: controller)
        window.beginSheet(sheet!)
    }
}

/// Quick add from anywhere: a floating panel the global shortcut opens over whatever
/// is in front, which takes one line and goes away.
final class QuickAddPanel {
    private let core: Core
    private var panel: NSPanel?

    init(core: Core) {
        self.core = core
    }

    func show() {
        if let panel {
            panel.makeKeyAndOrderFront(nil)
            return
        }
        let controller = QuickAddViewController(core: core, initial: "") { [weak self] change in
            self?.panel?.close()
            self?.panel = nil
            if let change {
                // Said after the panel has gone, so it is not cut off by focus returning.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    Announcer.say(change.announcement, notices: change.notices)
                }
            }
        }
        let panel = NSPanel(contentViewController: controller)
        panel.styleMask = [.titled, .closable, .nonactivatingPanel]
        panel.title = "Quick Add"
        panel.level = .floating
        panel.isReleasedWhenClosed = false
        panel.center()
        self.panel = panel
        NSApp.activate(ignoringOtherApps: true)
        panel.makeKeyAndOrderFront(nil)
    }
}
