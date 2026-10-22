import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// A task's or block's reminder list, the app's: opened by the row's Reminders, its own
/// reminders swiped away, a default not.
@MainActor
final class ReminderListTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    private func settle(_ until: () -> Bool = { true }) {
        let deadline = Date().addingTimeInterval(10)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !(core.app.isStill && until()) && Date() < deadline
    }

    func testABlocksReminderListShowsTheDefaultThenItsOwnWhichSwipesRemove() throws {
        // Tomorrow, so its reminder is still to come whenever the test runs.
        _ = try core.app.device.addBlock(block: newBlock(title: "Deep work", at: "09:00", minutes: 30, date: "tomorrow", kind: "work", repeat: nil))
        core.app.tick()
        settle()
        core.app.send(.open(.blocks))
        settle { core.app.view.list?.items.isEmpty == false }
        let blocks = try XCTUnwrap(core.app.view.list)
        let item = try XCTUnwrap(blocks.items.first)
        // A list's row offers its actions in short, each held once in the list.
        let reminders = try XCTUnwrap(item.actions.map { blocks.offered[Int($0)] }.first { $0.kind == .reminders })
        core.app.act(reminders, on: item.row.id)
        settle { core.app.view.reminders != nil }

        let list = RemindersViewController(core: core)
        list.loadViewIfNeeded()
        list.view.frame = CGRect(x: 0, y: 0, width: 400, height: 900)
        list.beginAppearanceTransition(true, animated: false)
        list.endAppearanceTransition()
        list.view.layoutIfNeeded()
        settle { !list.items.isEmpty }
        XCTAssertEqual(list.items.map(\.title), ["5 minutes before it starts"])
        XCTAssertEqual(list.rowActions(for: list.items[0]).trailing, [], "a default cannot be removed alone")

        // Add Reminder asks for a phrase, the app's question, answered here as a person would.
        list.add()
        settle { core.app.view.asking != nil }
        core.app.send(.answer(.text(text: "at end")))
        settle { list.items.first?.title == "When it ends" }
        XCTAssertEqual(list.items.map(\.title), ["When it ends"])
        let swipes = list.rowActions(for: list.items[0]).trailing.map(\.title)
        XCTAssertEqual(swipes, ["Remove Reminder"])
    }
}
