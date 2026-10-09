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
    /// input screen, where the text goes, and Done brings it back.
    private func type(_ text: String, into field: XCUIElement) {
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.tap()
        let input = app.textViews.firstMatch
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        input.typeText(text)
        app.buttons["Done"].tap()
    }

    /// Scrolls the list until `element` is on the screen: a watch's list holds only the rows
    /// it shows.
    @discardableResult
    private func reveal(_ element: XCUIElement) -> XCUIElement {
        // By the Digital Crown, a little at a time: even a slow swipe moved most of a list
        // and past what was looked for.
        for _ in 0..<30 where !(element.exists && element.isHittable) {
            XCUIDevice.shared.rotateDigitalCrown(delta: 0.15)
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
        app.buttons["Add"].tap()

        XCTAssertTrue(app.buttons["Tasks"].waitForExistence(timeout: 5))
        app.buttons["Tasks"].tap()
        let row = app.buttons["call the bank"]
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        XCTAssertTrue((row.value as? String ?? "").contains("due tomorrow"), "\(row.value ?? "")")
        try audit("tasks")
        row.tap()
        XCTAssertTrue(app.navigationBars["Task"].waitForExistence(timeout: 5))
        try audit("task details")
        reveal(app.buttons["Mark Done"]).tap()
        // The details redraw from the top, where the list holds only what it shows.
        reveal(app.buttons["Mark Not Done"])
    }

    func testTheDayOpensOnTodayAndPassesAnAudit() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        app.buttons["Today"].tap()
        XCTAssertTrue(app.staticTexts["Today. No blocks"].waitForExistence(timeout: 5) || app.navigationBars["Today"].exists)
        try audit("the day")
        reveal(app.buttons["Next Day"]).tap()
        XCTAssertTrue(reveal(app.buttons["Today"]).exists, "a way back to today")
    }
}
