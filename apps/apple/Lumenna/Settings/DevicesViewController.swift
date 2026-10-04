import UIKit

/// Paired devices and how syncing is going, in words rather than an icon (§9, §16.1).
final class DevicesViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Devices")
    }

    override func load() throws -> (items: [Item], count: String) {
        let status = try core.lumenna.syncStatus()
        let items = status.devices.map { device -> Item in
            var detail = [device.platform]
            if device.thisDevice {
                detail.append("this device")
            } else if let error = device.lastError {
                detail.append("last attempt failed: \(error)")
                if let success = device.lastSuccess { detail.append("last synced \(Self.ago(success))") }
            } else if let success = device.lastSuccess {
                detail.append("last synced \(Self.ago(success))")
            } else {
                detail.append("not synced yet")
            }
            return Item(key: device.nodeId, title: device.name, detail: detail.joined(separator: ", "))
        }
        // Sync Now as the first row: in the navigation bar beside Add it crowded the title
        // until it was clipped at large text sizes.
        let syncNow = Item(key: "sync-now", title: "Sync Now", detail: nil)
        return ([syncNow] + items, status.announcement)
    }

    /// "5 minutes ago", in this device's words.
    private static func ago(_ timestamp: String) -> String {
        guard let date = try? Date(timestamp, strategy: .iso8601) else { return timestamp }
        return date.formatted(.relative(presentation: .named))
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
        return [
            ItemAction(title: "Rename") { [weak self] item in
                self?.askForText("Rename \(item.title)", initial: item.title) { name in
                    self?.perform(on: item) { try lumenna.renameDevice(device: item.key, name: name) }
                }
            },
            ItemAction(title: "Unpair", destructive: true) { [weak self] item in
                self?.confirm(
                    "Unpair \(item.title)?",
                    message: "It stops syncing with your devices but keeps everything it already has. Unpairing is for a device you replaced; it does not take data back from a lost one.",
                    action: "Unpair"
                ) {
                    self?.perform(on: item) { try lumenna.unpairDevice(device: item.key) }
                }
            },
        ]
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
