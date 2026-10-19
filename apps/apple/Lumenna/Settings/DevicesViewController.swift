import LumennaApp
import UIKit

/// Paired devices and how syncing is going, in words rather than an icon: the app's Devices
/// page (`ViewModel.devices`), held in its own hand, so a rename or an unpairing lands back here.
final class DevicesViewController: AppListViewController {
    init(core: Core) {
        super.init(core: core, title: "Devices")
    }

    override var side: LumennaApp.Side? { .devices }

    override func appear() {
        // Lists the devices, and asks the network how syncing is going.
        core.app.send(.devices(.show))
    }

    override func rows() -> (items: [AppItem], count: String)? {
        guard let devices = core.app.view.devices else { return nil }
        let lines = core.app.lines.devices
        let listed = (devices.list?.devices ?? []).enumerated().map { index, device in
            // How syncing with it is going, as the core words it for every app.
            let line = index < lines.count ? lines[index] : nil
            return AppItem(key: device.nodeId, title: line?.title ?? device.name, detail: line.flatMap { $0.detail.isEmpty ? nil : $0.detail }, actions: device.actions)
        }
        // Sync Now as the first row: in the navigation bar beside Add it crowds the title
        // until it is clipped at large text sizes.
        let syncNow = AppItem(key: "sync-now", title: "Sync Now")
        let empty = devices.list.map { $0.devices.isEmpty ? $0.empty : "" } ?? ""
        return ([syncNow] + listed, [devices.status, empty].filter { !$0.isEmpty }.joined(separator: ". "))
    }

    override func open(_ item: AppItem) {
        // The app says it is syncing, then how the round went.
        if item.key == "sync-now" { core.app.send(.devices(.syncNow)) }
    }

    override var addTitle: String? { "Pair a device" }

    override func add() {
        navigationController?.pushViewController(PairingViewController(core: core), animated: true)
    }
}
