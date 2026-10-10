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
        menu.add(devices[clicked].actions) { [weak self] action in self?.perform(action) }
    }

    private var selected: DeviceView? {
        table.selectedRow >= 0 && table.selectedRow < devices.count ? devices[table.selectedRow] : nil
    }

    // The buttons and keys are the selected device's own actions of those kinds: this
    // device's row has no Unpair, so its button does nothing there.
    @objc private func renameSelected() {
        guard let device = selected else { return }
        if let action = device.actions.first(.rename) { perform(action) } else { said(.rename, device) }
    }

    @objc private func unpairSelected() {
        guard let device = selected else { return }
        if let action = device.actions.first(.unpair) { perform(action) } else { said(.unpair, device) }
    }

    /// Why the device does not offer `kind`, in the core's words.
    private func said(_ kind: ActionKind, _ device: DeviceView) {
        view.window?.showFailure(notOffered(kind: kind, subject: .device, thisDevice: device.thisDevice))
    }

    private func perform(_ action: Action) {
        view.window?.run(action, core: core) { [weak self] change, _ in
            self?.reload()
            Announcer.say(change.announcement, notices: change.notices)
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
