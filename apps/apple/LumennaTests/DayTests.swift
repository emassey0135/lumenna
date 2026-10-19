import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// The day over the app: each row's swipe actions, custom actions and menu, and the day's own
/// actions, asked for and run directly, as VoiceOver offers them. The store is seeded through
/// the device, and the app takes that in.
@MainActor
final class DayTests: XCTestCase {
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

    /// The day shown, once the app has taken in what was seeded.
    private func shownDay() -> DayViewController {
        core.app.tick()
        settle()
        let day = shown(DayViewController(core: core))
        settle { core.app.view.place == .today && list(in: day).numberOfItems(inSection: 0) == shownItems.count }
        return day
    }

    /// The app's day rows shown, not folded away, the heading apart.
    private var shownItems: [LumennaApp.DayItem] {
        (core.app.view.day?.items ?? []).filter { !$0.hidden && $0.kind != .summary }
    }

    private func offered(_ day: DayViewController, _ item: Int) -> Offered {
        Offered(
            trailing: titles(day.trailingSwipeActions(at: path(item))),
            custom: day.customActions(at: path(item)).map(\.name), menu: titles(day.menu(at: path(item)))
        )
    }

    private func block(repeating: String?) throws {
        _ = try core.app.device.addBlock(block: newBlock(
            title: "Deep work", at: "09:00", minutes: 60, date: nil, kind: "work", repeat: repeating
        ))
    }

    func testABlockOnTheDaySwipesAssignAndDeleteAndOffersEditCancelThisDayAndRemindersOnceAsActions() throws {
        try block(repeating: "every day")
        let day = shownDay()
        let index = try XCTUnwrap(shownItems.firstIndex { $0.kind == .block })
        let row = offered(day, index)
        XCTAssertEqual(row.trailing, ["Assign a Task", "Delete Block"])
        XCTAssertEqual(row.custom, ["Edit Block", "Cancel This Day", "Reminders"])
        XCTAssertEqual(row.listed.sorted(), shownItems[index].actions.map(\.title).sorted(), "each listed once")
        XCTAssertEqual(row.menu, shownItems[index].actions.map(\.title), "the menu has every action, in the app's order")
        let cancel = try XCTUnwrap(day.customActions(at: path(index)).first { $0.name == "Cancel This Day" })
        _ = cancel.actionHandler?(cancel)
        settle()
        XCTAssertEqual(try core.app.device.plan(date: nil).cancelled.map(\.title), ["Deep work"], "the custom action runs the app's")
    }

    func testEveryRowOfTheDayOffersEachOfItsActionsOnce() throws {
        try block(repeating: nil)
        let day = shownDay()
        for index in shownItems.indices {
            let row = offered(day, index)
            XCTAssertEqual(Set(row.listed).count, row.listed.count, "nothing listed twice: \(row)")
            XCTAssertEqual(Set(row.listed), Set(row.menu), "the menu and VoiceOver's list agree: \(row)")
        }
    }

    func testTheDaysOwnActionsAreOfferedInTheBarAndOnTheHeadingInTheCoresOrder() throws {
        // A work block all of today, so the day is under way whatever the time: 1440 minutes,
        // since 1439 ends at 23:59 and is not under way in the day's last minute.
        _ = try core.app.device.addBlock(block: newBlock(
            title: "All day", at: "00:00", minutes: 1440, date: Clock.isoDay(.now), kind: "work", repeat: nil
        ))
        let screen = shownDay()
        let actions = try XCTUnwrap(core.app.view.day).actions.map(\.title)
        XCTAssertTrue(actions.contains("Plan the Day"), "\(actions)")
        XCTAssertTrue(actions.contains("Replan from Now"), "\(actions)")
        XCTAssertEqual(screen.dayActionTitles, actions)
        XCTAssertEqual(screen.headingActions.map(\.name), actions, "VoiceOver reaches them on the day's heading")
    }
}
