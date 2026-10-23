import Foundation
import LumennaApp
import ServiceManagement

/// Opening at login, the Mac's way of keeping Lumenna running in the background: the app's
/// device setting `background` (`BackgroundKind.macLogin`), whose words are the core's and whose
/// value is the system's. The app hands what the person chose here (a `Turn`'s `background`),
/// and is told what the system now has, or why it refused.
enum LoginItem {
    /// What to tell the app at start, and after each change: the login item as the system has it.
    static var offered: LumennaApp.ShellEvent {
        .background(.offered(kind: .macLogin, on: isOn))
    }

    /// Registers the app as a login item, or unregisters it, and says how that went.
    static func set(_ on: Bool) -> LumennaApp.ShellEvent {
        do {
            if let test = Self.test {
                test.on = on
            } else if on {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
        } catch {
            return .background(.failed(reason: error.localizedDescription))
        }
        return offered
    }

    private static var isOn: Bool {
        test?.on ?? (SMAppService.mainApp.status == .enabled)
    }

    /// The UI tests' copy keeps it here instead: registering would put whatever build is under
    /// test among the Mac's login items, and CI's unsigned build cannot be registered at all.
    private final class Stand { var on = false }
    private static let test: Stand? = ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] == nil ? nil : Stand()
}
