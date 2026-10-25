import AppKit
import LumennaApp
import SwiftUI

/// The app's Devices page (`ViewModel.devices`) as the page shows it.
final class DevicesModel: ObservableObject {
    let core: Core
    /// Each device, and its line in this device's words: its platform, then how syncing with
    /// it is going.
    @Published private(set) var devices: [(device: LumennaApp.DeviceView, line: LumennaApp.Line?)] = []
    /// How syncing is going, and what is empty.
    @Published private(set) var status = ""
    /// The device in the page's own hand.
    @Published private(set) var held: String?
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.draw()
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Lists the devices, and asks the network how syncing is going.
    func appear() {
        core.app.send(.devices(.show))
        draw()
    }

    private func draw() {
        guard let page = core.app.view.devices else { return }
        let lines = core.app.lines.devices
        let listed = (page.list?.devices ?? []).enumerated().map { index, device in
            (device: device, line: index < lines.count ? lines[index] : nil)
        }
        if listed.map(\.device) != devices.map(\.device) || listed.map(\.line) != devices.map(\.line) {
            devices = listed
        }
        let empty = page.list.map { $0.devices.isEmpty ? $0.empty : "" } ?? ""
        let status = [page.status, empty].filter { !$0.isEmpty }.joined(separator: ". ")
        if self.status != status { self.status = status }
        let held = core.app.view.devicesHeld.focus
        if self.held != held { self.held = held }
    }

    /// The person moved to a device: held in the page's hand at once.
    func moved(to id: String) {
        guard id != held else { return }
        core.app.send(.hold(field0: .devices, field1: id))
        held = id
    }

    var inHand: LumennaApp.DeviceView? {
        held.flatMap { held in devices.first { $0.device.nodeId == held }?.device }
    }

    /// The device in hand's own action of `kind`, or the app says why it has none.
    func run(_ kind: LumennaApp.ActionKind) {
        guard let device = inHand else { return }
        if let action = device.offered.first(where: { $0.kind == kind }) {
            core.app.run(action, on: device.nodeId, side: .devices)
        } else {
            core.app.refuse([kind], on: device.nodeId, side: .devices)
        }
    }
}

/// Paired devices and how syncing is going, in words rather than an icon, held in the page's
/// own hand, so a rename or an unpairing lands back here. Each row is named by its whole line,
/// as the other lists' are; Return renames the device in hand and Delete unpairs it, as its
/// buttons do; its context menu has every one of its actions.
struct DevicesScreen: View {
    @ObservedObject var model: DevicesModel
    let pair: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if !model.status.isEmpty {
                Text(model.status).fixedSize(horizontal: false, vertical: true)
            }
            List(selection: Binding(get: { model.held }, set: { if let id = $0 { model.moved(to: id) } })) {
                ForEach(model.devices, id: \.device.nodeId) { shown in
                    row(shown.device, shown.line).tag(shown.device.nodeId)
                }
            }
            .accessibilityLabel("Paired devices")
            .frame(height: 200)
            .contextMenu(forSelectionType: String.self) { ids in
                if let id = ids.first, let device = model.devices.first(where: { $0.device.nodeId == id })?.device {
                    ForEach(device.offered, id: \.self) { action in
                        Button(action.asksFirst ? action.title + "…" : action.title) {
                            model.core.app.run(action, on: device.nodeId, side: .devices)
                        }
                    }
                }
            }
            HStack {
                Button("Sync Now") { model.core.app.send(.devices(.syncNow)) }
                Button("Pair a Device…") { pair() }
                Spacer()
                Button("Rename…") { model.run(.rename) }
                Button("Unpair…") { model.run(.unpair) }
            }
        }
        .padding(16)
        .frame(width: 520)
    }

    /// Return renames the device in hand, Delete unpairs it, the core's Delete (`ListKeys`).
    func key(_ key: ListKey) -> Bool {
        guard let device = model.inHand else { return false }
        switch key {
        case .returnKey: model.run(.rename)
        case .delete: model.core.app.key("delete", on: device.nodeId, side: .devices)
        case .space: return false
        }
        return true
    }

    private func row(_ device: LumennaApp.DeviceView, _ line: LumennaApp.Line?) -> some View {
        let title = line?.title ?? device.name
        let detail = line?.detail ?? ""
        return VStack(alignment: .leading, spacing: 2) {
            Text(title)
            if !detail.isEmpty {
                Text(detail).font(.subheadline).foregroundStyle(Color.quietLabel)
            }
        }
        // A group named by its whole line, as the task list's rows are.
        .accessibilityElement(children: .contain)
        .accessibilityLabel(line?.text ?? title)
        .accessibilityIdentifier(device.name)
    }
}

/// The Devices tab of Settings, hosted by a view that steps out of VoiceOver's tree.
final class DevicesViewController: NSViewController {
    private let core: Core
    private let model: DevicesModel
    private var screen: DevicesScreen!
    private var clicks: FocusOnClick?
    private var keys: ListKeys?

    init(core: Core) {
        self.core = core
        model = DevicesModel(core: core)
        super.init(nibName: nil, bundle: nil)
        title = "Devices"
        screen = DevicesScreen(
            model: model,
            pair: { [weak self] in
                guard let window = self?.view.window else { return }
                PairingSheet.present(on: window, core: core)
            }
        )
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func loadView() {
        view = FlatHostingView(rootView: screen)
    }

    override func viewWillAppear() {
        super.viewWillAppear()
        model.appear()
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        view.whenOutline { [weak self] outline in
            guard let self else { return }
            if self.clicks == nil { self.clicks = FocusOnClick(outline) }
            if self.keys == nil { self.keys = ListKeys(outline) { [screen = self.screen] in screen?.key($0) ?? false } }
        }
    }

    /// The app says it is syncing, then how the round went.
    @objc func syncNow() {
        core.app.send(.devices(.syncNow))
    }
}
