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

    /// Runs the action called `title`, as VoiceOver's action does.
    private func run(_ title: String, in configuration: UISwipeActionsConfiguration?) {
        guard let action = configuration?.actions.first(where: { $0.title == title }) else {
            return XCTFail("no action called \(title) among \(titles(configuration))")
        }
        action.handler(action, UIView()) { _ in }
    }

    private func path(_ item: Int) -> IndexPath { IndexPath(item: item, section: 0) }

    /// A task's actions after Mark Done, as the core gives them.
    private let taskActions = ["Edit Details", "Put in a Block", "Move to Project", "Make Subtask Of", "Wait For", "Move to Trash"]

    func testATaskOffersMarkDoneOneWayAndTheCoresOtherActionsTheOtherAndMarkDoneCompletesIt() throws {
        _ = try core.lumenna.addTask(text: "water the plants")
        let tasks = shown(TaskListViewController(core: core))
        XCTAssertEqual(titles(tasks.leadingSwipeActions(at: path(0))), ["Mark Done"])
        XCTAssertEqual(titles(tasks.trailingSwipeActions(at: path(0))), taskActions)
        run("Mark Done", in: tasks.leadingSwipeActions(at: path(0)))
        XCTAssertTrue(try core.lumenna.listTasks(query: "").rows.isEmpty, "completed, so no longer listed")
    }

    func testATaskWithSubtasksAlsoOffersCollapse() throws {
        _ = try core.lumenna.addTask(text: "essay")
        _ = try core.lumenna.addTask(text: "outline")
        let rows = try core.lumenna.listTasks(query: "").rows
        let essay = try XCTUnwrap(rows.first { $0.title == "essay" })
        let outline = try XCTUnwrap(rows.first { $0.title == "outline" })
        _ = try core.lumenna.moveTask(id: outline.id, to: .parent(id: essay.id))
        let tasks = shown(TaskListViewController(core: core))
        XCTAssertEqual(titles(tasks.trailingSwipeActions(at: path(0))), taskActions + ["Collapse"])
        run("Collapse", in: tasks.trailingSwipeActions(at: path(0)))
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
        XCTAssertEqual(list(in: tasks).numberOfItems(inSection: 0), 1, "the subtask is folded away")
        XCTAssertEqual(titles(tasks.trailingSwipeActions(at: path(0))), taskActions + ["Expand"])
    }

    func testATrashedTaskOffersRestoreAndDeleteFromTrash() throws {
        let added = try core.lumenna.addTask(text: "old idea")
        _ = try core.lumenna.trashTask(id: try XCTUnwrap(added.task?.id))
        let trash = shown(TaskListViewController(core: core, title: "Trash", query: "deleted", mode: .trash))
        XCTAssertNil(trash.leadingSwipeActions(at: path(0)), "nothing to mark done in the trash")
        XCTAssertEqual(titles(trash.trailingSwipeActions(at: path(0))), ["Restore", "Delete from Trash"])
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
        XCTAssertEqual(
            titles(sidebar.trailingSwipeActions(at: path(calls))),
            ["Rename", "Merge Into", "Colour", "Delete"],
            "a label's actions, the core's, as Browse offers them: alone, it moves neither up nor down"
        )
    }

    func testABlockOnTheDayOffersAssignEditAndDelete() throws {
        let fields = BlockFields(
            title: "Deep work", start: "09:00", minutes: "60", kind: "work", acceptsTasks: true,
            countsCapacity: true, anchored: false, repeat: "", until: "", minMinutes: "",
            taskFilter: "", colour: "", notes: ""
        )
        _ = try core.lumenna.addBlock(block: try newBlock(fields: fields, date: nil))
        let day = shown(DayViewController(core: core))
        let list = list(in: day)
        let offered = (0..<list.numberOfItems(inSection: 0)).map { titles(day.trailingSwipeActions(at: path($0))) }
        XCTAssertTrue(
            offered.contains { $0.starts(with: ["Assign a Task", "Edit Block"]) && $0.contains("Delete Block") },
            "\(offered)"
        )
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
