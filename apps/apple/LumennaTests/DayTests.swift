import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// The day over the app: each row's actions as the day offers them (its swipes, its other
/// accessibility actions, its menu: `DayScreen.actions`), and the day's own actions, asked for
/// and run directly. What VoiceOver lists of them is the VoiceOver UI tests'. The store is
/// seeded through the device, and the app takes that in.
@MainActor
final class DayTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        // A store of its own: Core takes its profile from here.
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    /// What a row offers, each way: its swipes, its other accessibility actions, its menu.
    private struct Offered {
        var trailing: [String]
        var custom: [String]
        var menu: [String]

        /// What VoiceOver, Switch Control and Full Keyboard Access list: the swipes, and the
        /// accessibility actions SwiftUI adds to them.
        var listed: [String] { trailing + custom }
    }

    /// Runs the main loop until the app is still and `until` holds, as a person waits for the
    /// screen to settle: the app answers on a thread of its own.
    private func settle(_ until: () -> Bool = { true }) {
        let deadline = Date().addingTimeInterval(10)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !(core.app.isStill && until()) && Date() < deadline
    }

    /// The day, opened as its screen opens it, once the app has taken in what was seeded.
    private func shownDay() -> DayModel {
        core.app.tick()
        settle()
        let day = DayModel(core: core)
        day.appear()
        settle { core.app.view.place == .today && day.shown.count == shownItems.count }
        return day
    }

    /// The app's day rows shown, not folded away, the heading apart.
    private var shownItems: [LumennaApp.DayItem] {
        (core.app.view.day?.items ?? []).filter { !$0.hidden && $0.kind != .summary }
    }

    private func split(_ day: DayModel, _ index: Int) -> AppRowActions {
        let item = day.shown[index].item
        return DayScreen.actions(of: item) { [weak day] action in day?.run(action, on: item) }
    }

    private func offered(_ day: DayModel, _ index: Int) -> Offered {
        let split = split(day, index)
        return Offered(trailing: split.trailing.map(\.title), custom: split.others.map(\.title), menu: split.actions.map(\.title))
    }

    /// Today's Deep work, never under way as the test runs: a block under way offers End Now and
    /// Add 15 Minutes too (at 09:22 the row's actions were those as well).
    private func block(repeating: String?) throws {
        let hour = Calendar.current.component(.hour, from: Date())
        _ = try core.app.device.addBlock(block: newBlock(
            title: "Deep work", at: (8..<11).contains(hour) ? "15:00" : "09:00", minutes: 60, date: nil, kind: "work", repeat: repeating
        ))
    }

    func testABlockOnTheDaySwipesAssignAndDeleteAndOffersEditCancelThisDayAndRemindersOnceAsActions() throws {
        try block(repeating: "every day")
        let day = shownDay()
        let index = try XCTUnwrap(day.shown.firstIndex { $0.item.kind == .block })
        let row = offered(day, index)
        XCTAssertEqual(row.trailing, ["Assign a Task", "Delete Block"])
        XCTAssertEqual(row.custom, ["Edit Block", "Cancel This Day", "Reminders"])
        XCTAssertEqual(row.listed.sorted(), shownItems[index].actions.map(\.title).sorted(), "each listed once")
        XCTAssertEqual(row.menu, shownItems[index].actions.map(\.title), "the menu has every action, in the app's order")
        let split = split(day, index)
        split.run(try XCTUnwrap(split.others.first { $0.title == "Cancel This Day" }))
        settle()
        XCTAssertEqual(try core.app.device.plan(date: nil).cancelled.map(\.title), ["Deep work"], "the accessibility action runs the app's")
    }

    func testEveryRowOfTheDayOffersEachOfItsActionsOnce() throws {
        try block(repeating: nil)
        let day = shownDay()
        for index in day.shown.indices {
            let row = offered(day, index)
            XCTAssertEqual(Set(row.listed).count, row.listed.count, "nothing listed twice: \(row)")
            XCTAssertEqual(Set(row.listed), Set(row.menu), "the menu and VoiceOver's list agree: \(row)")
        }
    }

    func testTheDaysOwnActionsAreOfferedInTheCoresOrder() throws {
        // A work block all of today, so the day is under way whatever the time: 1440 minutes,
        // since 1439 ends at 23:59 and is not under way in the day's last minute.
        _ = try core.app.device.addBlock(block: newBlock(
            title: "All day", at: "00:00", minutes: 1440, date: Clock.isoDay(.now), kind: "work", repeat: nil
        ))
        let day = shownDay()
        let actions = try XCTUnwrap(core.app.view.day).actions.map(\.title)
        XCTAssertTrue(actions.contains("Plan the Day"), "\(actions)")
        XCTAssertTrue(actions.contains("Replan from Now"), "\(actions)")
        XCTAssertEqual(day.actions.map(\.title), actions, "the heading's and the menu's, in the core's order")
    }
}
