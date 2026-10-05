import UIKit

/// Paired devices and how syncing is going, in words rather than an icon (§9, §16.1).
final class DevicesViewController: ItemListViewController {
    /// This device's identifier, whose row offers no Unpair: the core refuses it.
    private var thisDevice: Set<String> = []

    init(core: Core) {
        super.init(core: core, title: "Devices")
    }

    override func load() throws -> (items: [Item], count: String) {
        let status = try core.lumenna.syncStatus()
        thisDevice = Set(status.devices.filter(\.thisDevice).map(\.nodeId))
        let items = status.devices.map { device -> Item in
            // How syncing with it is going, as the core words it for every app.
            let detail = [device.platform] + device.status
            return Item(key: device.nodeId, title: device.name, detail: detail.joined(separator: ", "))
        }
        // Sync Now as the first row: in the navigation bar beside Add it crowded the title
        // until it was clipped at large text sizes.
        let syncNow = Item(key: "sync-now", title: "Sync Now", detail: nil)
        return ([syncNow] + items, status.announcement)
    }

    override func open(_ item: Item) {
        if item.key == "sync-now" {
            syncNow()
        }
    }

    override var addTitle: String? { "Pair a device" }

    override func add() {
        navigationController?.pushViewController(PairingViewController(core: core), animated: true)
    }

    override func actions(for item: Item) -> [ItemAction] {
        guard item.key != "sync-now" else { return [] }
        let lumenna = core.lumenna
        var actions = [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(item.title)", initial: item.title) { name in
                    self?.perform(on: item) { try lumenna.renameDevice(device: item.key, name: name) }
                }
            },
        ]
        // This device cannot unpair itself, so that is not offered on its own row.
        guard !thisDevice.contains(item.key) else { return actions }
        actions.append(
            ItemAction(title: "Unpair", destructive: true) { [weak self] item in
                self?.confirm(
                    "Unpair \(item.title)?",
                    message: "It stops syncing with your devices but keeps everything it already has. Unpairing is for a device you replaced; it does not take data back from a lost one.",
                    action: "Unpair"
                ) {
                    self?.perform(on: item) { try lumenna.unpairDevice(device: item.key) }
                }
            }
        )
        return actions
    }

    private func syncNow() {
        Announcer.say("Syncing")
        core.syncNow { [weak self] result in
            switch result {
            case let .success(report):
                Announcer.say(report.announcement, notices: report.peers.compactMap { peer in
                    peer.error.map { "\(peer.name): \($0)" }
                })
                self?.reload()
            case let .failure(error):
                self?.showFailure(error.sentence)
            }
        }
    }
}
