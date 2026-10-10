import SwiftUI

/// The settings that sync, in the form the iPhone and the Mac show them
/// (`Shared/Forms/Settings.swift`). Backups and exports are the phone's: a watch has nowhere
/// to save a file or pick one.
struct SettingsView: View {
    @EnvironmentObject private var core: WatchCore

    var body: some View {
        Hosted(core: core).navigationTitle("Settings")
    }

    private struct Hosted: View {
        @StateObject private var model: SettingsModel
        private let core: WatchCore

        init(core: WatchCore) {
            self.core = core
            _model = StateObject(wrappedValue: SettingsModel(core: core))
        }

        var body: some View {
            List {
                NavigationLink("Planning") { PlanningSettings(model: model).navigationTitle("Planning") }
                NavigationLink("Devices") { DevicesView(core: core) }
            }
        }
    }
}

/// The paired devices, as the phone lists them, from the synced list: the watch is not one of
/// them and syncs with none directly, so each says only its platform and version. Each
/// offers its actions, the core's, when tapped; pairing a new one is the phone's.
struct DevicesView: View {
    @EnvironmentObject private var core: WatchCore
    @StateObject private var asker: WatchAsker

    init(core: WatchCore) {
        _asker = StateObject(wrappedValue: WatchAsker(core: core))
    }

    var body: some View {
        let listed: DeviceList? = {
            _ = core.generation
            return core.read { try core.lumenna.devices() }
        }()
        List {
            Text("This watch syncs through your iPhone. Pair a new device from the iPhone.")
                .font(.footnote)
            if let listed, listed.devices.isEmpty {
                Text(listed.empty)
            }
            ForEach(listed?.devices ?? [], id: \.nodeId) { device in
                Button {
                    asker.offer(device.name, device.actions)
                } label: {
                    VStack(alignment: .leading) {
                        Text(device.name)
                        Text(([device.platform] + device.status).joined(separator: ", "))
                            .font(.footnote).foregroundStyle(Color.quietLabel)
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(device.name)
                    .accessibilityValue(([device.platform] + device.status).joined(separator: ", "))
                }
            }
        }
        .navigationTitle("Devices")
        .sheet(item: $asker.asked) { asked in
            NavigationStack { asked.view }
        }
    }
}
