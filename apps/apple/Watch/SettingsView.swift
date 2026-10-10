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

        init(core: WatchCore) {
            _model = StateObject(wrappedValue: SettingsModel(core: core))
        }

        var body: some View {
            List {
                NavigationLink("Planning") { PlanningSettings(model: model).navigationTitle("Planning") }
                NavigationLink("Devices") { DevicesView() }
            }
        }
    }
}

/// The paired devices, as the phone lists them, from the synced list: the watch is not one of
/// them and syncs with none directly, so each says only its platform and version. Rename and
/// Unpair as the phone offers them (`DeviceAction`); pairing a new one is the phone's.
struct DevicesView: View {
    @EnvironmentObject private var core: WatchCore
    @State private var asked: Asked?

    var body: some View {
        let listed: DeviceList? = {
            _ = core.generation
            return core.read { try core.lumenna.devices() }
        }()
        List {
            Text("This watch syncs through your iPhone. Pair a new device from the iPhone.")
                .font(.footnote)
            if let listed, listed.devices.isEmpty {
                Text("No paired devices")
            }
            ForEach(listed?.devices ?? [], id: \.nodeId) { device in
                Button {
                    asked = Asked(ChoicePrompt(title: device.name, choices: DeviceAction.of(thisDevice: device.thisDevice).map { action in
                        (action.title, { run(action, on: device) })
                    }))
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
        .sheet(item: $asked) { asked in
            NavigationStack { asked.view }
        }
    }

    private func run(_ action: DeviceAction, on device: DeviceView) {
        let lumenna = core.lumenna
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
            switch action {
            case .rename:
                asked = Asked(TextPrompt("Rename \(device.name)", initial: device.name) { name in
                    core.act { try lumenna.renameDevice(device: device.nodeId, name: name) }
                })
            case .unpair:
                asked = Asked(ChoicePrompt(title: "Unpair \(device.name)?", message: DeviceAction.unpairing, choices: [
                    ("Unpair", { core.act { try lumenna.unpairDevice(device: device.nodeId) } }),
                ]))
            }
        }
    }
}
