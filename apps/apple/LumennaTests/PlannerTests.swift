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

    func testReplanOffersOtherOptionsAndTakingOneShowsItsProposalInPlace() throws {
        try block("Deep work", at: "09:00", minutes: 120)
        try block("Lunch", kind: "break", at: "10:30", minutes: 30)
        try block("Review", at: "11:00", minutes: 60)
        let model = try ProposalModel(core: core, kind: .replan, now: "\(day)T10:40")
        XCTAssertEqual(model.words.others, "Other options")
        // Shortening Lunch below its minimum costs less than skipping it, so it comes first.
        XCTAssertTrue(model.alternatives.first?.sentence.hasPrefix("Let Lunch shorten") == true)
        let skip = try XCTUnwrap(model.alternatives.first { $0.sentence.hasPrefix("Skip Lunch today") })
        XCTAssertEqual(skip.sentence, "Skip Lunch today, your only break left today. Then nothing else needs to change.")
        XCTAssertEqual(model.alternatives.count, 3)
        model.choose(skip)
        XCTAssertNil(model.problem)
        XCTAssertEqual(model.announcement, "Lunch is skipped today.")
        XCTAssertEqual(model.items.count, 1, "the skip is the change to make")
        XCTAssertEqual(model.checked, [true])
        XCTAssertTrue(model.alternatives.isEmpty, "so no Other options heading")
    }

    func testPlanTheDayHasNoOtherOptions() throws {
        let model = try ProposalModel(core: core, kind: .planDay(day), now: "\(day)T08:00")
        XCTAssertTrue(model.words.others.isEmpty)
        XCTAssertTrue(model.alternatives.isEmpty)
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
        // The app takes in what the surface wrote, and the form fills in from the app's details.
        core.app.tick()
        let model = TaskDetailModel(core: core, id: id)
        let deadline = Date().addingTimeInterval(10)
        while model.task == nil, Date() < deadline {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
            if model.task == nil { model.show() }
        }
        XCTAssertFalse(model.task?.history.isEmpty ?? true, "the core says something of every task's past")
    }
}
