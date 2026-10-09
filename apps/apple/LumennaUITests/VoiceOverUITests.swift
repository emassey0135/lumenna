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
        XCTAssertTrue(waitUntil { (try? self.voiceOver.currentSpeech().utterance)?.contains("water the plants") == true },
                      "VoiceOver stays on the row now in the completed one's place")
    }

    func testTheDayIsReadAsAHeadingThenItsButtons() throws {
        try voiceOver.enable()
        try move(to: "Heading")
        try move(to: "Add block Button")
        try move(to: "Previous Day Button")
        try move(to: "Go to Day Button")
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
