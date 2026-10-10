import SwiftUI

@main
struct LumennaWatchApp: App {
    @StateObject private var opened = Opened()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            Group {
                if let core = opened.core {
                    RootView(core: core)
                        .environmentObject(core)
                        .environmentObject(core.phone)
                } else {
                    // Nothing works without the store, so the reason is the whole screen.
                    ScrollView {
                        Text("Lumenna could not open its store. \(opened.reason)")
                    }
                }
            }
            .onChange(of: phase) { _, phase in
                if phase == .active { opened.core?.phone.syncNow() }
            }
        }
    }
}

/// The store, opened once for the app's life.
private final class Opened: ObservableObject {
    let core: WatchCore?
    let reason: String

    init() {
        do {
            core = try WatchCore()
            reason = ""
        } catch {
            core = nil
            reason = error.sentence
        }
    }
}
