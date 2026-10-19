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

    private func offered(_ list: ItemListViewController, _ item: Int) -> Offered {
        Offered(
            trailing: titles(list.trailingSwipeActions(at: path(item))),
            custom: list.customActions(at: path(item)).map(\.name), menu: titles(list.menu(at: path(item)))
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
