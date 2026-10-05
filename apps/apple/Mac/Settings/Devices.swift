import AppKit

/// Paired devices and how syncing is going, in words rather than an icon.
final class DevicesViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate, NSMenuDelegate {
    private let core: Core
    private var devices: [DeviceView] = []
    private let status = NSTextField(wrappingLabelWithString: "")
    private let table = BlocksTable()

    init(core: Core) {
        self.core = core
        super.init(nibName: nil, bundle: nil)
        title = "Devices"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        let column = NSTableColumn(identifier: .init("device"))
        table.addTableColumn(column)
        table.headerView = nil
        table.dataSource = self
        table.delegate = self
        table.usesAutomaticRowHeights = true
        table.setAccessibilityLabel("Paired devices")
        table.deleted = { [weak self] in self?.unpairSelected() }
        table.returned = { [weak self] in self?.renameSelected() }
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
        NotificationCenter.default.addObserver(self, selector: #selector(reload), name: Core.changed, object: nil)
        reload()
    }

    @objc private func reload() {
        do {
            let sync = try core.lumenna.syncStatus()
            devices = sync.devices
            status.stringValue = ([sync.announcement] + sync.notices).joined(separator: ". ")
        } catch {
            status.stringValue = error.sentence
        }
        table.reloadData()
    }

    /// Its platform, then how syncing with it is going, as the core words it for every app.
    private static func detail(_ device: DeviceView) -> String {
        ([device.platform] + device.status).joined(separator: ", ")
    }

    func numberOfRows(in tableView: NSTableView) -> Int { devices.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = TwoLineCell()
        let detail = Self.detail(devices[row])
        cell.show(title: devices[row].name, detail: detail)
        cell.setAccessibilityLabel(devices[row].name)
        cell.setAccessibilityValueDescription(detail)
        return cell
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let clicked = table.clickedRow >= 0 ? table.clickedRow : table.selectedRow
        guard clicked >= 0 else { return }
        table.selectRowIndexes([clicked], byExtendingSelection: false)
        menu.addItem(ClosureMenuItem(title: "Rename…") { [weak self] in self?.renameSelected() })
        if !devices[clicked].thisDevice {
            menu.addItem(ClosureMenuItem(title: "Unpair…") { [weak self] in self?.unpairSelected() })
        }
    }

    private var selected: DeviceView? {
        table.selectedRow >= 0 && table.selectedRow < devices.count ? devices[table.selectedRow] : nil
    }

    @objc private func renameSelected() {
        guard let device = selected, let window = view.window else { return }
        window.askForText("Rename \(device.name)", initial: device.name) { [weak self] name in
            self?.change { try self!.core.lumenna.renameDevice(device: device.nodeId, name: name) }
        }
    }

    @objc private func unpairSelected() {
        guard let device = selected, !device.thisDevice, let window = view.window else { return }
        window.confirm(
            "Unpair \(device.name)?",
            message: "It stops syncing with your devices but keeps everything it already has. Unpairing is for a device you replaced; it does not take data back from a lost one.",
            action: "Unpair"
        ) { [weak self] in
            self?.change { try self!.core.lumenna.unpairDevice(device: device.nodeId) }
        }
    }

    private func change(_ operation: () throws -> Change) {
        do {
            let change = try operation()
            reload()
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            view.window?.showFailure(error.sentence)
        }
    }

    @objc func syncNow() {
        Announcer.say("Syncing")
        AppDelegate.shared?.syncNow { [weak self] in self?.reload() }
    }

    @objc private func pair() {
        guard let window = view.window else { return }
        PairingSheet.present(on: window, core: core)
    }
}
