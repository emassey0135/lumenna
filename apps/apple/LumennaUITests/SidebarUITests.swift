import XCTest

/// The iPad's sidebar, as the Mac's: every place, headings that fold by their actions, and a
/// place's own actions. Skipped on iPhone, which has tabs. In portrait, where the sidebar is
/// a button away: in landscape XCUITest's coordinates come out rotated, and part-swipes and
/// taps land on the wrong row.
final class SidebarUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "the iPhone has tabs, not a sidebar")
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = UUID().uuidString
        app.launch()
        showSidebar()
    }

    /// The screen as it was when a check failed, kept with the results: what went wrong on a
    /// runner can be seen, not only read about.
    override func record(_ issue: XCTIssue) {
        if let app {
            let shot = XCTAttachment(screenshot: app.screenshot())
            shot.name = "Failure"
            shot.lifetime = .keepAlways
            add(shot)
        }
        super.record(issue)
    }


    /// The sidebar, shown if the window keeps it behind a button.
    private func showSidebar() {
        // A hidden sidebar's rows still exist, off the screen: only one that can be tapped will do.
        if !(place("Today").waitForExistence(timeout: 3) && place("Today").isHittable) {
            app.buttons["Show Sidebar"].firstMatch.tap()
        }
        XCTAssertTrue(place("Today").waitForExistence(timeout: 5))
    }

    private func place(_ title: String) -> XCUIElement {
        app.cells.matching(NSPredicate(format: "label == %@", title)).firstMatch
    }

    /// Runs one of a row's actions: a swipe action, as VoiceOver lists it. A part swipe:
    /// across a narrow sidebar row, a whole one runs the first action itself.
    private func act(_ row: XCUIElement, _ action: String) {
        row.coordinate(withNormalizedOffset: CGVector(dx: 0.9, dy: 0.5))
            .press(forDuration: 0.05, thenDragTo: row.coordinate(withNormalizedOffset: CGVector(dx: 0.4, dy: 0.5)))
        let button = app.buttons[action].firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 5), "\(action) is among the row's actions")
        button.tap()
    }

    private func audit() throws {
        var issues: [String] = []
        try app.performAccessibilityAudit { issue in
            // As in the other audits: SwiftUI's partly-scaling text, never a UIKit cell's.
            if issue.auditType == .dynamicType, issue.compactDescription.contains("partially"),
               issue.detailedDescription.contains("SwiftUI") { return true }
            issues.append("\(issue.compactDescription) — \(issue.detailedDescription) [\(issue.element.map { "\($0.label) \($0.frame)" } ?? "unnamed")]")
            return true
        }
        XCTAssertTrue(issues.isEmpty, issues.joined(separator: "\n"))
    }

    func testTheSidebarListsEveryPlaceAndOpensOne() throws {
        for title in ["Today", "Tasks", "Projects", "Inbox", "Labels", "Saved Filters", "Blocks", "Trash", "Settings"] {
            XCTAssertTrue(place(title).exists, "\(title) is in the sidebar")
        }
        place("Inbox").tap()
        XCTAssertTrue(app.navigationBars["Inbox"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["No task open"].exists, "nothing is open beside it yet")
        try audit()
    }

    func testAHeadingCollapsesAndExpandsByItsActionsAndSaysWhichItIs() {
        let projects = place("Projects")
        XCTAssertTrue((projects.value as? String)?.contains("expanded") == true)
        act(projects, "Collapse")
        XCTAssertTrue(place("Inbox").waitForNonExistence(timeout: 5))
        XCTAssertTrue((projects.value as? String)?.contains("collapsed") == true)
        act(projects, "Expand")
        XCTAssertTrue(place("Inbox").waitForExistence(timeout: 5))
    }

    func testANewProjectIsMadeFromItsHeadingAndListedUnderIt() {
        act(place("Projects"), "New Project")
        let field = app.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText("Garden")
        app.alerts.buttons["Save"].tap()
        XCTAssertTrue(place("Garden").waitForExistence(timeout: 5))
        place("Garden").tap()
        XCTAssertTrue(app.navigationBars["Garden"].waitForExistence(timeout: 5))
        // Quick add starts with the project, so the task lands in it.
        app.buttons["Add task"].tap()
        let line = app.textViews["New task"]
        XCTAssertTrue(line.waitForExistence(timeout: 5))
        line.typeText("dig the beds")
        app.buttons["Add"].tap()
        XCTAssertTrue(app.cells.matching(NSPredicate(format: "label == 'dig the beds'")).firstMatch.waitForExistence(timeout: 5))
        // ⌘Z just after quick add closes does not reach the list on iPad yet (ROADMAP); it does
        // from the list itself (KeyboardUITests).
    }

    func testALabelAndASavedFilterAreMadeFromTheirHeadings() throws {
        act(place("Labels"), "New Label")
        var field = app.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText("calls")
        app.alerts.buttons["Save"].tap()
        XCTAssertTrue(place("calls").waitForExistence(timeout: 5))

        act(place("Saved Filters"), "New Saved Filter")
        field = app.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText("Urgent")
        app.alerts.buttons["Next"].tap()
        field = app.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText("p1")
        app.alerts.buttons["Save"].tap()
        XCTAssertTrue(place("Urgent").waitForExistence(timeout: 5))
        try audit()
        // A label's own actions are offered on it, as in Browse on the iPhone.
        act(place("calls"), "Rename")
        XCTAssertTrue(app.alerts["Rename calls"].waitForExistence(timeout: 5))
    }

    func testSettingsOpensItsPagesBesideItsList() {
        place("Settings").tap()
        app.cells.containing(NSPredicate(format: "label CONTAINS 'Planning'")).firstMatch.tap()
        XCTAssertTrue(app.navigationBars["Planning"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.navigationBars["Settings"].exists, "the list of pages stays beside the page")
    }
}

