import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// The places over the app — the iPad's sidebar and Browse's lists — each row's swipe actions,
/// custom actions and menu asked for directly, as VoiceOver offers them. The store is seeded
/// through the device, and the app takes that in.
@MainActor
final class BrowseTests: XCTestCase {
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

    private func offered(_ list: AppListViewController, _ item: Int) -> Offered {
        Offered(
            trailing: titles(list.trailingSwipeActions(at: path(item))),
            custom: list.customActions(at: path(item)).map(\.name), menu: titles(list.menu(at: path(item)))
        )
    }

    /// `list` shown once the app has taken in what was seeded and its rows are drawn.
    private func shownList<List: AppListViewController>(_ list: List) -> List {
        core.app.tick()
        settle()
        let shown = shown(list)
        settle { list.items.count > 0 }
        return shown
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

    func testTheSidebarOffersNewProjectAndCollapseOnItsProjectsHeading() throws {
        _ = try core.app.device.addLabel(name: "calls")
        let sidebar = shownList(SidebarViewController(core: core) { _ in })
        settle { sidebar.items.contains { $0.title == "calls" } }
        let keys = sidebar.items.map(\.key)
        let projects = try XCTUnwrap(keys.firstIndex(of: LumennaApp.sidebarKey(kind: .group(.projects))))
        XCTAssertEqual(titles(sidebar.trailingSwipeActions(at: path(projects))), ["New Project", "Collapse"])
        let calls = try XCTUnwrap(sidebar.items.firstIndex { $0.title == "calls" })
        let label = offered(sidebar, calls)
        XCTAssertEqual(label.trailing, ["Rename", "Delete"], "a label's primary actions, the core's")
        XCTAssertEqual(label.custom, ["Merge Into", "Colour"], "the rest, as Browse offers them: alone, it moves neither up nor down")
        for (index, item) in sidebar.items.enumerated() {
            assertEachOnce(offered(sidebar, index), item.actions)
        }
    }

    func testMoveUnderOffersWhatTheCoreOffersNotTheListAsFolded() throws {
        _ = try core.app.device.addProject(name: "Work", parent: nil)
        _ = try core.app.device.addProject(name: "Reports", parent: "Work")
        _ = try core.app.device.addProject(name: "Home", parent: nil)
        let projects = shownList(PlacesViewController(core: core, group: .projects, title: "Projects"))
        settle { projects.items.contains { $0.title == "Reports" } }
        let index = try XCTUnwrap(projects.items.firstIndex { $0.title == "Work" })
        // Folding Work away hides Reports from the list, and must not change what is offered.
        let collapse = try XCTUnwrap(projects.items[index].actions.first { $0.kind == .collapse })
        projects.perform(collapse, on: projects.items[index])
        settle { !projects.items.contains { $0.title == "Reports" } }
        let work = try XCTUnwrap(projects.items.first { $0.title == "Work" })
        let move = try XCTUnwrap(work.actions.first { $0.kind == .moveUnder })
        projects.perform(move, on: work)
        settle { core.app.view.asking?.choices != nil }
        XCTAssertEqual(core.app.view.asking?.choices?.choices.map(\.title), ["Home"])
    }
}
