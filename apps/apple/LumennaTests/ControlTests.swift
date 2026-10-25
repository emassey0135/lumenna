import Foundation
import LumennaApp
import XCTest
@testable import Lumenna

/// The controls' model (`Widgets/ControlModel.swift`, compiled into these tests): every control
/// the core offers is one here, named in the core's words, the timer's toggle drawn from the
/// core's `TimerControl` alone, and the watch's New Task complication the core's launch shortcut.
@MainActor
final class ControlTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    func testEveryControlTheCoreOffersIsOneHereInItsKind() {
        let offered = LumennaApp.controls()
        XCTAssertEqual(offered.map(\.id), [Controls.timer, Controls.newTask], "a control the core adds needs one here")
        XCTAssertEqual(Controls.control(Controls.timer)?.kind, .toggle)
        XCTAssertEqual(Controls.control(Controls.newTask)?.kind, .button)
    }

    func testTheControlsAreNamedInTheCoresWords() throws {
        let timer = try XCTUnwrap(Controls.control(Controls.timer))
        XCTAssertEqual(Controls.name(Controls.timer), timer.name)
        XCTAssertFalse(timer.description.isEmpty)
        let newTask = try XCTUnwrap(Controls.control(Controls.newTask))
        let shortcut = try XCTUnwrap(LumennaApp.launchShortcuts().first { $0.id == Controls.newTask })
        XCTAssertEqual(newTask.name, shortcut.title)
        // App Intents' titles are literals, read at build time: kept the core's by this test.
        XCTAssertEqual(String(localized: NewTaskControlIntent.title), newTask.name)
        XCTAssertEqual(NewTaskControlIntent.launch, newTask.id)
    }

    func testARunningTimerIsOnAndSaysItsTaskAndThatAPressStopsIt() {
        let control = LumennaApp.TimerControl(
            running: true, detail: "Essay", action: "Stop Timer", actionSentence: "Stop timer", enabled: true
        )
        let state = TimerControlState(control)
        XCTAssertTrue(state.isOn)
        XCTAssertEqual(state.title, Controls.name(Controls.timer))
        XCTAssertEqual(state.value, "Essay")
        XCTAssertEqual(state.action, "Stop Timer")
        XCTAssertTrue(state.enabled)
        XCTAssertEqual(state.spoken, "\(state.title), Stop Timer, Essay")
    }

    func testAStoppedTimerIsOffAndSaysWhatAPressWouldStart() {
        let control = LumennaApp.TimerControl(
            running: false, detail: "Pay the rent", action: "Start Timer", actionSentence: "Start timer", enabled: true
        )
        let state = TimerControlState(control)
        XCTAssertFalse(state.isOn)
        XCTAssertEqual(state.value, "Pay the rent")
        XCTAssertEqual(state.spoken, "\(state.title), Start Timer, Pay the rent")
    }

    func testWithNothingToStartTheTimerIsDisabledAndSaysTheCoresReason() throws {
        let control = try core.app.device.timerControl()
        let state = TimerControlState(control)
        XCTAssertFalse(state.isOn)
        XCTAssertFalse(state.enabled)
        XCTAssertFalse(state.value.isEmpty, "the core's reason nothing would start")
        XCTAssertEqual(state.value, control.detail)
        XCTAssertTrue(state.spoken.hasSuffix(control.detail))
    }

    func testAControlThatCannotReachItsStoreSaysSoInTheCoresWordsAndDoesNothing() {
        let state = TimerControlState(Controls.unknown())
        XCTAssertFalse(state.isOn)
        XCTAssertFalse(state.enabled)
        XCTAssertEqual(state.value, LumennaApp.storeUnopened(reason: ""))
    }

    func testTheTimersControlFileReadsBackAsTheControlWrittenAndTellsOnlyOfAChange() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let device = core.app.device
        let file = TimerControlFile(directory: directory) { try device.timerControl() }
        XCTAssertTrue(try file.writeNow())
        XCTAssertFalse(try file.writeNow(), "an unchanged control tells no control to read again")
        let written = try String(contentsOf: file.url, encoding: .utf8)
        XCTAssertEqual(try LumennaApp.timerControlFromFile(contents: written), try device.timerControl())
    }

    func testTheRequestsTheMacsExtensionMayHandOnAreNamedOneByOne() {
        XCTAssertEqual(WidgetRequests.allowed, ["complete_task": ["id"], "toggle_timer": []])
    }

    func testTheTimersPressHandedToTheAppRunsWithNothingWhateverTheFileNames() throws {
        let container = FileManager.default.temporaryDirectory.appendingPathComponent("controls-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: container) }
        try WidgetRequests.hand(.init(operation: WidgetRequests.toggleTimer, arguments: ["id": "a", "confirm": "true"]), into: container)
        var ran: [(String, [LumennaApp.Argument])] = []
        let requests = WidgetRequests(container: container)
        XCTAssertTrue(requests.take { ran.append(($0, $1)) })
        XCTAssertEqual(ran.map(\.0), ["toggle_timer"])
        XCTAssertEqual(ran.first?.1.count, 0)
        XCTAssertFalse(requests.take { _, _ in XCTFail("a press runs once") })
    }

    func testALaunchAskedForBeforeAnythingShowsTheAppWaitsForIt() {
        let launches = Launches()
        var done: [String] = []
        launches.ask("new-task")
        XCTAssertEqual(done, [])
        launches.handle { done.append($0) }
        XCTAssertEqual(done, ["new-task"])
        launches.ask("today")
        XCTAssertEqual(done, ["new-task", "today"])
    }

    func testTheNewTaskComplicationIsTheCoresLaunchShortcutOpenedByItsURL() throws {
        let shortcut = try XCTUnwrap(LaunchComplication.shortcut, "the core's launch shortcuts lost New Task")
        XCTAssertEqual(shortcut.id, "new-task")
        XCTAssertEqual(LaunchComplication.name, shortcut.title)
        XCTAssertEqual(LaunchComplication.description, shortcut.description)
        XCTAssertEqual(shortcut.icon, "add", "drawn as a plus")
        let url = LaunchURL.of(LaunchComplication.launch)
        XCTAssertEqual(url.absoluteString, "lumenna://launch/new-task")
        XCTAssertEqual(LaunchURL.id(in: url), "new-task")
    }

    func testTheNewTaskComplicationIsAKindOfItsOwnAndNoGlance() {
        XCTAssertFalse(LumennaApp.glanceWidgets().map(\.id).contains(LaunchComplication.kind))
        XCTAssertFalse(LumennaApp.controls().map(\.id).contains(LaunchComplication.kind))
        XCTAssertNil(GlanceWidgets.families[LaunchComplication.kind])
    }
}
