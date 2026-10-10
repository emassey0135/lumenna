import XCTest

/// The watch app driven by accessibility label, as VoiceOver reaches it, on a store of its own.
final class LumennaWatchUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = UUID().uuidString
        // Not synced: the simulator's watch is paired with a phone whose store would arrive.
        app.launchEnvironment["LUMENNA_NO_SYNC"] = "1"
        app.launch()
    }

    /// Runs the audit and fails once with every issue it found, each with its element.
    private func audit(_ screen: String) throws {
        var issues: [String] = []
        try app.performAccessibilityAudit { issue in
            let element = issue.element.map { "\($0.elementType.rawValue) '\($0.label)'" }
            issues.append("\(issue.compactDescription) — \(issue.detailedDescription) [\(element ?? "unnamed element")]")
            return true
        }
        XCTAssertTrue(issues.isEmpty, "\(screen)\n" + issues.joined(separator: "\n"))
    }

    /// Types into a text field as a person does on a watch: the field opens the system's
    /// input screen, where the text goes, and Done brings it back. `clearing` deletes what
    /// is there first.
    private func type(_ text: String, into field: XCUIElement, clearing: Int = 0) {
        reveal(field)
        field.tap()
        let input = app.textViews.firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        input.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: clearing) + text)
        app.buttons["Done"].tap()
    }

    private func add(_ text: String) {
        XCTAssertTrue(app.buttons["New Task"].firstMatch.waitForExistence(timeout: 10))
        app.buttons["New Task"].firstMatch.tap()
        type(text, into: app.textFields["Task"])
        reveal(app.buttons["Add"]).tap()
    }

    /// Back up one screen.
    private func back() {
        app.navigationBars.buttons["Back"].firstMatch.tap()
    }

    /// Scrolls until `element` is on the screen: a watch's list holds only the rows it shows.
    /// By the Digital Crown, a little at a time, since even a slow swipe moved most of a list
    /// and past what was looked for; in a `form`, by swipes, since there the Crown goes to
    /// the control in focus, a date picker or a stepper, and scrolls nothing. `upward` goes
    /// back towards the top.
    @discardableResult
    private func reveal(_ element: XCUIElement, form: Bool = false, upward: Bool = false) -> XCUIElement {
        for _ in 0..<30 where !(element.exists && element.isHittable) {
            switch (form, upward) {
            case (true, false): app.swipeUp(velocity: .slow)
            case (true, true): app.swipeDown(velocity: .slow)
            case (false, _): XCUIDevice.shared.rotateDigitalCrown(delta: upward ? -0.15 : 0.15)
            }
        }
        XCTAssertTrue(element.exists, "never scrolled to \(element)")
        return element
    }

    func testThePlacesAreListedAndPassAnAudit() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["Tasks"].exists)
        // A place's name carries what is in it: "Inbox, no tasks".
        reveal(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Inbox'")).firstMatch)
        try audit("places")
    }

    func testQuickAddReadsBackThenTheTaskIsListedAndOpens() throws {
        XCTAssertTrue(app.buttons["Tasks"].waitForExistence(timeout: 10))
        app.buttons["New Task"].firstMatch.tap()
        type("call the bank tomorrow p1", into: app.textFields["Task"])
        let readback = app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'priority 1'")).firstMatch
        XCTAssertTrue(readback.waitForExistence(timeout: 5), "what will be saved is read back first")
        try audit("quick add")
        reveal(app.buttons["Add"]).tap()

        XCTAssertTrue(app.buttons["Tasks"].waitForExistence(timeout: 5))
        app.buttons["Tasks"].tap()
        let row = app.buttons["call the bank"]
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        XCTAssertTrue((row.value as? String ?? "").contains("due tomorrow"), "\(row.value ?? "")")
        try audit("tasks")
        row.tap()
        XCTAssertTrue(app.navigationBars["Task"].waitForExistence(timeout: 5))
        try audit("task details")
        reveal(app.buttons["Mark Done"], form: true).tap()
        // The details redraw from the top, where the list holds only what it shows.
        reveal(app.buttons["Mark Not Done"], form: true)
    }

    func testTheDayOpensOnTodayAndPassesAnAudit() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        app.buttons["Today"].tap()
        XCTAssertTrue(app.staticTexts["Today. No blocks"].waitForExistence(timeout: 5) || app.navigationBars["Today"].exists)
        try audit("the day")
        reveal(app.buttons["Next Day"]).tap()
        XCTAssertTrue(reveal(app.buttons["Today"]).exists, "a way back to today")
    }

    func testATasksFieldsAreEditedInThePhonesFormAndSaved() throws {
        add("water plants")
        app.buttons["Tasks"].tap()
        app.buttons["water plants"].tap()
        let title = app.textFields["Title"]
        XCTAssertTrue(title.waitForExistence(timeout: 5), "the form the phone edits with")
        try audit("the task form")
        type("water the plants", into: title, clearing: "water plants".count)
        reveal(app.buttons["Save"], form: true).tap()
        back()
        XCTAssertTrue(app.buttons["water the plants"].waitForExistence(timeout: 5))
    }

    func testQuickAddOffersWhatFinishesTheLastWordOnceTheLineIsEntered() throws {
        XCTAssertTrue(app.buttons["New Task"].firstMatch.waitForExistence(timeout: 10))
        app.buttons["New Task"].firstMatch.tap()
        let field = app.textFields["Task"]
        type("call the bank #In", into: field)
        // Named "project Inbox", not "#Inbox": the sigil is punctuation VoiceOver may skip.
        let offered = reveal(app.buttons["project Inbox"])
        try audit("completions")
        offered.tap()
        reveal(field, upward: true)
        XCTAssertTrue(((field.value as? String) ?? "").contains("#Inbox"), "\(field.value ?? "")")
    }

    func testABlockIsAddedFromTheDayInThePhonesForm() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        app.buttons["Today"].tap()
        reveal(app.buttons["Add Block"]).tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        try audit("the block form")
        type("Deep work", into: name)
        reveal(app.buttons["Save"], form: true).tap()
        let block = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Deep work'")).firstMatch
        XCTAssertTrue(block.waitForExistence(timeout: 5) || reveal(block).exists)
    }

    func testSettingsShowWhatSyncsAndPassAnAudit() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        reveal(app.buttons["Settings"]).tap()
        XCTAssertTrue(app.switches["Completing a task completes its subtasks"].waitForExistence(timeout: 5))
        try audit("settings")
    }

    func testAProjectIsMadeFromItsHeadingAndRenamed() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        reveal(app.buttons["New Project"]).tap()
        type("Garden", into: app.textFields["New Project"])
        reveal(app.buttons["Add"]).tap()
        let garden = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Garden'")).firstMatch
        reveal(garden).tap()
        reveal(app.buttons["Rename"]).tap()
        type("Yard", into: app.textFields["Rename Garden"], clearing: "Garden".count)
        app.buttons["Save"].tap()
        let yard = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Yard'")).firstMatch
        XCTAssertTrue(reveal(yard).exists, "renamed, and back among the places")
    }
}
