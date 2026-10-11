import UIKit
import UserNotifications
import XCTest
@testable import Lumenna

/// The planner as the Apple apps show it: the day's own actions, the proposal screens, a
/// choice's reason, a task's history, and Carry Forward on a reminder.
@MainActor
final class PlannerTests: XCTestCase {
    private var core: Core!
    /// A day long past, and a moment on it, so nothing here depends on when it runs.
    private let day = "2026-05-06"

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    private func block(_ title: String, kind: String = "work", at: String, minutes: UInt32) throws {
        _ = try core.lumenna.addBlock(block: NewBlock(title: title, at: at, minutes: minutes, date: day, kind: kind, repeat: nil))
    }

    private func shown<Screen: UIViewController>(_ screen: Screen) -> Screen {
        screen.loadViewIfNeeded()
        screen.view.frame = CGRect(x: 0, y: 0, width: 400, height: 900)
        screen.beginAppearanceTransition(true, animated: false)
        screen.endAppearanceTransition()
        screen.view.layoutIfNeeded()
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.1))
        return screen
    }

    // MARK: - The day's own actions

    func testTheDaysOwnActionsAreOfferedInTheBarAndOnTheHeadingInTheCoresOrder() throws {
        // A work block all of today, so the day is under way whatever the time: 1440 minutes,
        // since 1439 ends at 23:59 and is not under way in the day's last minute.
        _ = try core.lumenna.addBlock(block: NewBlock(
            title: "All day", at: "00:00", minutes: 1440, date: Clock.isoDay(.now), kind: "work", repeat: nil
        ))
        let screen = shown(DayViewController(core: core))
        let actions = try core.lumenna.plan(date: nil).actions.map(\.title)
        XCTAssertTrue(actions.contains("Plan the Day"), "\(actions)")
        XCTAssertTrue(actions.contains("Replan from Now"), "\(actions)")
        XCTAssertEqual(screen.dayActionTitles, actions)
        XCTAssertEqual(screen.headingActions.map(\.name), actions, "VoiceOver reaches them on the day's heading")
    }

    // MARK: - Plan the Day

    func testPlanTheDayShowsEachSittingWithItsReasonAndTheNextPlanAfterAPartialApply() throws {
        try block("Morning", at: "09:00", minutes: 30)
        try block("Afternoon", at: "14:00", minutes: 30)
        for task in ["Report p1", "Email p2", "Filing p3"] { _ = try core.lumenna.addTask(text: task) }
        let model = try ProposalModel(core: core, kind: .planDay(day), now: "\(day)T08:00")
        XCTAssertEqual(model.words.title, "Plan the Day")
        XCTAssertEqual(model.announcement, "Plans 2 tasks: Report in Morning and Email in Afternoon.")
        XCTAssertEqual(model.items.count, 2)
        XCTAssertEqual(model.items[0].reason, "Suggested because priority 1.")
        XCTAssertEqual(model.checked, [true, true], "everything checked to begin with")
        var closed = false
        model.close = { closed = true }
        var applied: [Change] = []
        model.applied = { applied.append($0) }

        // Report turned down, Email taken: the core plans again around it, and the screen
        // shows that plan rather than closing.
        model.binding(0).wrappedValue = false
        model.apply()
        XCTAssertEqual(applied.count, 1)
        XCTAssertFalse(closed, "a next plan is shown")
        XCTAssertEqual(model.items.map(\.sentence).count, 1)
        XCTAssertTrue(model.items[0].sentence.hasPrefix("Filing"), model.items[0].sentence)
        XCTAssertEqual(model.checked, [true])

        model.apply()
        XCTAssertTrue(closed, "nothing more proposed, so the screen closes")
        XCTAssertEqual(applied.count, 2)
    }

    // MARK: - Replan from Now

    func testReplanSaysItsProposalInTheDevicesClockAndAStaleOneIsShownAndAskedAgain() throws {
        try block("Deep work", at: "09:00", minutes: 120)
        try block("Lunch", kind: "break", at: "10:30", minutes: 30)
        let now = "\(day)T10:40"
        let model = try ProposalModel(core: core, kind: .replan, now: now)
        XCTAssertEqual(model.words.title, "Replan from Now")
        let lunch = Clock.style == .twelveHour ? "Lunch moves from 10:30 AM to 11:00 AM." : "Lunch moves from 10:30 to 11:00."
        XCTAssertEqual(model.announcement, lunch)
        XCTAssertEqual(model.items.count, 1)

        // Made elsewhere meanwhile: this proposal is stale, said on the screen, and the
        // screen asks again, which now proposes nothing.
        let other = try core.lumenna.replan(style: .twentyFourHour, pins: ReplanPins(keep: [], skip: [], sittings: []), now: now)
        _ = try core.lumenna.applyReplan(proposal: other, accepted: other.changes, now: now)
        model.apply()
        XCTAssertNotNil(model.problem)
        XCTAssertTrue(model.items.isEmpty, "asked again: \(model.items)")
    }

    func testReplanWithNothingToChangeSaysSoAndOffersNoApply() throws {
        let model = try ProposalModel(core: core, kind: .replan, now: "\(day)T10:40")
        XCTAssertTrue(model.items.isEmpty)
        XCTAssertEqual(model.announcement, model.replan?.announcement)
        XCTAssertFalse(model.words.nothing.isEmpty)
    }

    // MARK: - A choice's reason

    func testAChoicesReasonIsItsDescriptionAndSecondLineNeverItsName() throws {
        let reason = "Suggested because priority 1."
        let choice = Choice(id: "a", title: "Report", depth: 0, detail: "Inbox", date: nil, start: nil, end: nil, reason: reason)
        let picker = shown(ChoicePicker(core: core, title: "Assign a Task", choices: [choice]) { _ in })
        func find(_ view: UIView) -> UICollectionView? {
            (view as? UICollectionView) ?? view.subviews.lazy.compactMap(find).first
        }
        let list = try XCTUnwrap(find(picker.view))
        list.layoutIfNeeded()
        let cell = try XCTUnwrap(list.cellForItem(at: IndexPath(item: 0, section: 0)) as? UICollectionViewListCell)
        XCTAssertEqual(cell.accessibilityLabel, "Report")
        XCTAssertEqual(cell.accessibilityHint, reason)
        let content = try XCTUnwrap(cell.contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(content.secondaryText, "Inbox\n\(reason)")
    }

    // MARK: - A task's history

    func testTheTaskScreenHasTheTasksHistory() throws {
        let id = try XCTUnwrap(core.lumenna.addTask(text: "Report").task?.id)
        let model = TaskDetailModel(core: core, id: id)
        XCTAssertFalse(model.task?.history.isEmpty ?? true, "the core says something of every task's past")
    }

    // MARK: - Carry Forward on a reminder

    private func reminder(carry: String) -> DueReminder {
        DueReminder(
            key: "k", occurrence: day, at: "\(day)T10:00:00Z", target: "series", kind: "block", title: "Deep work",
            text: "Deep work ends now. Carry the rest forward?", missed: "Deep work ended", dismiss: "Dismiss",
            snooze: "Snooze", carry: carry
        )
    }

    func testAReminderOfferingCarryForwardHasTheButtonBesideDismissAndSnooze() throws {
        let carried = reminder(carry: "Carry Forward")
        XCTAssertEqual(Reminders.content(carried).categoryIdentifier, Reminders.carryCategory)
        XCTAssertEqual(Reminders.content(reminder(carry: "")).categoryIdentifier, Reminders.category)
        let categories = Reminders.categories(for: [reminder(carry: ""), carried])
        let carry = try XCTUnwrap(categories.first { $0.identifier == Reminders.carryCategory })
        XCTAssertEqual(carry.actions.map(\.title), ["Dismiss", "Snooze", "Carry Forward"])
        XCTAssertEqual(carry.actions.last?.identifier, Reminders.carryAction)
        let plain = try XCTUnwrap(categories.first { $0.identifier == Reminders.category })
        XCTAssertEqual(plain.actions.map(\.title), ["Dismiss", "Snooze"])
    }
}
