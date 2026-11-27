import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// What a screen offers of its own beside its rows, the core's (`ViewModel.screenActions`), as
/// the iPhone and iPad draw it: Undo, Redo and New in the navigation bar, a list's rest in its
/// Place Actions menu, the day's among its buttons; and the settings' pages as the core lists
/// them for this device.
@MainActor
final class ScreenActionTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        // A store of its own: Core takes its profile from here.
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    /// Runs the main loop until the app is still and `until` holds, as a person waits for the
    /// screen to settle: the app answers on a thread of its own.
    private func settle(_ until: () -> Bool = { true }) {
        let deadline = Date().addingTimeInterval(10)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !(core.app.isStill && until()) && Date() < deadline
    }

    /// `screen` loaded and brought to the front, as its tab or column shows it, once its bar is
    /// drawn.
    private func shown(_ screen: UIViewController) {
        core.app.tick()
        settle()
        screen.loadViewIfNeeded()
        screen.beginAppearanceTransition(true, animated: false)
        screen.endAppearanceTransition()
        settle { !(screen.navigationItem.rightBarButtonItems ?? []).isEmpty }
    }

    private func names(_ items: [UIBarButtonItem]?) -> [String] {
        (items ?? []).compactMap(\.accessibilityLabel)
    }

    func testAProjectsBarIsItsOwnActionsInTheCoresOrderTheRestInPlaceActions() throws {
        _ = try core.app.device.addProject(name: "Garden", parent: nil)
        let list = TaskListViewController(core: core, place: .project("Garden"))
        shown(list)
        XCTAssertEqual(core.app.view.screenActions.map(\.id), ["new-task", "filter", "new-project-inside", "new-section", "undo", "redo"])
        XCTAssertEqual(names(list.navigationItem.leftBarButtonItems), ["Undo", "Redo"])
        // Laid from the edge inwards, so read from the leading side in the core's order.
        let trailing = Array((list.navigationItem.rightBarButtonItems ?? []).reversed())
        XCTAssertEqual(names(trailing), ["New Task", Wording.placeActions])
        let menu = try XCTUnwrap(trailing.last?.menu)
        XCTAssertEqual(menu.children.map(\.title), ["Filter Tasks", "New Project Inside", "New Section"])
    }

    func testTheDaysBarHoldsNewBlockUndoAndRedoAndItsButtonsTheRest() {
        let day = DayViewController(core: core)
        shown(day)
        XCTAssertEqual(names(day.navigationItem.leftBarButtonItems), ["Undo", "Redo"])
        XCTAssertEqual(names(day.navigationItem.rightBarButtonItems), ["New Block"])
        let besides = core.app.view.screenActions.filter { !ScreenActionBar.holds($0) }.map(\.id)
        XCTAssertEqual(besides, ["previous-day", "next-day", "go-to-day", "go-to-now", "time-left"])
    }

    func testTheFilterFieldIsWhereTheCoreOffersFilterTasks() {
        let tasks = TaskListModel(core: core, place: .tasks)
        tasks.appear()
        settle { !tasks.screenActions.isEmpty }
        XCTAssertTrue(tasks.filters)
        let blocks = TaskListModel(core: core, place: .blocks)
        blocks.appear()
        settle { !blocks.screenActions.isEmpty }
        XCTAssertFalse(blocks.filters, "Blocks offers no filter")
        XCTAssertEqual(blocks.screenActions.map(\.id), ["new-block", "undo", "redo"])
    }

    func testNewTaskIsTheCoresKeyAndQuickAddComesUpOnTheLineItOpens() throws {
        let tasks = TaskListModel(core: core, place: .tasks)
        tasks.appear()
        settle { !tasks.screenActions.isEmpty }
        let new = try XCTUnwrap(tasks.screenActions.first { $0.id == "new-task" })
        var opened = false
        core.app.perform(new, lineOpened: { opened = true })
        settle { opened }
        XCTAssertTrue(opened, "quick add follows the line the app opened")
        XCTAssertNotNil(core.app.view.line)
    }

    func testSettingsListsTheCoresPagesForThisDeviceWithoutGeneral() {
        let settings = SettingsViewController(core: core)
        settings.loadViewIfNeeded()
        settings.appear()
        settle { core.app.view.settings != nil }
        let listed = settings.rows()?.items.map(\.key) ?? []
        XCTAssertEqual(listed, core.app.view.settings?.pages.map(\.id))
        XCTAssertFalse(listed.contains("general"), "an iPhone keeps nothing running and follows its own clock: \(listed)")
        XCTAssertTrue(listed.contains("export"))
        let about = core.app.view.settings?.pages.first { $0.id == "about" }
        XCTAssertEqual(about?.lines, LumennaApp.settingsPages(ownClock: true, sentence: false).first { $0.id == "about" }?.lines,
                       "an iPhone has no About of its own, so About is a page of the core's lines")
        XCTAssertTrue(about?.lines.contains { $0.contains("https://") } ?? false, "with the website to open")
    }
}
