import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// What every Apple app's screens follow (`Shared/`), driven over a store of the test's own as a
/// screen drives it: the app's questions, the sidebar as a list says it, the block form, pairing,
/// an export's file, and the ways in from outside.
@MainActor
final class SharedScreensTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        // A store of its own: Core takes its profile from here.
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    /// Runs the main loop until the app is still and `until` holds, as a person waits for the
    /// screen to settle: the app answers on a thread of its own.
    private func settle(_ until: () -> Bool = { true }) {
        let deadline = Date().addingTimeInterval(10)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !(core.app.isStill && until()) && Date() < deadline
    }

    /// Asks as a platform would, keeping what it was asked: only a line of text is answered here.
    private final class Asker: QuestionAsking {
        var asked: [(title: String, initial: String, problem: String?, day: String?, answer: (String?) -> Void)] = []

        func text(title: String, label: String, initial: String, hint: String, problem: String?, yes: String, syntax: LumennaApp.Syntax?, day: String?, answered: @escaping (String?) -> Void) {
            asked.append((title, initial, problem, day, answered))
        }

        func confirm(title: String, message: String, yes: String, destructive: Bool, answered: @escaping (Bool) -> Void) { answered(false) }
        func pick(title: String, offered: [Lumenna.Offered], yes: String, answered: @escaping (Picked) -> Void) { answered(.cancelled) }
        func choose(title: String, message: String, answers: [(id: String, title: String)], destructive: Bool, answered: @escaping (String?) -> Void) { answered(nil) }
        func tell(title: String, _ sentence: String) {}
    }

    func testGoToDayIsAskedWithTheDayShownAndARefusedDayIsAskedAgainWithWhy() throws {
        let asker = Asker()
        let questions = AppQuestions(app: core.app, asker: asker) { _ in }
        core.app.send(.open(.today))
        core.app.send(.day(.ask))
        settle { !asker.asked.isEmpty }
        let first = try XCTUnwrap(asker.asked.first)
        XCTAssertEqual(first.day, Clock.isoDay(.now), "the calendar opens on the day shown, today")
        first.answer("someday soon")
        settle { asker.asked.count == 2 }
        let again = try XCTUnwrap(asker.asked.last)
        XCTAssertNotNil(again.problem, "why, in the core's words")
        XCTAssertEqual(again.initial, "someday soon", "with what was typed")
        let tomorrow = try XCTUnwrap(Calendar.current.date(byAdding: .day, value: 1, to: .now))
        again.answer(Clock.isoDay(tomorrow))
        settle { core.app.view.asking == nil }
        XCTAssertEqual(core.app.view.day?.date, Clock.isoDay(tomorrow))
        _ = questions
    }

    func testTheSidebarLeavesOutWhatIsFoldedAndSaysTheLevelWhereItChanges() throws {
        _ = try core.app.device.addProject(name: "Work", parent: nil)
        _ = try core.app.device.addProject(name: "Reports", parent: "Work")
        _ = try core.app.device.addProject(name: "Home", parent: nil)
        settle { core.app.sidebarItems(under: .projects).contains { $0.title == "Reports" } }
        let projects = core.app.sidebarItems(under: .projects).filter { ["Work", "Reports", "Home"].contains($0.title) }
        XCTAssertEqual(projects.map(\.title), ["Work", "Reports", "Home"])
        XCTAssertEqual(projects.map(\.level), [nil, "level 2", "level 1"], "said only where it changes")
        XCTAssertEqual(projects[0].expanded, true)
        let collapse = try XCTUnwrap(projects[0].actions.first { $0.kind == .collapse })
        core.app.run(collapse, on: projects[0].key, side: .sidebar)
        settle { !core.app.sidebarItems(under: .projects).contains { $0.title == "Reports" } }
        let folded = try XCTUnwrap(core.app.sidebarItems(under: .projects).first { $0.title == "Work" })
        XCTAssertEqual(folded.expanded, false)
        XCTAssertTrue(folded.spoken?.contains("collapsed") == true, folded.spoken ?? "")
    }

    func testANewBlockTypedInTheFormIsSavedAndTheFormCloses() throws {
        // The day first, as a person is on it, once the app holds the store: asked for at once,
        // on a slow runner a new block found no store yet and opened nothing.
        core.app.send(.open(.today))
        settle { core.app.view.day != nil }
        core.app.send(.block(.new(start: nil, minutes: nil)))
        settle { core.app.view.blockForm != nil }
        let form = AppBlockFormModel(core: core)
        var closed = false
        form.close = { closed = true }
        settle()
        form.fields.title = "Deep work"
        settle()
        form.save()
        settle { closed }
        XCTAssertTrue(closed, "the app let the form go")
        XCTAssertTrue(try core.app.device.plan(date: nil).blocks.contains { $0.title == "Deep work" })
    }

    func testPairingOpensOnTheCoresWordsAndAnEmptyCodeIsRefusedInThem() throws {
        let pairing = PairingModel(core: core, thisDevice: "this iPhone", name: "Test iPhone")
        XCTAssertEqual(pairing.status, pairing.words.intro)
        var said: String?
        pairing.failed = { said = $0 }
        pairing.entered = "   "
        pairing.join()
        XCTAssertEqual(said, pairing.words.needCode, "asked for a code, not paired with nothing")
    }

    func testAnExportGoesToTheLatestRequestAlone() throws {
        var first = 0
        var latest: [LumennaApp.SaveFile] = []
        core.app.file(from: .settings(.export(.json))) { _ in first += 1 }
        core.app.file(from: .settings(.export(.json))) { latest.append($0) }
        settle { !latest.isEmpty }
        settle()
        XCTAssertEqual(first, 0, "the earlier request was replaced")
        XCTAssertEqual(latest.count, 1, "and the latest answered once")
        XCTAssertFalse(latest[0].bytes.isEmpty)
    }

    func testAListMadeForAPlaceTheAppHasLeftDoesNotPullTheAppBack() {
        // As the Mac's window opens: its first turn shows the app's first place, Tasks, while
        // the window has already asked for the day.
        core.app.send(.open(.tasks))
        settle { core.app.view.place == .tasks }
        core.app.send(.open(.today))
        TaskListModel(core: core, place: .tasks).appear()
        settle()
        XCTAssertEqual(core.app.view.place, .today, "the day the window asked for, not Tasks again")
    }

    func testTheAppIconsQuickActionsAreTheCoresLaunchesAndHandEachBack() {
        let items = QuickActions.items
        XCTAssertEqual(items.map(\.localizedTitle), LumennaApp.launchShortcuts().map(\.title))
        XCTAssertEqual(items.compactMap(QuickActions.launch(of:)), LumennaApp.launchShortcuts().map(\.id))
        XCTAssertNil(QuickActions.launch(of: UIApplicationShortcutItem(type: "elsewhere", localizedTitle: "Other")))
    }

    func testALaunchWaitsForWhatShowsTheAppAgainOnceItsSceneHasGone() {
        var handled: [String] = []
        Launches.shared.handle { handled.append($0) }
        Launches.shared.letGo()
        Launches.shared.ask("today")
        XCTAssertTrue(handled.isEmpty, "not sent to a window no longer there")
        var next: [String] = []
        Launches.shared.handle { next.append($0) }
        XCTAssertEqual(next, ["today"])
        Launches.shared.letGo()
    }

    func testTheMissedSummaryIsNamedByWhatItSumsUp() {
        let missed = [("a", "2026-10-10"), ("b", "2026-10-11")].map { key, occurrence in
            LumennaApp.DueReminder(
                key: key, occurrence: occurrence, at: "2026-10-10T09:00:00Z", target: "t", kind: "block", title: "T",
                text: "T starts", missed: "T started", dismiss: "Dismiss", snooze: "Snooze", carry: "", replan: ""
            )
        }
        XCTAssertEqual(Reminders.missedIdentifier(missed), Reminders.missedIdentifier(missed), "the same each time, on every device")
        XCTAssertNotEqual(Reminders.missedIdentifier(missed), Reminders.missedIdentifier(Array(missed.prefix(1))))
    }

    func testAnInstantIsReadToTheNanosecondOrTheSecond() throws {
        let whole = try XCTUnwrap(Instant.parse("2026-10-25T09:30:00Z"))
        let fine = try XCTUnwrap(Instant.parse("2026-10-25T09:30:00.5Z"))
        XCTAssertEqual(fine.timeIntervalSince(whole), 0.5, accuracy: 0.001)
        XCTAssertEqual(Instant.parse(Instant.rfc3339(whole)), whole)
    }
}
