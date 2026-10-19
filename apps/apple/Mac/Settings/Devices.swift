import AppKit
import LumennaApp

/// Paired devices and how syncing is going, in words rather than an icon: the app's Devices
/// page (`ViewModel.devices`), held in its own hand, so a rename or an unpairing lands back here.
final class DevicesViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSMenuDelegate {
    private let core: Core
    private var devices: [(device: LumennaApp.DeviceView, line: LumennaApp.Line?)] = []
    private let status = NSTextField(wrappingLabelWithString: "")
    private let table = BlocksTable()
    /// Whether the table is being set from the app's, not moved by the person.
    private var following = false
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Devices"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    override func loadView() {
        let column = NSTableColumn(identifier: .init("device"))
        table.addTableColumn(column)
        table.headerView = nil
        table.dataSource = self
        table.delegate = self
        table.usesAutomaticRowHeights = true
        table.setAccessibilityLabel("Paired devices")
        table.deleted = { [weak self] in self?.run(.unpair) }
        table.returned = { [weak self] in self?.run(.rename) }
        let menu = NSMenu()
        menu.delegate = self
        table.menu = menu
        let scroll = NSScrollView()
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder

        let buttons = NSStackView(views: [
            NSButton(title: "Sync Now", target: self, action: #selector(syncNow)),
            NSButton(title: "Pair a Device…", target: self, action: #selector(pair)),
            NSView(),
            NSButton(title: "Rename…", target: self, action: #selector(renameSelected)),
            NSButton(title: "Unpair…", target: self, action: #selector(unpairSelected)),
        ])
        let stack = NSStackView(views: [status, scroll, buttons])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        for view in [status, scroll, buttons] {
            view.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -32).isActive = true
        }
        scroll.heightAnchor.constraint(equalToConstant: 200).isActive = true
        stack.widthAnchor.constraint(equalToConstant: 520).isActive = true
        view = stack
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
    }

    override func viewWillAppear() {
        super.viewWillAppear()
        // Lists the devices, and asks the network how syncing is going.
        core.app.send(.devices(.show))
        draw()
    }

    private func draw() {
        guard let page = core.app.view.devices else { return }
        let lines = core.app.lines.devices
        let listed = (page.list?.devices ?? []).enumerated().map { index, device in
            (device: device, line: index < lines.count ? lines[index] : nil)
        }
        let empty = page.list.map { $0.devices.isEmpty ? $0.empty : "" } ?? ""
        status.stringValue = [page.status, empty].filter { !$0.isEmpty }.joined(separator: ". ")
        following = true
        defer { following = false }
        if listed.map(\.device) != devices.map(\.device) || listed.map(\.line) != devices.map(\.line) {
            devices = listed
            table.reloadData()
        }
        let held = core.app.view.devicesHeld
        if let focus = held.focus, let row = devices.firstIndex(where: { $0.device.nodeId == focus }), table.selectedRow != row {
            table.selectRowIndexes([row], byExtendingSelection: false)
        }
    }

    func numberOfRows(in tableView: NSTableView) -> Int { devices.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = TwoLineCell()
        // Its platform, then how syncing with it is going, as the core words it for every app.
        let shown = devices[row]
        let title = shown.line?.title ?? shown.device.name
        let detail = shown.line?.detail ?? ""
        cell.show(title: title, detail: detail)
        cell.setAccessibilityLabel(title)
        cell.setAccessibilityValueDescription(detail)
        return cell
    }

    func tableViewSelectionDidChange(_ notification: Notification) {
        guard !following, let device = selected else { return }
        core.app.send(.hold(field0: .devices, field1: device.nodeId))
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = table.clickedRow >= 0 ? table.clickedRow : table.selectedRow
        guard clicked >= 0, clicked < devices.count else { return }
        table.selectRowIndexes([clicked], byExtendingSelection: false)
        let device = devices[clicked].device
        menu.add(device.actions) { [weak self] action in
            self?.core.app.run(action, on: device.nodeId, side: .devices)
        }
    }

    private var selected: LumennaApp.DeviceView? {
        table.selectedRow >= 0 && table.selectedRow < devices.count ? devices[table.selectedRow].device : nil
    }

    // The buttons and keys are the selected device's own actions of those kinds: this
    // device's row has no Unpair, so its button says why there.
    @objc private func renameSelected() { run(.rename) }
    @objc private func unpairSelected() { run(.unpair) }

    private func run(_ kind: LumennaApp.ActionKind) {
        guard let device = selected else { return }
        if let action = device.actions.first(where: { $0.kind == kind }) {
            core.app.run(action, on: device.nodeId, side: .devices)
        } else {
            view.window?.showFailure(LumennaApp.notOffered(kind: kind, subject: .device, thisDevice: device.thisDevice))
        }
    }

    /// The app says it is syncing, then how the round went.
    @objc func syncNow() {
        core.app.send(.devices(.syncNow))
    }

    @objc private func pair() {
        guard let window = view.window else { return }
        PairingSheet.present(on: window, core: core)
    }
}
