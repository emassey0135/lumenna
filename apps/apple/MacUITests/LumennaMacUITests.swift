import XCTest

/// The Mac app driven the way a VoiceOver user drives it: by accessibility label and the
/// keyboard, not by position.
final class LumennaMacUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        // A fresh store for every test, in the app's temporary directory.
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = UUID().uuidString
        app.launch()
    }

    override func tearDown() {
        app.terminate()
    }

    private var window: XCUIElement { app.windows.firstMatch }

    private func place(_ name: String) {
        window.outlines["Places"].staticTexts[name].click()
    }

    private func addTask(_ text: String) {
        app.typeKey("n", modifierFlags: .command)
        let field = app.textFields["New task"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText(text)
        app.buttons["Add"].click()
    }

    func testTheAppOpensOnTheDay() {
        XCTAssertTrue(window.waitForExistence(timeout: 10))
        XCTAssertTrue(window.outlines["The day"].waitForExistence(timeout: 5))
    }

    func testAddingATaskListsItAndSpaceCompletesIt() {
        place("Tasks")
        addTask("buy milk tomorrow")
        let outline = window.outlines["Tasks"]
        let row = outline.staticTexts["buy milk"]
        XCTAssertTrue(row.waitForExistence(timeout: 5))
        row.click()
        app.typeKey(.space, modifierFlags: [])
        XCTAssertTrue(row.waitForNonExistence(timeout: 5), "a completed task leaves the open list")
        app.typeKey("z", modifierFlags: .command)
        XCTAssertTrue(outline.staticTexts["buy milk"].waitForExistence(timeout: 5), "undo brings it back")
    }

    func testTheDetailPaneEditsTheSelectedTask() {
        place("Tasks")
        addTask("water plants every monday")
        window.outlines["Tasks"].staticTexts["water plants"].click()
        let repeats = window.textFields["Repeats"]
        XCTAssertTrue(repeats.waitForExistence(timeout: 5))
        XCTAssertEqual(repeats.value as? String, "every monday")
    }

    func testABlockIsAddedFromTheDay() {
        XCTAssertTrue(window.outlines["The day"].waitForExistence(timeout: 5))
        app.typeKey("n", modifierFlags: [.command, .shift])
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.click()
        name.typeText("Deep work")
        app.buttons["Save"].click()
        XCTAssertTrue(window.outlines["The day"].staticTexts.containing(NSPredicate(format: "value CONTAINS 'Deep work'")).firstMatch.waitForExistence(timeout: 5)
            || window.outlines["The day"].staticTexts.matching(NSPredicate(format: "label CONTAINS 'Deep work'")).firstMatch.waitForExistence(timeout: 5))
    }

    func testTheMainWindowPassesAnAccessibilityAudit() throws {
        XCTAssertTrue(window.waitForExistence(timeout: 10))
        try app.performAccessibilityAudit()
    }
}
