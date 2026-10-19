import LumennaApp
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
                NavigationLink("Devices") { DevicesView() }
            }
        }
    }
}

/// The paired devices, as the phone lists them: the app's Devices page, from the synced list.
/// The watch is not one of them and syncs with none directly, so each says only its platform
/// and version, and the page says the watch syncs through its phone. Each offers its actions,
/// the app's, when tapped; pairing a new one is the phone's.
struct DevicesView: View {
    @EnvironmentObject private var core: WatchCore
    @EnvironmentObject private var asker: WatchAsker

    var body: some View {
        _ = core.generation
        let page = core.app.view.devices
        let lines = core.app.lines.devices
        let devices = page?.list?.devices ?? []
        return List {
            Text("This watch syncs through your iPhone. Pair a new device from the iPhone.")
                .font(.footnote)
            if let list = page?.list, list.devices.isEmpty {
                Text(list.empty)
            }
            ForEach(Array(devices.enumerated()), id: \.element.nodeId) { index, device in
                let line = index < lines.count ? lines[index] : nil
                let title = line?.title ?? device.name
                let detail = line?.detail ?? ""
                Button {
                    asker.offer(title, device.actions, on: device.nodeId, side: .devices)
                } label: {
                    VStack(alignment: .leading) {
                        Text(title)
                        Text(detail).font(.footnote).foregroundStyle(Color.quietLabel)
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(title)
                    .accessibilityValue(detail)
                }
            }
        }
        .navigationTitle("Devices")
        .onAppear { core.app.send(.devices(.show)) }
    }
}
