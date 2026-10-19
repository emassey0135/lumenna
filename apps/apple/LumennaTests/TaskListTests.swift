import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// The task list over the app: each row's swipe actions, custom actions and menu, asked for
/// and run directly, as VoiceOver offers them, without a swipe having to land. The store is
/// seeded through the surface beside it, and the app takes that in.
@MainActor
final class TaskListTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        // A store of its own: Core takes its profile from here.
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    /// `screen` loaded and laid out.
    private func shown<Screen: UIViewController>(_ screen: Screen) -> Screen {
        screen.loadViewIfNeeded()
        screen.view.frame = CGRect(x: 0, y: 0, width: 400, height: 900)
        screen.beginAppearanceTransition(true, animated: false)
        screen.endAppearanceTransition()
        screen.view.layoutIfNeeded()
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
        return screen
    }

    private func list(in screen: UIViewController) -> UICollectionView {
        func find(_ view: UIView) -> UICollectionView? {
            (view as? UICollectionView) ?? view.subviews.lazy.compactMap(find).first
        }
        return find(screen.view)!
    }

    private func titles(_ configuration: UISwipeActionsConfiguration?) -> [String] {
        configuration?.actions.compactMap(\.title) ?? []
    }

    private func titles(_ menu: UIMenu?) -> [String] {
        menu?.children.map(\.title) ?? []
    }

    /// Runs the action called `title`, as VoiceOver's action does.
    private func run(_ title: String, in configuration: UISwipeActionsConfiguration?) {
        guard let action = configuration?.actions.first(where: { $0.title == title }) else {
            return XCTFail("no action called \(title) among \(titles(configuration))")
        }
        action.handler(action, UIView()) { _ in }
    }

    private func path(_ item: Int) -> IndexPath { IndexPath(item: item, section: 0) }

    /// What a row offers, each way: its swipes, its other accessibility actions, its menu.
    private struct Offered {
        var leading: [String] = []
        var trailing: [String]
        var custom: [String]
        var menu: [String]

        /// What VoiceOver, Switch Control and Full Keyboard Access list: the swipes, and the
        /// custom actions UIKit adds to them.
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

    /// The task list of `place` shown, once the app has taken in what was written through the
    /// surface and listed it.
    private func shownTasks(_ place: LumennaApp.Place = .tasks) -> TaskListViewController {
        core.app.tick()
        settle()
        let tasks = shown(TaskListViewController(core: core, place: place))
        settle { core.app.view.place == place && list(in: tasks).numberOfItems(inSection: 0) == shownItems.count }
        return tasks
    }

    /// The app's rows shown, not folded away.
    private var shownItems: [LumennaApp.Item] { (core.app.view.list?.items ?? []).filter { !$0.hidden } }

    private func offered(_ tasks: TaskListViewController, _ item: Int) -> Offered {
        Offered(
            leading: titles(tasks.leadingSwipeActions(at: path(item))), trailing: titles(tasks.trailingSwipeActions(at: path(item))),
            custom: tasks.customActions(at: path(item)).map(\.name), menu: titles(tasks.menu(at: path(item)))
        )
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
    private let otherTaskActions = ["Edit Details", "Put in a Block", "Move to Project", "Make Subtask Of", "Wait For"]

    func testATaskSwipesMarkDoneOneWayAndMoveToTrashTheOtherAndOffersTheRestOnceAsActions() throws {
        _ = try core.lumenna.addTask(text: "water the plants")
        let tasks = shownTasks()
        let row = offered(tasks, 0)
        XCTAssertEqual(row.leading, ["Mark Done"])
        XCTAssertEqual(row.trailing, ["Move to Trash"])
        XCTAssertEqual(row.custom, otherTaskActions)
        assertEachOnce(row, shownItems[0].row.actions)
        run("Mark Done", in: tasks.leadingSwipeActions(at: path(0)))
        settle()
        XCTAssertTrue(try core.lumenna.listTasks(query: "").rows.isEmpty, "completed, so no longer listed")
    }

    func testTheCellCarriesTheOtherActionsForVoiceOver() throws {
        _ = try core.lumenna.addTask(text: "water the plants")
        let tasks = shownTasks()
        let cell = try XCTUnwrap(list(in: tasks).cellForItem(at: path(0)))
        XCTAssertEqual(cell.accessibilityCustomActions?.map(\.name), otherTaskActions)
    }

    func testATaskWithSubtasksAlsoOffersCollapse() throws {
        _ = try core.lumenna.addTask(text: "essay")
        _ = try core.lumenna.addTask(text: "outline")
        let rows = try core.lumenna.listTasks(query: "").rows
        let essay = try XCTUnwrap(rows.first { $0.title == "essay" })
        let outline = try XCTUnwrap(rows.first { $0.title == "outline" })
        _ = try core.lumenna.moveTask(id: outline.id, to: .parent(id: essay.id))
        let tasks = shownTasks()
        XCTAssertEqual(titles(tasks.trailingSwipeActions(at: path(0))), ["Move to Trash", "Collapse"])
        assertEachOnce(offered(tasks, 0), shownItems[0].row.actions)
        run("Collapse", in: tasks.trailingSwipeActions(at: path(0)))
        settle { list(in: tasks).numberOfItems(inSection: 0) == 1 }
        XCTAssertEqual(list(in: tasks).numberOfItems(inSection: 0), 1, "the subtask is folded away")
        XCTAssertEqual(titles(tasks.trailingSwipeActions(at: path(0))), ["Move to Trash", "Expand"])
        XCTAssertEqual(titles(tasks.menu(at: path(0))).last, "Expand")
    }

    func testATrashedTaskOffersRestoreAndDeleteFromTrash() throws {
        let added = try core.lumenna.addTask(text: "old idea")
        _ = try core.lumenna.trashTask(id: try XCTUnwrap(added.task?.id))
        let trash = shownTasks(.trash)
        XCTAssertNil(trash.leadingSwipeActions(at: path(0)), "nothing to mark done in the trash")
        XCTAssertEqual(titles(trash.trailingSwipeActions(at: path(0))), ["Restore", "Delete from Trash"])
        assertEachOnce(offered(trash, 0), shownItems[0].row.actions)
        run("Restore", in: trash.trailingSwipeActions(at: path(0)))
        settle()
        XCTAssertEqual(try core.lumenna.listTasks(query: "").rows.map(\.title), ["old idea"])
    }

    func testMarkingDoneFromTheKeyboardIsTheRowsOwnAction() throws {
        _ = try core.lumenna.addTask(text: "water the plants")
        let tasks = shownTasks()
        let list = list(in: tasks)
        list.selectItem(at: path(0), animated: false, scrollPosition: [])
        XCTAssertTrue(tasks.canPerformAction(#selector(TaskListViewController.moveToTrash), withSender: nil))
        tasks.toggleDone()
        settle()
        XCTAssertTrue(try core.lumenna.listTasks(query: "").rows.isEmpty, "completed, so no longer listed")
    }

}
