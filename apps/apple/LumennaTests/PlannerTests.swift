import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// The planner as the Apple apps show it: the proposal screens over the app, a choice's reason,
/// and a task's history.
@MainActor
final class PlannerTests: XCTestCase {
    private var core: Core!
    /// A day long past, and a moment on it, so nothing here depends on when it runs.
    private let day = "2026-05-06"

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    private func block(_ title: String, kind: String = "work", at: String, minutes: UInt32, date: String? = nil) throws {
        _ = try core.app.device.addBlock(block: newBlock(title: title, at: at, minutes: minutes, date: date ?? day, kind: kind))
    }

    /// Waits for the app to be still and `until` to hold, as a person waits for the screen.
    private func settle(until: () -> Bool = { true }) {
        let deadline = Date().addingTimeInterval(10)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !(core.app.isStill && until()) && Date() < deadline
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

    /// Replan from Now against a moment on `day`, as an overrun's button opens it.
    private func replan(at time: String) -> AppProposalModel {
        // What was seeded, taken in first.
        core.app.tick()
        settle()
        core.app.send(.planFrom("\(day)T\(time)"))
        core.app.send(.proposal(.replan))
        settle { core.app.view.proposal?.ready == true }
        return AppProposalModel(core: core)
    }

    // MARK: - Plan the Day

    func testPlanTheDayShowsEachSittingWithItsReasonAndTheNextPlanAfterAPartialApply() throws {
        // Tomorrow, so its blocks are still to come whenever the test runs.
        try block("Morning", at: "09:00", minutes: 30, date: "tomorrow")
        try block("Afternoon", at: "14:00", minutes: 30, date: "tomorrow")
        for task in ["Report p1", "Email p2", "Filing p3"] { _ = try core.app.device.addTask(text: task) }
        core.app.tick()
        settle()
        core.app.send(.open(.today))
        core.app.send(.day(.next))
        settle { core.app.view.day?.actions.contains { $0.kind == .planDay } == true }
        let plan = try XCTUnwrap(core.app.view.day?.actions.first { $0.kind == .planDay })
        core.app.send(.perform(plan))
        settle { core.app.view.proposal?.ready == true }
        let model = AppProposalModel(core: core)
        var closed = false
        model.close = { closed = true }
        XCTAssertEqual(model.title, "Plan the Day")
        XCTAssertEqual(model.proposal?.announcement, "Plans 2 tasks: Report in Morning and Email in Afternoon.")
        let changes = try XCTUnwrap(model.proposal?.changes)
        XCTAssertEqual(changes.count, 2)
        XCTAssertEqual(changes[0].reason, "Suggested because priority 1.")
        XCTAssertEqual(changes.map(\.checked), [true, true], "everything checked to begin with")

        // Report turned down, Email taken: the core plans again around it, and the screen
        // shows that plan rather than closing.
        model.binding(0).wrappedValue = false
        settle { model.proposal?.changes.first?.checked == false }
        model.apply()
        settle { model.proposal?.changes.count == 1 }
        XCTAssertFalse(closed, "a next plan is shown")
        let next = try XCTUnwrap(model.proposal?.changes)
        XCTAssertEqual(next.count, 1)
        XCTAssertTrue(next[0].sentence.hasPrefix("Filing"), next[0].sentence)
        XCTAssertEqual(next.map(\.checked), [true])

        model.apply()
        settle { closed }
        XCTAssertTrue(closed, "nothing more proposed, so the screen closes")
    }

    // MARK: - Replan from Now

    func testReplanSaysItsProposalInTheDevicesClockAndAStaleOneIsShownAndAskedAgain() throws {
        try block("Deep work", at: "09:00", minutes: 120)
        try block("Lunch", kind: "break", at: "10:30", minutes: 30)
        let model = replan(at: "10:40")
        XCTAssertEqual(model.title, "Replan from Now")
        let lunch = Clock.style == .twelveHour ? "Lunch moves from 10:30 AM to 11:00 AM." : "Lunch moves from 10:30 to 11:00."
        XCTAssertEqual(model.proposal?.announcement, lunch)
        XCTAssertEqual(model.proposal?.changes.count, 1)

        // Made elsewhere meanwhile: this proposal is stale, said on the screen, and the
        // screen asks again, which now proposes nothing.
        let now = "\(day)T10:40"
        let other = try core.app.device.replan(style: .twentyFourHour, pins: LumennaApp.ReplanPins(keep: [], skip: [], sittings: [], floors: []), now: now)
        _ = try core.app.device.applyReplan(proposal: other, accepted: other.changes, now: now)
        model.apply()
        settle { model.proposal?.refusal != nil }
        XCTAssertNotNil(model.proposal?.refusal)
        XCTAssertTrue(model.proposal?.changes.isEmpty ?? false, "asked again: \(String(describing: model.proposal?.changes))")
    }

    func testReplanOffersOtherOptionsAndTakingOneShowsItsProposalInPlace() throws {
        try block("Deep work", at: "09:00", minutes: 120)
        try block("Lunch", kind: "break", at: "10:30", minutes: 30)
        try block("Review", at: "11:00", minutes: 60)
        let model = replan(at: "10:40")
        XCTAssertEqual(model.proposal?.words.others, "Other options")
        let others = try XCTUnwrap(model.proposal?.others)
        // Shortening Lunch below its minimum costs less than skipping it, so it comes first.
        XCTAssertTrue(others.first?.hasPrefix("Let Lunch shorten") == true, "\(others)")
        let skip = try XCTUnwrap(others.firstIndex { $0.hasPrefix("Skip Lunch today") })
        XCTAssertEqual(others[skip], "Skip Lunch today, your only break left today. Then nothing else needs to change.")
        XCTAssertEqual(others.count, 3)
        model.take(skip)
        settle { model.proposal?.announcement == "Lunch is skipped today." }
        XCTAssertNil(model.proposal?.refusal)
        XCTAssertEqual(model.proposal?.announcement, "Lunch is skipped today.")
        XCTAssertEqual(model.proposal?.changes.map(\.checked), [true], "the skip is the change to make")
        XCTAssertTrue(model.proposal?.others.isEmpty ?? false, "so no Other options heading")
    }

    func testReplanWithNothingToChangeSaysSoAndOffersNoApply() throws {
        let model = replan(at: "10:40")
        XCTAssertTrue(model.proposal?.changes.isEmpty ?? false)
        XCTAssertFalse(model.proposal?.words.nothing.isEmpty ?? true)
    }

    // MARK: - A choice's reason

    func testAChoicesReasonIsItsDescriptionAndSecondLineNeverItsName() throws {
        let reason = "Suggested because priority 1."
        let item = AppItem(key: "a", title: "Report", detail: "Inbox", reason: reason)
        let picker = shown(ChoicePicker(core: core, title: "Assign a Task", items: [item], chosen: { _ in }, cancelled: {}))
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
        let id = try XCTUnwrap(core.app.device.addTask(text: "Report").task?.id)
        core.app.tick()
        let model = TaskDetailModel(core: core, id: id)
        settle { model.task != nil }
        XCTAssertFalse(model.task?.history.isEmpty ?? true, "the core says something of every task's past")
    }
}
