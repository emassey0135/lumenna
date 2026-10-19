import AppKit
import LumennaApp

/// Every block series, by when it starts: for the ones on no day near enough to find from the
/// planner. The app's Blocks place (`ViewModel.list`), its row in hand selected.
final class BlocksViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSMenuDelegate {
    private let core: Core
    private var rows: [(item: LumennaApp.Item, line: LumennaApp.Line?)] = []
    let table = BlocksTable()
    private let count = NSTextField(labelWithString: "")
    /// Whether the table is being set from the app's, not moved by the person.
    private var following = false
    /// The landing count this list last moved the selection for.
    private var landed: UInt32?
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Blocks"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

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
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
        landed = core.app.view.landing
        draw()
    }

    override var preferredFirstResponder: NSView? { table }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.window?.makeFirstResponder(table)
    }

    /// Draws the app's Blocks while they are shown, the row in hand selected.
    private func draw() {
        let shown = core.app.view
        guard shown.place == .blocks, let list = shown.list else { return }
        let lines = core.app.lines
        count.stringValue = lines.readback
        let worded = list.items.enumerated().map { index, item in (item, index < lines.list.count ? lines.list[index] : nil) }
        following = true
        defer { following = false }
        if list.items != drawn?.items || lines.list != drawn?.lines {
            drawn = (list.items, lines.list)
            rows = worded.map { (item: $0.0, line: $0.1) }
            table.reloadData()
        }
        let land = landed.map { shown.landing > $0 } ?? false
        landed = shown.landing
        guard let focus = shown.focus, let row = rows.firstIndex(where: { $0.item.row.id == focus }) else { return }
        if table.selectedRow != row { table.selectRowIndexes([row], byExtendingSelection: false) }
        if land { table.scrollRowToVisible(row) }
    }

    /// The rows and lines last drawn, so a redraw that changes nothing leaves the table alone.
    private var drawn: (items: [LumennaApp.Item], lines: [LumennaApp.Line])?

    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = TwoLineCell()
        // Said as a task row is: when, in this Mac's clock ("every weekday at 9:00 AM"), then
        // how long — the app's line.
        let shown = rows[row]
        let detail = shown.line?.detail ?? ""
        cell.show(title: shown.item.row.title, detail: detail.isEmpty ? nil : detail)
        cell.setAccessibilityLabel(shown.item.row.title)
        cell.setAccessibilityValueDescription(detail)
        return cell
    }

    /// The person moved to a row: the app's row in hand.
    func tableViewSelectionDidChange(_ notification: Notification) {
        guard !following, table.selectedRow >= 0, table.selectedRow < rows.count else { return }
        core.app.send(.focus(rows[table.selectedRow].item.row.id))
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = table.clickedRow >= 0 ? table.clickedRow : table.selectedRow
        guard clicked >= 0, clicked < rows.count else { return }
        table.selectRowIndexes([clicked], byExtendingSelection: false)
        let row = rows[clicked].item.row
        menu.add(row.actions) { [weak self] action in self?.core.app.run(action, on: row.id) }
    }

    private var selectedRow: LumennaApp.RowView? {
        table.selectedRow >= 0 && table.selectedRow < rows.count ? rows[table.selectedRow].item.row : nil
    }

    /// Return, or a double click: the block's Edit Block, the block form for every occurrence.
    @objc private func editSelected() {
        guard let row = selectedRow, let edit = row.actions.first(where: { $0.kind == .edit }) else { return }
        core.app.run(edit, on: row.id)
    }

    /// Delete: the block's own Delete Block.
    private func deleteSelected() {
        guard let row = selectedRow else { return }
        if let action = row.actions.first(where: { $0.kind == .delete }) {
            core.app.run(action, on: row.id)
        } else {
            Announcer.say(LumennaApp.notOffered(kind: .delete, subject: .block, thisDevice: false))
        }
    }

    @objc func newBlock(_ sender: Any?) {
        core.app.send(.block(.new(start: nil, minutes: nil)))
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
