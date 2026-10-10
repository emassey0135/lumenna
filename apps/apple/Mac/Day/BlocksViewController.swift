import AppKit

/// Every block series, by when it starts: for the ones on no day near enough to find
/// from the planner.
final class BlocksViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSMenuDelegate {
    private let core: Core
    private var rows: [RowView] = []
    let table = BlocksTable()
    private let count = NSTextField(labelWithString: "")

    init(core: Core, window: MainWindowController) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Blocks"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        count.textColor = .quietLabel
        let add = NSButton(title: "Add Block…", target: self, action: #selector(newBlock(_:)))
        let header = NSStackView(views: [count, NSView(), add])
        header.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 6, right: 12)

        let column = NSTableColumn(identifier: .init("block"))
        table.addTableColumn(column)
        table.headerView = nil
        table.dataSource = self
        table.delegate = self
        table.usesAutomaticRowHeights = true
        table.style = .inset
        table.setAccessibilityLabel("Blocks")
        table.target = self
        table.doubleAction = #selector(editSelected)
        table.returned = { [weak self] in self?.editSelected() }
        table.deleted = { [weak self] in self?.deleteSelected() }
        let menu = NSMenu()
        menu.delegate = self
        table.menu = menu
        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true

        let stack = NSStackView(views: [header, scroll])
        stack.orientation = .vertical
        stack.spacing = 0
        header.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        scroll.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
        view = stack
        NotificationCenter.default.addObserver(self, selector: #selector(reload), name: Core.changed, object: nil)
        reload()
    }

    override var preferredFirstResponder: NSView? { table }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(table)
        if table.selectedRow < 0, !rows.isEmpty { table.selectRowIndexes([0], byExtendingSelection: false) }
    }

    @objc private func reload() {
        let keep = table.selectedRow >= 0 && table.selectedRow < rows.count ? rows[table.selectedRow].id : nil
        let index = table.selectedRow
        do {
            let listing = try core.lumenna.listBlocks()
            rows = listing.rows
            count.stringValue = listing.rows.isEmpty && !listing.empty.isEmpty ? listing.empty : listing.announcement
        } catch {
            count.stringValue = error.sentence
        }
        table.reloadData()
        let target = keep.flatMap { id in rows.firstIndex { $0.id == id } } ?? (index >= 0 ? min(index, rows.count - 1) : nil)
        if let target, target >= 0 { table.selectRowIndexes([target], byExtendingSelection: false) }
    }

    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = TwoLineCell()
        // Said as a task row is: when, in this Mac's clock ("every weekday at 9:00 AM"), then
        // how long.
        let detail = RowSpeech.details(rows[row])
        cell.show(title: rows[row].title, detail: detail)
        cell.setAccessibilityLabel(rows[row].title)
        cell.setAccessibilityValueDescription(detail ?? "")
        return cell
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = table.clickedRow >= 0 ? table.clickedRow : table.selectedRow
        guard clicked >= 0 else { return }
        table.selectRowIndexes([clicked], byExtendingSelection: false)
        menu.add(rows[clicked].actions) { [weak self] action in self?.perform(action) }
    }

    /// Runs one of the selected block's actions, the core's: Edit Block is the block form.
    private func perform(_ action: Action) {
        guard let window = view.window else { return }
        window.run(action, core: core, form: { [weak self] _ in self?.editSelected() }) { [weak self] change, _ in
            self?.reload()
            Announcer.say(change.announcement, notices: change.notices)
        }
    }

    @objc private func editSelected() {
        guard table.selectedRow >= 0, let window = view.window else { return }
        let id = rows[table.selectedRow].id
        do {
            try BlockFormModel.series(core: core, id: id).present(on: window) { [weak self] change in
                self?.reload()
                Announcer.say(change.announcement, notices: change.notices)
            }
        } catch {
            window.showFailure(error.sentence)
        }
    }

    /// Delete: the block's own Delete Block.
    private func deleteSelected() {
        guard table.selectedRow >= 0, let action = rows[table.selectedRow].actions.first(.delete) else { return }
        perform(action)
    }

    @objc func newBlock(_ sender: Any?) {
        guard let window = view.window else { return }
        BlockFormModel(core: core, purpose: .add).present(on: window) { [weak self] change in
            self?.reload()
            Announcer.say(change.announcement, notices: change.notices)
        }
    }
}

/// A table that answers Return and Delete itself.
final class BlocksTable: NSTableView {
    var returned: (() -> Void)?
    var deleted: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        guard event.modifierFlags.intersection([.command, .option, .control]).isEmpty else {
            super.keyDown(with: event)
            return
        }
        switch event.keyCode {
        case 36, 76: returned?()
        case 51, 117: deleted?()
        default: super.keyDown(with: event)
        }
    }
}
