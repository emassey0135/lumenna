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
        XCTAssertEqual(offered.map(\.id), [Controls.timer, Controls.newTask, Controls.energyToday], "a control the core adds needs one here")
        XCTAssertEqual(Controls.control(Controls.timer)?.kind, .toggle)
        XCTAssertEqual(Controls.control(Controls.newTask)?.kind, .button)
        XCTAssertEqual(Controls.control(Controls.energyToday)?.kind, .button)
    }

    func testEnergyTodaysControlIsItsLaunchShortcutInItsWords() throws {
        let energy = try XCTUnwrap(Controls.control(Controls.energyToday))
        let shortcut = try XCTUnwrap(LumennaApp.launchShortcuts().first { $0.id == Controls.energyToday })
        XCTAssertEqual(energy.name, shortcut.title)
        // App Intents' titles are literals, read at build time: kept the core's by this test.
        XCTAssertEqual(String(localized: EnergyTodayControlIntent.title), energy.name)
        XCTAssertEqual(EnergyTodayControlIntent.launch, energy.id)
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
            running: true, detail: "Essay", action: "Stop Timer", actionSentence: "Stop timer", enabled: true, timer: nil
        )
        let state = TimerControlState(control)
        XCTAssertTrue(state.isOn)
        XCTAssertEqual(state.title, Controls.name(Controls.timer))
        XCTAssertEqual(state.value, "Essay")
        XCTAssertEqual(state.action, "Stop Timer")
        XCTAssertTrue(state.enabled)
        XCTAssertEqual(state.spoken, "\(state.title), Stop Timer, Essay")
    }

    /// A timer running on a sitting of 25 minutes, from 09:00.
    private func running(planned: UInt32? = 25, runsOut: String? = "2026-10-25T09:25:00Z") -> LumennaApp.TimerControl {
        LumennaApp.TimerControl(
            running: true, detail: "Essay", action: "Stop Timer", actionSentence: "Stop timer", enabled: true,
            timer: LumennaApp.GlanceTimer(
                sitting: "s", task: "Essay", block: "Deep work", countingFrom: "2026-10-25T09:00:00Z",
                plannedMins: planned, runsOutAt: runsOut, emoji: nil
            )
        )
    }

    private func at(_ instant: String) throws -> Date { try XCTUnwrap(Instant.parse(instant)) }

    func testARunningTimersControlSaysWhatIsLeftOfItsPlannedLengthElseHowLongSoFar() throws {
        let state = TimerControlState(running(), at: try at("2026-10-25T09:12:30Z"))
        XCTAssertEqual(state.value, "Essay, 13 minutes left", "the core's words beside the task")
        XCTAssertEqual(state.spoken, "\(state.title), Stop Timer, Essay, 13 minutes left")
        XCTAssertEqual(TimerControlState(running(), at: try at("2026-10-25T09:28:00Z")).value, "Essay, 3 minutes over")
        XCTAssertEqual(TimerControlState(running(planned: nil, runsOut: nil), at: try at("2026-10-25T09:12:30Z")).value, "Essay, 12 minutes")
    }

    func testTheMenuBarsTimerItemIsTheCoresLineCountingDown() {
        let line = LumennaApp.timerControlLine(control: running(), now: "2026-10-25T09:12:30Z", sentence: false)
        XCTAssertEqual(line, "Stop Timer, Essay, 13 minutes left")
    }

    // MARK: - The Live Activity

    func testTheLiveActivitysStateIsTheControlsTimerWithTheCoresButtons() throws {
        let state = try XCTUnwrap(TimerActivityState.of(running(), at: try at("2026-10-25T09:12:30Z")))
        XCTAssertEqual(state.sitting, "s", "what its Pause and Stop act on")
        XCTAssertEqual(state.task, "Essay")
        XCTAssertEqual(state.block, "Deep work")
        XCTAssertEqual(state.countingFrom, try at("2026-10-25T09:00:00Z"))
        XCTAssertEqual(state.runsOutAt, try at("2026-10-25T09:25:00Z"))
        XCTAssertEqual(state.plannedMins, 25)
        XCTAssertEqual(state.upWords, "time is up", "the core's words for the moment it runs out")
        XCTAssertEqual(state.pause, Wording.pauseTimer)
        XCTAssertEqual(state.stop, Wording.stopTimer)
        // App Intents' titles are literals, read at build time: kept the core's by this test.
        XCTAssertEqual(String(localized: PauseTimerActivityIntent.title), Wording.pauseTimer)
        XCTAssertEqual(String(localized: StopTimerActivityIntent.title), Wording.stopTimer)
    }

    func testTheLiveActivityCountsDownToItsPlannedLengthThenUpAndShowsNothingWhileNoTimerRuns() throws {
        let state = try XCTUnwrap(TimerActivityState.of(running()))
        XCTAssertEqual(TimerActivityState.countsDown(state, at: try at("2026-10-25T09:12:30Z"))?.upperBound, try at("2026-10-25T09:25:00Z"))
        XCTAssertNil(TimerActivityState.countsDown(state, at: try at("2026-10-25T09:26:00Z")), "past it, it counts up")
        let unplanned = try XCTUnwrap(TimerActivityState.of(running(planned: nil, runsOut: nil)))
        XCTAssertNil(TimerActivityState.countsDown(unplanned, at: try at("2026-10-25T09:12:30Z")), "with no planned length, only up")
        XCTAssertNil(unplanned.upWords)
        let stopped = LumennaApp.TimerControl(
            running: false, detail: "Pay the rent", action: "Start Timer", actionSentence: "Start timer", enabled: true, timer: nil
        )
        XCTAssertNil(TimerActivityState.of(stopped), "no timer runs, so no Live Activity")
        XCTAssertNil(TimerActivityState.of(try core.app.device.timerControl()))
    }

    func testAStoppedTimerIsOffAndSaysWhatAPressWouldStart() {
        let control = LumennaApp.TimerControl(
            running: false, detail: "Pay the rent", action: "Start Timer", actionSentence: "Start timer", enabled: true, timer: nil
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
