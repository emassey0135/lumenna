import UIKit
import XCTest
@testable import Lumenna

/// Each list's swipe actions, asked for and run directly: the same list VoiceOver offers as a
/// row's actions, checked without a swipe having to land. A couple of UI tests still swipe,
/// for the gesture itself.
@MainActor
final class SwipeActionTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        // A store of its own: Core takes its profile from here.
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    /// `screen` loaded and laid out, its list filled.
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

    /// Runs the custom action called `title`, as VoiceOver's action does.
    private func run(_ title: String, in actions: [UIAccessibilityCustomAction]) {
        guard let action = actions.first(where: { $0.name == title }) else {
            return XCTFail("no custom action called \(title) among \(actions.map(\.name))")
        }
        _ = action.actionHandler?(action)
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

    private func offered(_ tasks: TaskListViewController, _ item: Int) -> Offered {
        Offered(
            leading: titles(tasks.leadingSwipeActions(at: path(item))), trailing: titles(tasks.trailingSwipeActions(at: path(item))),
            custom: tasks.customActions(at: path(item)).map(\.name), menu: titles(tasks.menu(at: path(item)))
        )
    }

    private func offered(_ list: ItemListViewController, _ item: Int) -> Offered {
        Offered(
            trailing: titles(list.trailingSwipeActions(at: path(item))),
            custom: list.customActions(at: path(item)).map(\.name), menu: titles(list.menu(at: path(item)))
        )
    }

    private func offered(_ day: DayViewController, _ item: Int) -> Offered {
        Offered(
            trailing: titles(day.trailingSwipeActions(at: path(item))),
            custom: day.customActions(at: path(item)).map(\.name), menu: titles(day.menu(at: path(item)))
        )
    }

    /// Every one of the core's `actions` is listed for VoiceOver once, swiped only if primary,
    /// and in the long-press menu in the core's order, with Expand or Collapse after them.
    private func assertEachOnce(_ offered: Offered, _ actions: [Action], fold: String? = nil, file: StaticString = #filePath, line: UInt = #line) {
        let names = actions.map(\.title)
        let listed = offered.listed.filter { $0 != fold }
        XCTAssertEqual(listed.sorted(), names.sorted(), "each action listed once: \(offered)", file: file, line: line)
        XCTAssertEqual(Set(listed).count, listed.count, "nothing listed twice: \(offered)", file: file, line: line)
        let primary = Set(actions.filter(\.primary).map(\.title))
        XCTAssertTrue((offered.leading + offered.trailing).filter { $0 != fold }.allSatisfy(primary.contains), "only primary actions swipe: \(offered)", file: file, line: line)
        XCTAssertEqual(offered.menu, names + [fold].compactMap { $0 }, "the menu has every action: \(offered)", file: file, line: line)
    }

    /// A task's actions after Mark Done, as the core gives them; Move to Trash is swiped.
    private let otherTaskActions = ["Edit Details", "Put in a Block", "Move to Project", "Make Subtask Of", "Wait For"]

    func testATaskSwipesMarkDoneOneWayAndMoveToTrashTheOtherAndOffersTheRestOnceAsActions() throws {
        _ = try core.lumenna.addTask(text: "water the plants")
        let tasks = shown(TaskListViewController(core: core))
        let row = offered(tasks, 0)
        XCTAssertEqual(row.leading, ["Mark Done"])
        XCTAssertEqual(row.trailing, ["Move to Trash"])
        XCTAssertEqual(row.custom, otherTaskActions)
        assertEachOnce(row, try core.lumenna.listTasks(query: "").rows[0].actions)
        run("Mark Done", in: tasks.leadingSwipeActions(at: path(0)))
        XCTAssertTrue(try core.lumenna.listTasks(query: "").rows.isEmpty, "completed, so no longer listed")
    }

    func testTheCellCarriesTheOtherActionsForVoiceOver() throws {
        _ = try core.lumenna.addTask(text: "water the plants")
        let tasks = shown(TaskListViewController(core: core))
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
        let tasks = shown(TaskListViewController(core: core))
        XCTAssertEqual(titles(tasks.trailingSwipeActions(at: path(0))), ["Move to Trash", "Collapse"])
        assertEachOnce(offered(tasks, 0), try core.lumenna.listTasks(query: "").rows[0].actions, fold: "Collapse")
        run("Collapse", in: tasks.trailingSwipeActions(at: path(0)))
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
        XCTAssertEqual(list(in: tasks).numberOfItems(inSection: 0), 1, "the subtask is folded away")
        XCTAssertEqual(titles(tasks.trailingSwipeActions(at: path(0))), ["Move to Trash", "Expand"])
        XCTAssertEqual(titles(tasks.menu(at: path(0))).last, "Expand")
    }

    func testATrashedTaskOffersRestoreAndDeleteFromTrash() throws {
        let added = try core.lumenna.addTask(text: "old idea")
        _ = try core.lumenna.trashTask(id: try XCTUnwrap(added.task?.id))
        let trash = shown(TaskListViewController(core: core, title: "Trash", query: "deleted", mode: .trash))
        XCTAssertNil(trash.leadingSwipeActions(at: path(0)), "nothing to mark done in the trash")
        XCTAssertEqual(titles(trash.trailingSwipeActions(at: path(0))), ["Restore", "Delete from Trash"])
        assertEachOnce(offered(trash, 0), try core.lumenna.listTasks(query: "deleted").rows[0].actions)
        run("Restore", in: trash.trailingSwipeActions(at: path(0)))
        XCTAssertEqual(try core.lumenna.listTasks(query: "").rows.map(\.title), ["old idea"])
    }

    func testTheSidebarOffersNewProjectAndCollapseOnItsProjectsHeading() throws {
        _ = try core.lumenna.addLabel(name: "calls")
        let sidebar = shown(SidebarViewController(core: core) { _ in })
        let rows = sidebar.items.map(\.key)
        let projects = try XCTUnwrap(rows.firstIndex(of: "group:projects"))
        XCTAssertEqual(titles(sidebar.trailingSwipeActions(at: path(projects))), ["New Project", "Collapse"])
        let calls = try XCTUnwrap(rows.firstIndex(of: "label:calls"))
        let label = offered(sidebar, calls)
        XCTAssertEqual(label.trailing, ["Rename", "Delete"], "a label's primary actions, the core's")
        XCTAssertEqual(label.custom, ["Merge Into", "Colour"], "the rest, as Browse offers them: alone, it moves neither up nor down")
        assertEachOnce(label, sidebar.items[calls].actions)
        for (index, item) in sidebar.items.enumerated() {
            let fold = titles(sidebar.trailingSwipeActions(at: path(index))).last.flatMap { ["Collapse", "Expand"].contains($0) ? $0 : nil }
            assertEachOnce(offered(sidebar, index), item.actions, fold: fold)
        }
    }

    func testABlockOnTheDaySwipesAssignAndDeleteAndOffersEditAndCancelThisDayOnceAsActions() throws {
        let fields = BlockFields(
            title: "Deep work", start: "09:00", minutes: "60", kind: "work", acceptsTasks: true,
            countsCapacity: true, anchored: false, repeat: "every day", until: "", minMinutes: "",
            taskFilter: "", colour: "", notes: ""
        )
        _ = try core.lumenna.addBlock(block: try newBlock(fields: fields, date: nil))
        let day = shown(DayViewController(core: core))
        let list = list(in: day)
        let plan = try core.lumenna.plan(date: nil)
        let block = try XCTUnwrap(plan.blocks.first)
        let index = try XCTUnwrap((0..<list.numberOfItems(inSection: 0)).first { offered(day, $0).menu.contains("Delete Block") })
        let row = offered(day, index)
        XCTAssertEqual(row.trailing, ["Assign a Task", "Delete Block"])
        XCTAssertEqual(row.custom, ["Edit Block", "Cancel This Day"])
        assertEachOnce(row, block.actions)
        run("Cancel This Day", in: day.customActions(at: path(index)))
        XCTAssertEqual(try core.lumenna.plan(date: nil).cancelled.map(\.title), ["Deep work"], "the custom action runs the core's")
    }

    func testEveryRowOfTheDayOffersEachOfItsActionsOnce() throws {
        let fields = BlockFields(
            title: "Deep work", start: "09:00", minutes: "60", kind: "work", acceptsTasks: true,
            countsCapacity: true, anchored: false, repeat: "", until: "", minMinutes: "",
            taskFilter: "", colour: "", notes: ""
        )
        _ = try core.lumenna.addBlock(block: try newBlock(fields: fields, date: nil))
        let day = shown(DayViewController(core: core))
        let list = list(in: day)
        for index in 0..<list.numberOfItems(inSection: 0) {
            let row = offered(day, index)
            XCTAssertEqual(Set(row.listed).count, row.listed.count, "nothing listed twice: \(row)")
            XCTAssertEqual(Set(row.listed), Set(row.menu), "the menu and VoiceOver's list agree: \(row)")
        }
    }

    func testMarkingDoneFromTheKeyboardIsTheRowsOwnAction() throws {
        _ = try core.lumenna.addTask(text: "water the plants")
        let tasks = shown(TaskListViewController(core: core))
        let list = list(in: tasks)
        list.selectItem(at: path(0), animated: false, scrollPosition: [])
        XCTAssertTrue(tasks.canPerformAction(#selector(TaskListViewController.moveToTrash), withSender: nil))
        tasks.toggleDone()
        XCTAssertTrue(try core.lumenna.listTasks(query: "").rows.isEmpty, "completed, so no longer listed")
    }

    func testMoveUnderOffersWhatTheCoreOffersNotTheListAsFolded() throws {
        _ = try core.lumenna.addProject(name: "Work", parent: nil)
        _ = try core.lumenna.addProject(name: "Reports", parent: "Work")
        _ = try core.lumenna.addProject(name: "Home", parent: nil)
        let projects = shown(ProjectsViewController(core: core))
        let work = try XCTUnwrap(projects.items.first { $0.key == "Work" })
        // Folding Work away hides Reports from the list, and must not change what is offered.
        projects.toggleFold("Work")
        let move = try XCTUnwrap(work.actions.first { $0.kind == .moveUnder })
        XCTAssertEqual(try core.lumenna.choices(action: move).choices.map(\.title), ["Home"])
    }
}
