import UIKit
import XCTest
@testable import Lumenna

/// The iPhone's keyboard commands, run as UIKit runs a key command: down the responder chain
/// from the screen in front. For a key the simulator keeps from the app (⌘F, from its second
/// launch on), this is what shows the command does what it says.
@MainActor
final class KeyboardCommandTests: XCTestCase {
    private var core: Core!
    private var window: UIWindow!

    override func setUp() async throws {
        // A store of its own: Core takes its profile from here.
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    override func tearDown() async throws {
        window?.isHidden = true
        window = nil
    }

    /// The iPhone's root in a window of its own, in front, on Today.
    private func showRoot() throws -> RootTabs {
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let root = RootTabs(core: core)
        window = UIWindow(windowScene: scene)
        window.rootViewController = root
        window.makeKeyAndVisible()
        settle()
        return root
    }

    private func settle(_ seconds: TimeInterval = 0.2) {
        RunLoop.current.run(until: Date(timeIntervalSinceNow: seconds))
    }

    func testCommandFIsTheIPhonesFilterTasks() {
        let filter = KeyboardCommands.all.first { $0.input == "f" && $0.modifierFlags == .command }
        XCTAssertEqual(filter?.action, #selector(CommandActions.filterTasks))
        XCTAssertEqual(filter?.title, "Filter Tasks")
    }

    func testFilterTasksFromTodayOpensTheTasksWithTheFilterInHand() throws {
        let root = try showRoot()
        XCTAssertEqual(root.selectedIndex, 0)
        // Sent as a key command sends it: to whatever answers it, from the first responder.
        XCTAssertTrue(UIApplication.shared.sendAction(#selector(CommandActions.filterTasks), to: nil, from: nil, for: nil))
        var field: UIView?
        for _ in 0..<50 where field == nil {
            settle(0.1)
            // The Tasks tab's one text field, SwiftUI's, whose name is on SwiftUI's own element.
            let tasks = root.selectedViewController?.view
            field = (UIResponder.first as? UIView).flatMap { ($0 is UITextView || $0 is UITextField) && tasks.map($0.isDescendant) == true ? $0 : nil }
        }
        XCTAssertEqual(root.selectedIndex, 1)
        XCTAssertNotNil(field, "the Filter is first responder")
    }
}
