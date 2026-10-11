import UIKit
import UserNotifications
import XCTest
@testable import Lumenna

/// What the reminders hand the notification center, given a store: the system's own center
/// cannot be relied on to fire in a test, so a stand-in records what it was given. Every
/// block is placed relative to now, so no test depends on the hour it runs.
@MainActor
final class ReminderScheduleTests: XCTestCase {
    private var core: Core!
    private var centre: Centre!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
        centre = Centre()
    }

    /// Records what it is handed, holding what it was told is pending.
    final class Centre: NotificationCentre {
        var pending: [String] = []
        var scheduled: [UNNotificationRequest] = []
        var cancelled: [String] = []
        var categories: Set<UNNotificationCategory> = []
        var status: UNAuthorizationStatus = .authorized

        func pendingIdentifiers(_ done: @escaping ([String]) -> Void) { done(pending) }
        func schedule(_ request: UNNotificationRequest) { scheduled.append(request) }
        func cancel(_ identifiers: [String]) { cancelled += identifiers }
        func useCategories(_ categories: Set<UNNotificationCategory>) { self.categories = categories }
        func authorization(_ done: @escaping (UNAuthorizationStatus) -> Void) { done(status) }
        func ask(_ done: @escaping (Bool) -> Void) { done(true) }
    }

    /// A one-off block starting `minutes` from now, on whatever day that is; its series.
    @discardableResult
    private func block(_ title: String, inMinutes minutes: Int) throws -> (series: String, start: Date) {
        let start = Date().addingTimeInterval(TimeInterval(minutes * 60))
        let parts = Calendar.current.dateComponents([.hour, .minute], from: start)
        let fields = BlockFields(
            title: title, start: String(format: "%02d:%02d", parts.hour ?? 0, parts.minute ?? 0), minutes: "30", kind: "work",
            acceptsTasks: true, countsCapacity: true, anchored: false, repeat: "", until: "", minMinutes: "",
            taskFilter: "", colour: "", notes: ""
        )
        _ = try core.lumenna.addBlock(block: try newBlock(fields: fields, date: Clock.isoDay(start)))
        let series = try XCTUnwrap(core.lumenna.plan(date: Clock.isoDay(start)).blocks.first { $0.title == title }?.series)
        return (series, start)
    }

    /// Re-arms and waits for it to finish.
    private func rearm() {
        let schedule = ReminderSchedule(lumenna: core.lumenna, centre: centre)
        let done = expectation(description: "re-armed")
        schedule.rearm { done.fulfill() }
        wait(for: [done], timeout: 5)
    }

    private func moment(_ request: UNNotificationRequest) -> Date? {
        (request.trigger as? UNCalendarNotificationTrigger)?.nextTriggerDate()
    }

    func testABlockIsHandedOverAtItsDefaultsMomentWithTheCoresWordsAndButtons() throws {
        let (_, start) = try block("Deep work", inMinutes: 60)
        rearm()
        XCTAssertEqual(centre.scheduled.count, 1, "the default, five minutes before it starts")
        let request = try XCTUnwrap(centre.scheduled.first)
        XCTAssertEqual(request.content.title, "Deep work")
        XCTAssertEqual(request.content.body, "Deep work starts in 5 minutes — nothing assigned yet")
        XCTAssertEqual(request.content.categoryIdentifier, Reminders.category)
        XCTAssertTrue(request.identifier.hasPrefix(Reminders.prefix))
        let fires = try XCTUnwrap(moment(request))
        let expected = Calendar.current.date(bySetting: .second, value: 0, of: start.addingTimeInterval(-5 * 60)) ?? start
        XCTAssertEqual(fires.timeIntervalSince1970, expected.timeIntervalSince1970, accuracy: 60, "at the core's moment")
        let category = try XCTUnwrap(centre.categories.first)
        XCTAssertEqual(category.actions.map(\.title), ["Dismiss", "Snooze 10 Minutes"], "the core's words")
    }

    func testReArmingTwiceGivesTheSameIdentifierSoTheSystemReplacesRatherThanDuplicates() throws {
        try block("Deep work", inMinutes: 60)
        rearm()
        let first = centre.scheduled.map(\.identifier)
        centre.pending = first
        centre.scheduled = []
        rearm()
        XCTAssertEqual(centre.scheduled.map(\.identifier), first)
        XCTAssertTrue(centre.cancelled.isEmpty, "nothing still firing is cancelled")
    }

    func testWhatNoLongerFiresIsCancelledAndAnotherAppsNotificationsAreLeftAlone() throws {
        try block("Deep work", inMinutes: 60)
        centre.pending = ["\(Reminders.prefix)gone 2026-01-01", "something else"]
        rearm()
        XCTAssertEqual(centre.cancelled, ["\(Reminders.prefix)gone 2026-01-01"])
    }

    func testRemindersTurnedOffOnThisDeviceHandOverNothingAndCancelWhatWasPending() throws {
        try block("Deep work", inMinutes: 60)
        rearm()
        centre.pending = centre.scheduled.map(\.identifier)
        centre.scheduled = []
        _ = try core.lumenna.setSetting(key: "reminders", value: "false")
        rearm()
        XCTAssertTrue(centre.scheduled.isEmpty)
        XCTAssertEqual(centre.cancelled, centre.pending)
    }

    func testOnlyTheEarliestSixtyStillToComeAreScheduled() {
        let now = Date()
        let due = (-5..<70).map { minutes in
            DueReminder(
                key: "key\(minutes)", occurrence: "2026-10-10", at: Reminders.rfc3339(now.addingTimeInterval(TimeInterval(minutes * 60 + 30))),
                target: "t", kind: "block", title: "T", text: "T starts", missed: "T started", dismiss: "Dismiss", snooze: "Snooze"
            )
        }.reversed()
        let requests = Reminders.requests(Array(due), after: now)
        XCTAssertEqual(requests.count, Reminders.most)
        XCTAssertEqual(requests.first?.identifier, "\(Reminders.prefix)key0 2026-10-10", "the earliest still to come; past ones have fired")
    }

    func testDismissFromTheNotificationIsTheCoresAndThatFiringIsNoLongerScheduled() throws {
        try block("Deep work", inMinutes: 60)
        rearm()
        let request = try XCTUnwrap(centre.scheduled.first)
        ReminderResponder(lumenna: core.lumenna) { _ in }.answer(Reminders.dismissAction, info: request.content.userInfo)
        centre.scheduled = []
        rearm()
        XCTAssertTrue(centre.scheduled.isEmpty, "dismissed, on every device")
    }

    func testSnoozeFromTheNotificationSchedulesItAgainTenMinutesFromNow() throws {
        try block("Deep work", inMinutes: 60)
        rearm()
        let request = try XCTUnwrap(centre.scheduled.first)
        ReminderResponder(lumenna: core.lumenna) { _ in }.answer(Reminders.snoozeAction, info: request.content.userInfo)
        centre.scheduled = []
        rearm()
        let again = try XCTUnwrap(centre.scheduled.first)
        XCTAssertEqual(again.identifier, request.identifier, "the same firing, replaced")
        let fires = try XCTUnwrap(moment(again))
        XCTAssertEqual(fires.timeIntervalSinceNow, 10 * 60, accuracy: 60)
    }

    func testChoosingTheNotificationOpensWhatItIsAbout() throws {
        let (series, start) = try block("Deep work", inMinutes: 60)
        rearm()
        let request = try XCTUnwrap(centre.scheduled.first)
        var opened: RemindedOf?
        ReminderResponder(lumenna: core.lumenna) { opened = $0 }.answer(UNNotificationDefaultActionIdentifier, info: request.content.userInfo)
        XCTAssertEqual(opened?.kind, "block")
        XCTAssertEqual(opened?.target, series)
        XCTAssertEqual(opened?.day, Clock.isoDay(start), "the occurrence's day")
    }

    func testAProcessThatRunsShowsWhatCameDueOnceAndThenNothingMore() throws {
        try block("Deep work", inMinutes: 60)
        let fire = try XCTUnwrap(Reminders.moment(core.lumenna.upcomingReminders(until: Reminders.rfc3339(Date().addingTimeInterval(3 * 3600))).due[0].at))
        let delivery = ReminderDelivery(lumenna: core.lumenna, centre: centre)
        XCTAssertTrue(delivery.deliver(now: Reminders.rfc3339(fire.addingTimeInterval(-120))).isEmpty, "nothing owed from before it ran")
        let shown = delivery.deliver(now: Reminders.rfc3339(fire.addingTimeInterval(60)))
        XCTAssertEqual(shown.map(\.content.title), ["Deep work"])
        XCTAssertNil(shown.first?.trigger, "shown at once")
        XCTAssertTrue(delivery.deliver(now: Reminders.rfc3339(fire.addingTimeInterval(120))).isEmpty, "given once")
    }

    /// Sets this device's own settings as an earlier run left them, in the file the core
    /// keeps them in: what moments in the past cannot be made any other way.
    private func deviceSettings(_ values: [String: String]) throws {
        let profile = try Core.profileDirectory()
        let file = profile.appendingPathComponent("device-settings")
        let kept = ((try? String(contentsOf: file, encoding: .utf8)) ?? "")
            .split(separator: "\n").map(String.init)
            .filter { line in !values.keys.contains { line.hasPrefix("\($0)\t") } }
        let lines = kept + values.map { "\($0.key)\t\($0.value)" }
        try (lines.joined(separator: "\n") + "\n").write(to: file, atomically: true, encoding: .utf8)
    }

    func testAFiringPastTheArmedWindowIsCaughtUpAndShownOnce() throws {
        // One fired three minutes ago, recent enough to say as it is; one half an hour ago,
        // said in the summary. Both after the window last handed over ended.
        try block("Stand-up", inMinutes: 2)
        try block("Deep work", inMinutes: -25)
        let schedule = ReminderSchedule(lumenna: core.lumenna, centre: centre)
        XCTAssertTrue(schedule.catchUp().isEmpty, "the first time, nothing is owed")
        let hoursAgo = { (hours: Double) in Reminders.rfc3339(Date().addingTimeInterval(-hours * 3600)) }
        // As if last armed two hours ago, for a window that ran out an hour ago.
        try deviceSettings(["reminders-caught-up": hoursAgo(2), "reminders-armed-until": hoursAgo(1)])
        let shown = schedule.catchUp()
        XCTAssertEqual(shown.count, 2, "\(shown.map(\.content.body))")
        let fresh = try XCTUnwrap(shown.first { $0.content.title == "Stand-up" })
        XCTAssertNil(fresh.trigger, "shown at once")
        XCTAssertEqual(fresh.content.categoryIdentifier, Reminders.category, "with Dismiss and Snooze")
        let summary = try XCTUnwrap(shown.first { $0.content.title.isEmpty })
        XCTAssertEqual(summary.content.body, "1 reminder missed: Deep work started.")
        XCTAssertTrue(schedule.catchUp().isEmpty, "shown once")
    }

    func testAFiringWithinTheArmedWindowIsTheSystemsToShowAndNotCaughtUp() throws {
        try block("Stand-up", inMinutes: 2)
        let schedule = ReminderSchedule(lumenna: core.lumenna, centre: centre)
        schedule.catchUp()
        let hoursAgo = { (hours: Double) in Reminders.rfc3339(Date().addingTimeInterval(-hours * 3600)) }
        try deviceSettings(["reminders-caught-up": hoursAgo(2), "reminders-armed-until": hoursAgo(-40)])
        XCTAssertTrue(schedule.catchUp().isEmpty, "the system kept what it was handed")
    }

    func testOneEditReArmsOnceAndReArmingCausesNoMore() throws {
        let schedule = ReminderSchedule(lumenna: core.lumenna, centre: centre)
        schedule.follow()
        try block("Deep work", inMinutes: 60)
        NotificationCenter.default.post(name: Core.changed, object: nil)
        // Long enough for a re-arm that led to another, and that to a third, to show.
        RunLoop.current.run(until: Date(timeIntervalSinceNow: 3))
        XCTAssertEqual(schedule.rearms, 1, "one edit, one re-arm, and nothing after")
        XCTAssertEqual(centre.scheduled.count, 1)
    }

    func testABlocksReminderListShowsTheDefaultThenItsOwnWhichSwipesRemove() throws {
        let (series, _) = try block("Deep work", inMinutes: 120)
        let list = RemindersViewController(core: core, target: series)
        list.loadViewIfNeeded()
        list.view.frame = CGRect(x: 0, y: 0, width: 400, height: 900)
        list.beginAppearanceTransition(true, animated: false)
        list.endAppearanceTransition()
        list.view.layoutIfNeeded()
        XCTAssertEqual(list.items.map(\.title), ["5 minutes before it starts"])
        XCTAssertNil(list.trailingSwipeActions(at: IndexPath(item: 0, section: 0)), "a default cannot be removed alone")
        let add = try XCTUnwrap(core.lumenna.remindersOf(target: series).actions.first)
        _ = try core.lumenna.act(action: add, answer: .text(text: "at end"))
        list.reload()
        XCTAssertEqual(list.items.map(\.title), ["when it ends"])
        let swipes = list.trailingSwipeActions(at: IndexPath(item: 0, section: 0))?.actions.compactMap(\.title)
        XCTAssertEqual(swipes, ["Remove Reminder"])
    }
}
