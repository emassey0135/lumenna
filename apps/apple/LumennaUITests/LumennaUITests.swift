import XCTest

/// The app driven the way a VoiceOver user drives it: by accessibility label, not by position.
final class LumennaUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        // A fresh store for every test, in the app's temporary directory.
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = UUID().uuidString
        app.launch()
    }

    private func add(_ text: String) {
        app.buttons["Add task"].tap()
        let field = app.textViews["New task"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText(text)
        app.buttons["Add"].tap()
    }

    private func row(_ title: String) -> XCUIElement {
        app.cells.matching(NSPredicate(format: "label == %@", title)).firstMatch
    }

    func testAddingATaskReadsItBackAndListsIt() {
        app.buttons["Add task"].tap()
        let field = app.textViews["New task"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText("write the chapter p1")
        // The readback names what will be saved before anything is.
        let readback = app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'priority 1'"))
        XCTAssertTrue(readback.firstMatch.waitForExistence(timeout: 5))
        app.buttons["Add"].tap()

        XCTAssertTrue(row("write the chapter").waitForExistence(timeout: 5))
    }

    func testTodayMeansTodayWhereThePhoneIs() {
        add("water the plants today")
        // In the phone's own time zone: the ISO style's default is UTC, which is already
        // tomorrow every evening in the Americas.
        let today = Date.now.formatted(
            Date.ISO8601FormatStyle(timeZone: .current).year().month().day()
        )
        let cell = row("water the plants")
        XCTAssertTrue(cell.waitForExistence(timeout: 5))
        XCTAssertTrue(
            (cell.value as? String ?? "").contains(today),
            "due \(today), but the row says \(cell.value ?? "nothing")"
        )
    }

    func testCompletingRemovesTheTaskAndUndoBringsItBack() {
        add("review PR")
        let cell = row("review PR")
        XCTAssertTrue(cell.waitForExistence(timeout: 5))

        cell.swipeRight()
        app.buttons["Mark Done"].tap()
        XCTAssertTrue(cell.waitForNonExistence(timeout: 5))

        app.buttons["Undo"].tap()
        XCTAssertTrue(row("review PR").waitForExistence(timeout: 5))
    }

    func testAFilterSaysHowItWasUnderstood() {
        add("urgent thing p1")
        add("other thing")
        let filter = app.textViews["Filter"]
        filter.tap()
        filter.typeText("p1")
        let readback = app.staticTexts.containing(NSPredicate(format: "label CONTAINS '1 task'"))
        XCTAssertTrue(readback.firstMatch.waitForExistence(timeout: 5))
        XCTAssertTrue(row("urgent thing").exists)
        XCTAssertFalse(row("other thing").exists)
    }

    func testEditingATaskSavesOnlyWhatChanged() {
        add("draft")
        row("draft").tap()
        // Found by its label, which a SwiftUI text field loses once it has text unless it
        // is given one — this test is what caught that.
        let title = app.textFields["Title"]
        if !title.waitForExistence(timeout: 5) {
            XCTFail("no Title field in:\n\(app.debugDescription)")
        }
        // A double tap selects the word, so what is typed replaces it wherever the cursor was.
        title.doubleTap()
        title.typeText("draft the essay")
        app.buttons["Save"].tap()
        app.navigationBars.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(row("draft the essay").waitForExistence(timeout: 5))
    }

    /// Runs the audit and fails once with every issue it found, each with the element it
    /// objects to. Left to itself the audit stops at the first, which hides the rest.
    private func audit() throws {
        var issues: [String] = []
        try app.performAccessibilityAudit { issue in
            // The keyboard's predictive-text cells are the system's, not this app's, and
            // nothing here can label them.
            if issue.detailedDescription.contains("TUIPredictionViewCell") {
                return true
            }
            let element = issue.element.map { "\($0.elementType.rawValue) '\($0.label)' \($0.frame)" }
            issues.append("\(issue.compactDescription) — \(issue.detailedDescription) [\(element ?? "unnamed element")]")
            return true
        }
        XCTAssertTrue(issues.isEmpty, "\n" + issues.joined(separator: "\n"))
    }

    func testTheTaskListPassesAnAccessibilityAudit() throws {
        add("write the chapter tomorrow p1")
        XCTAssertTrue(row("write the chapter").waitForExistence(timeout: 5))
        try audit()
    }

    func testQuickAddPassesAnAccessibilityAudit() throws {
        app.buttons["Add task"].tap()
        XCTAssertTrue(app.textViews["New task"].waitForExistence(timeout: 5))
        app.textViews["New task"].typeText("call mum friday")
        try audit()
    }
}
