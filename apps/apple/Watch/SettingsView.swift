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
            PlanningSettings(model: model)
        }
    }
}
