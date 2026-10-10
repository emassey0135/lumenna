import XCTest

/// What VoiceOver itself says, with VoiceOver on: iOS 27's `XCUIVoiceOverService` moves it
/// and hands back its speech, so these check the screen reader's own output, not only the
/// accessibility tree. Actions go through the keyboard commands, which act on VoiceOver's
/// row: no swipe, so nothing here depends on a gesture landing.
@available(iOS 27, *)
@MainActor
final class VoiceOverUITests: XCTestCase {
    private var app: XCUIApplication!
    private var voiceOver: XCUIVoiceOverService { XCUIDevice.shared.voiceOverService }

    override func setUp() async throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = UUID().uuidString
        app.launch()
    }

    override func tearDown() async throws {
        try? voiceOver.disable()
    }

    /// Adds tasks through quick add, before VoiceOver is on.
    private func add(_ texts: String...) {
        let tasks = app.tabBars.buttons["Tasks"]
        if tasks.exists { tasks.tap() } else { app.typeKey("2", modifierFlags: .command) }
        for text in texts {
            app.buttons["Add task"].tap()
            let field = app.textViews["New task"]
            XCTAssertTrue(field.waitForExistence(timeout: 5))
            field.typeText(text)
            app.buttons["Add"].tap()
        }
    }

    /// Moves VoiceOver forward until it says `words`, and returns what it said.
    @discardableResult
    private func move(to words: String, within steps: Int = 20) throws -> String {
        var said = try voiceOver.currentSpeech().utterance
        for _ in 0..<steps where !said.contains(words) {
            said = try voiceOver.moveForward().utterance
        }
        XCTAssertTrue(said.contains(words), "VoiceOver never reached \"\(words)\"; it last said \"\(said)\"")
        return said
    }

    func testATaskIsReadAsItsTitleThenWhatIsDueThenThatItHasActions() throws {
        add("call the bank tomorrow")
        try voiceOver.enable()
        let said = try move(to: "call the bank")
        XCTAssertTrue(said.contains("due"), said)
        XCTAssertTrue(said.contains("Actions"), "its swipe actions are offered to VoiceOver: \(said)")
    }

    func testMarkingATaskDoneLeavesVoiceOverOnTheTaskNowInItsPlace() throws {
        add("call the bank", "water the plants")
        try voiceOver.enable()
        try move(to: "call the bank")
        app.typeKey("k", modifierFlags: .command)
        // Everything said meanwhile: the row focus lands on, then the core's announcement.
        var heard: [String] = []
        let landed = waitUntil {
            let said = (try? self.voiceOver.currentSpeech().utterance) ?? ""
            if heard.last != said { heard.append(said) }
            return said.contains("water the plants")
        }
        let now = (try? voiceOver.currentSpeech().utterance) ?? ""
        XCTAssertTrue(landed, "VoiceOver stays on the row now in the completed one's place; it said \(heard), now \(now)")
    }

    func testThePairingCodeFieldIsFollowedByAPasteButton() throws {
        let settings = app.tabBars.buttons["Settings"]
        if settings.exists { settings.tap() } else { app.typeKey(",", modifierFlags: .command) }
        let devices = app.cells.containing(NSPredicate(format: "label CONTAINS 'Devices and Sync'")).firstMatch
        XCTAssertTrue(devices.waitForExistence(timeout: 5))
        devices.tap()
        let pair = app.buttons["Pair a device"]
        XCTAssertTrue(pair.waitForExistence(timeout: 10))
        pair.tap()
        XCTAssertTrue(app.textViews["Code from the other device"].waitForExistence(timeout: 5))
        try voiceOver.enable()
        try move(to: "Code from the other device")
        let said = try voiceOver.moveForward().utterance
        XCTAssertTrue(said.contains("Paste"), "after the field VoiceOver said \"\(said)\"")
        XCTAssertTrue(said.contains("Button"), said)
    }

    /// SwiftUI's picker reached VoiceOver unnamed, its title a separate stop before it.
    func testTheBlockFormsStartIsReadAsStartsAt() throws {
        let today = app.tabBars.buttons["Today"]
        if today.exists { today.tap() } else { app.typeKey("1", modifierFlags: .command) }
        app.buttons["Add block"].tap()
        XCTAssertTrue(app.textFields["Name"].waitForExistence(timeout: 5))
        try voiceOver.enable()
        var heard: [String] = []
        var said = try voiceOver.currentSpeech().utterance
        for _ in 0..<12 where !said.contains("Lasts") {
            heard.append(said)
            said = try voiceOver.moveForward().utterance
        }
        let start = heard.first { $0.contains("PM") || $0.contains("AM") }
        XCTAssertNotNil(start, "no time was read: \(heard)")
        XCTAssertTrue(start?.hasPrefix("Starts at") == true, "the time picker is read as \"\(start ?? "")\"; heard \(heard)")
        XCTAssertEqual(heard.filter { $0.contains("Starts at") }.count, 1, "said more than once: \(heard)")
    }

    func testTheDayIsReadAsAHeadingThenItsButtons() throws {
        try voiceOver.enable()
        try move(to: "Heading")
        try move(to: "Add block Button")
        try move(to: "Previous Day Button")
        try move(to: "Go to Day Button")
    }


    /// Every action VoiceOver offers on the row it is on, in its order, read by stepping its
    /// actions rotor (VO-Command-Down Arrow) until it comes round to the first again.
    private func actionsOffered() -> [String] {
        var offered: [String] = []
        for _ in 0..<30 {
            app.typeKey(.downArrow, modifierFlags: [.control, .option, .command])
            let said = waitForSpeech(after: offered.last)
            if offered.contains(said) { break }
            offered.append(said)
        }
        return offered
    }

    /// What VoiceOver says once it says something other than `previous`.
    private func waitForSpeech(after previous: String?) -> String {
        var said = ""
        _ = waitUntil {
            said = (try? self.voiceOver.currentSpeech().utterance) ?? ""
            return !said.isEmpty && said != previous
        }
        return said
    }

    func testATaskOffersVoiceOverEachOfItsActionsOnceTheSwipedOnesAmongThem() throws {
        add("call the bank tomorrow")
        try voiceOver.enable()
        try move(to: "call the bank")
        let offered = actionsOffered()
        let actions = ["Mark Done", "Edit Details", "Put in a Block", "Move to Project", "Make Subtask Of", "Wait For", "Move to Trash"]
        for action in actions {
            XCTAssertEqual(offered.filter { $0 == action }.count, 1, "\(action) once among \(offered)")
        }
        print("VoiceOver offers a task: \(offered)")
        let others = offered.filter { !actions.contains($0) }
        XCTAssertTrue(others.allSatisfy { $0.localizedCaseInsensitiveContains("activate") || $0.localizedCaseInsensitiveContains("context menu") },
                      "nothing else but VoiceOver's own: \(offered)")
    }

    func testABlockOffersVoiceOverEachOfItsActionsOnceItsMenuOnlyActionsAmongThem() throws {
        app.buttons["Add block"].tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Deep work")
        let repeats = app.textFields["Repeats"]
        repeats.tap()
        repeats.typeText("every day")
        app.buttons["Save"].tap()
        try voiceOver.enable()
        try move(to: "Deep work", within: 30)
        let offered = actionsOffered()
        print("VoiceOver offers a block: \(offered)")
        for action in ["Assign a Task", "Edit Block", "Cancel This Day", "Delete Block"] {
            XCTAssertEqual(offered.filter { $0 == action }.count, 1, "\(action) once among \(offered)")
        }
    }

    /// Whether `condition` comes true within five seconds, asked four times a second.
    private func waitUntil(_ condition: () -> Bool) -> Bool {
        for _ in 0..<20 {
            if condition() { return true }
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.25))
        }
        return condition()
    }
}
