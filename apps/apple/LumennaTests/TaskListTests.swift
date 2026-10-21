import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// The task list over the app: each row's actions as the list offers them (its swipes, its
/// other accessibility actions, its menu: `TaskListScreen.actions`), asked for and run
/// directly, without a swipe having to land. What VoiceOver lists of them is the VoiceOver UI
/// tests'. The store is seeded through the device, and the app takes that in.
@MainActor
final class TaskListTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        // A store of its own: Core takes its profile from here.
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    /// What a row offers, each way: its swipes, its other accessibility actions, its menu.
    private struct Offered {
        var leading: [String] = []
        var trailing: [String]
        var custom: [String]
        var menu: [String]

        /// What VoiceOver, Switch Control and Full Keyboard Access list: the swipes, and the
        /// accessibility actions SwiftUI adds to them.
        var listed: [String] { leading + trailing + custom }
    }

    /// Runs the main loop until the app is still and `until` holds, as a person waits for the
    /// screen to settle: the app answers on a thread of its own.
    private func settle(_ until: () -> Bool = { true }) {
        let deadline = Date().addingTimeInterval(10)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !(core.app.isStill && until()) && Date() < deadline
    }

    /// The task list of `place`, opened as its screen opens it, once the app has taken in what
    /// was seeded and listed it.
    private func shownTasks(_ place: LumennaApp.Place = .tasks) -> TaskListModel {
        core.app.tick()
        settle()
        let tasks = TaskListModel(core: core, place: place)
        tasks.appear()
        settle { core.app.view.place == place && tasks.shown.count == shownItems.count }
        return tasks
    }

    /// The app's rows shown, not folded away.
    private var shownItems: [LumennaApp.Item] { (core.app.view.list?.items ?? []).filter { !$0.hidden } }

    /// The row's actions as the list splits them, each run as the list runs it.
    private func split(_ tasks: TaskListModel, _ item: Int) -> AppRowActions {
        let row = tasks.shown[item].row
        return TaskListScreen.actions(of: row) { [weak tasks] action in tasks?.run(action, on: row) }
    }

    private func offered(_ tasks: TaskListModel, _ item: Int) -> Offered {
        let split = split(tasks, item)
        return Offered(
            leading: split.leading.map(\.title), trailing: split.trailing.map(\.title),
            custom: split.others.map(\.title), menu: split.actions.map(\.title)
        )
    }

    /// Runs the row's action called `title`, as VoiceOver's action does.
    private func run(_ title: String, _ tasks: TaskListModel, _ item: Int) {
        let split = split(tasks, item)
        guard let action = split.actions.first(where: { $0.title == title }) else {
            return XCTFail("no action called \(title) among \(split.actions.map(\.title))")
        }
        split.run(action)
    }

    private func assertEachOnce(_ offered: Offered, _ actions: [LumennaApp.Action], file: StaticString = #filePath, line: UInt = #line) {
        let names = actions.map(\.title)
        let listed = offered.listed
        XCTAssertEqual(listed.sorted(), names.sorted(), "each action listed once: \(offered)", file: file, line: line)
        XCTAssertEqual(Set(listed).count, listed.count, "nothing listed twice: \(offered)", file: file, line: line)
        let swiped = Set(actions.filter { $0.primary || [.expand, .collapse].contains($0.kind) }.map(\.title))
        XCTAssertTrue((offered.leading + offered.trailing).allSatisfy(swiped.contains), "only primary actions and folding swipe: \(offered)", file: file, line: line)
        XCTAssertEqual(offered.menu, names, "the menu has every action, in the app's order: \(offered)", file: file, line: line)
    }

    /// A task's actions after Mark Done, as the core gives them; Move to Trash is swiped.
    private let otherTaskActions = ["Edit Details", "Put in a Block", "Move to Project", "Make Subtask Of", "Wait For", "Reminders"]

    func testATaskSwipesMarkDoneOneWayAndMoveToTrashTheOtherAndOffersTheRestOnceAsActions() throws {
        _ = try core.app.device.addTask(text: "water the plants")
        let tasks = shownTasks()
        let row = offered(tasks, 0)
        XCTAssertEqual(row.leading, ["Mark Done"])
        XCTAssertEqual(row.trailing, ["Move to Trash"])
        XCTAssertEqual(row.custom, otherTaskActions)
        assertEachOnce(row, shownItems[0].row.actions)
        run("Mark Done", tasks, 0)
        settle()
        XCTAssertTrue(try core.app.device.listTasks(query: "").rows.isEmpty, "completed, so no longer listed")
    }

    func testATaskWithSubtasksAlsoOffersCollapse() throws {
        _ = try core.app.device.addTask(text: "essay")
        _ = try core.app.device.addTask(text: "outline")
        let rows = try core.app.device.listTasks(query: "").rows
        let essay = try XCTUnwrap(rows.first { $0.title == "essay" })
        let outline = try XCTUnwrap(rows.first { $0.title == "outline" })
        _ = try core.app.device.moveTask(id: outline.id, to: .parent(id: essay.id))
        let tasks = shownTasks()
        XCTAssertEqual(offered(tasks, 0).trailing, ["Move to Trash", "Collapse"])
        assertEachOnce(offered(tasks, 0), shownItems[0].row.actions)
        run("Collapse", tasks, 0)
        settle { tasks.shown.count == 1 }
        XCTAssertEqual(tasks.shown.count, 1, "the subtask is folded away")
        XCTAssertEqual(offered(tasks, 0).trailing, ["Move to Trash", "Expand"])
        XCTAssertEqual(offered(tasks, 0).menu.last, "Expand")
    }

    func testATrashedTaskOffersRestoreAndDeleteFromTrash() throws {
        let added = try core.app.device.addTask(text: "old idea")
        _ = try core.app.device.trashTask(id: try XCTUnwrap(added.task?.id))
        let trash = shownTasks(.trash)
        XCTAssertTrue(offered(trash, 0).leading.isEmpty, "nothing to mark done in the trash")
        XCTAssertEqual(offered(trash, 0).trailing, ["Restore", "Delete from Trash"])
        assertEachOnce(offered(trash, 0), shownItems[0].row.actions)
        run("Restore", trash, 0)
        settle()
        XCTAssertEqual(try core.app.device.listTasks(query: "").rows.map(\.title), ["old idea"])
    }

    func testMarkingDoneFromTheKeyboardIsTheRowsOwnAction() throws {
        _ = try core.app.device.addTask(text: "water the plants")
        core.app.tick()
        settle()
        let tasks = TaskListViewController(core: core, place: .tasks)
        tasks.loadViewIfNeeded()
        tasks.beginAppearanceTransition(true, animated: false)
        tasks.endAppearanceTransition()
        // The first row is in hand as soon as the list opens: the Task menu's commands act on it.
        settle { core.app.view.list?.items.isEmpty == false && core.app.view.focus != nil }
        XCTAssertTrue(tasks.canPerformAction(#selector(TaskListViewController.moveToTrash), withSender: nil))
        tasks.toggleDone()
        settle()
        XCTAssertTrue(try core.app.device.listTasks(query: "").rows.isEmpty, "completed, so no longer listed")
    }
}
