import Foundation
import LumennaApp
import WidgetKit
import XCTest
@testable import Lumenna

/// The widgets' model (`Widgets/GlanceWidgets.swift`, compiled into these tests): every widget
/// the core offers is a WidgetKit widget in its sizes, each moment of the glance an entry dated
/// by its `at`, a running timer counted from the core's instant, the Smart Stack told what is
/// under way, Today's buttons named by the core's Mark Done and the task, and a smaller size
/// showing the first of a widget's parts.
@MainActor
final class WidgetTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    func testEveryWidgetTheCoreOffersIsAWidgetInTheSizesItComesIn() {
        let offered = LumennaApp.glanceWidgets().map(\.id)
        XCTAssertEqual(Set(GlanceWidgets.families.keys), Set(offered))
        for id in offered {
            XCTAssertFalse(GlanceWidgets.families[id]?.isEmpty ?? true, "\(id) has no WidgetKit widget")
            XCTAssertNotNil(GlanceWidgets.widget(id))
        }
    }

    func testNowIsTheSmallWidgetAndTheLockScreensAndTodayTheLargerOnes() {
        XCTAssertEqual(GlanceWidgets.families["now"], [.systemSmall, .accessoryCircular, .accessoryRectangular, .accessoryInline])
        XCTAssertEqual(GlanceWidgets.families["today"], [.systemMedium, .systemLarge])
    }

    func testASmallerSizeShowsTheFirstOfAWidgetsPartsThatFit() throws {
        let today = try XCTUnwrap(GlanceWidgets.widget("today"))
        let fitting = GlanceWidgets.fitting(today.parts)
        XCTAssertEqual(fitting.first, today.parts)
        XCTAssertEqual(fitting.last, [.headline])
        for (index, shown) in fitting.enumerated() {
            XCTAssertEqual(shown, Array(today.parts.prefix(today.parts.count - index)))
        }
        XCTAssertEqual(GlanceWidgets.fitting(try XCTUnwrap(GlanceWidgets.widget("now")).parts), [[.headline]])
    }

    func testEachMomentIsAnEntryDatedByItsAtInOrder() throws {
        _ = try core.app.device.addTask(text: "Pay the rent today")
        let glance = try core.app.glance(hours: GlanceFile.hours)
        let entries = GlanceWidgets.entries(glance)
        XCTAssertEqual(entries.count, glance.count)
        XCTAssertEqual(entries.map(\.glance), glance)
        XCTAssertEqual(entries.map(\.date), entries.map(\.date).sorted())
        let first = try XCTUnwrap(entries.first)
        XCTAssertEqual(first.date.timeIntervalSinceNow, 0, accuracy: 60)
        XCTAssertEqual(first.glance.tasks.first?.title, "Pay the rent")
    }

    func testTodaysButtonIsTheCoresMarkDoneNamedWithTheTask() throws {
        _ = try core.app.device.addTask(text: "Pay the rent today")
        let entry = try XCTUnwrap(core.app.glance(hours: 1).first)
        let line = try XCTUnwrap(entry.tasks.first)
        XCTAssertEqual(entry.markDone, "Mark Done")
        XCTAssertEqual(GlanceWidgets.markDone(entry, line), "Mark Done, Pay the rent")
    }

    func testAnInstantReadsToTheNanosecondOrTheSecond() throws {
        let whole = try XCTUnwrap(GlanceWidgets.instant("2026-10-25T09:30:00Z"))
        XCTAssertEqual(whole.timeIntervalSince1970, 1_792_920_600)
        let fine = try XCTUnwrap(GlanceWidgets.instant("2026-10-25T09:30:00.123456789Z"))
        XCTAssertEqual(fine.timeIntervalSince(whole), 0.123456789, accuracy: 0.000_001)
        XCTAssertNil(GlanceWidgets.instant("soon"))
    }

    func testATimerRunningCountsFromTheCoresInstantAndIsWorthShowing() throws {
        var entry = LumennaApp.glanceEmpty(locale: Writing())
        let quiet = GlanceTimelineEntry(date: Date(), glance: entry)
        XCTAssertNil(quiet.timerStart)
        XCTAssertEqual(quiet.relevance?.score, 0)
        entry.moment.timer = LumennaApp.GlanceTimer(sitting: "s", task: "Essay", block: "Deep work", countingFrom: "2026-10-25T09:00:00Z")
        let timing = GlanceTimelineEntry(date: Date(), glance: entry)
        XCTAssertEqual(timing.timerStart, GlanceWidgets.instant("2026-10-25T09:00:00Z"))
        XCTAssertEqual(timing.relevance?.score, 1)
    }

    func testABlockUnderWayIsWorthShowingAndFillsFromItsStartToItsEnd() throws {
        var entry = LumennaApp.glanceEmpty(locale: Writing())
        entry.moment.current = LumennaApp.GlanceBlock(
            id: "b@2026-10-25", title: "Deep work", start: "09:00", end: "11:00",
            startsAt: "2026-10-25T09:00:00Z", endsAt: "2026-10-25T11:00:00Z", kind: "work", colour: nil, tasks: []
        )
        let block = GlanceTimelineEntry(date: Date(), glance: entry)
        XCTAssertEqual(block.relevance?.score, 1)
        XCTAssertEqual(block.blockUnderWay?.lowerBound, GlanceWidgets.instant("2026-10-25T09:00:00Z"))
        XCTAssertEqual(block.blockUnderWay?.upperBound, GlanceWidgets.instant("2026-10-25T11:00:00Z"))
    }

    func testWithNoGlanceYetAWidgetSaysTheCoresEmptyDay() {
        let empty = LumennaApp.glanceEmpty(locale: Writing())
        XCTAssertEqual(empty.spoken, "Nothing more planned today. Nothing due.")
        XCTAssertEqual(empty.moment.date, Clock.isoDay(Date()))
    }

    func testAWidgetOpensTheAppOnTheCoresLaunch() throws {
        let today = try XCTUnwrap(LumennaApp.launchShortcuts().first { $0.id == "today" })
        let url = LaunchURL.of(today.id)
        XCTAssertEqual(url.absoluteString, "lumenna://launch/today")
        XCTAssertEqual(LaunchURL.id(in: url), "today")
        XCTAssertNil(LaunchURL.id(in: URL(string: "lumenna://tasks/overdue")!))
    }
}
