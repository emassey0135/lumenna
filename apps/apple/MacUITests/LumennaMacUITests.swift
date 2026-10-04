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

    /// The main window, by identifier: the first window can be a system one, such as Siri's.
    private var window: XCUIElement { app.windows["main"] }

    private func place(_ name: String) {
        window.outlines["Places"].staticTexts[name].click()
    }

    /// Puts text in a field by pasting it, then restores the clipboard.
    ///
    /// Not typed: with VoiceOver running — as it is on the machine these run on — keystrokes
    /// XCUITest synthesizes one by one can reach VoiceOver instead, which opened its Item
    /// Chooser and swallowed the rest. Command-V passes through VoiceOver untouched.
    private func enter(_ text: String, into field: XCUIElement) {
        let pasteboard = NSPasteboard.general
        let saved = pasteboard.string(forType: .string)
        field.click()
        field.typeKey("a", modifierFlags: .command)
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
        field.typeKey("v", modifierFlags: .command)
        let entered = NSPredicate(format: "value == %@", text)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation(for: entered, evaluatedWith: field)], timeout: 5), .completed,
                       "entered \(field.value ?? "")")
        pasteboard.clearContents()
        if let saved { pasteboard.setString(saved, forType: .string) }
    }

    private func addTask(_ text: String) {
        app.typeKey("n", modifierFlags: .command)
        let field = app.textFields["New task"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        enter(text, into: field)
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
        XCTAssertTrue(repeats.waitForExistence(timeout: 5), app.debugDescription)
        XCTAssertTrue(window.scrollViews["Task details"].exists, "the pane is one named scroll area")
        XCTAssertEqual(repeats.value as? String, "every monday")
    }

    func testABlockIsAddedFromTheDay() {
        XCTAssertTrue(window.outlines["The day"].waitForExistence(timeout: 5))
        app.typeKey("n", modifierFlags: [.command, .shift])
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        enter("Deep work", into: name)
        app.buttons["Save"].click()
        let day = window.outlines["The day"]
        XCTAssertTrue(
            day.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS 'Deep work'")).firstMatch
                .waitForExistence(timeout: 5),
            app.debugDescription
        )
    }

    /// Runs the audit and fails once with every issue it found, each with the element it
    /// objects to; left to itself the audit stops at the first.
    private func audit(_ screen: String, in front: XCUIElement? = nil) throws {
        var issues: [String] = []
        // The audit covers every window, and the system dims a window that is not in front —
        // one behind a sheet, or behind Settings. What is dimmed is not what anyone reads, so
        // only the window in front is judged: a sheet when one is open.
        let sheet = app.sheets.firstMatch
        let sheetFrame = sheet.exists ? sheet.frame : front?.frame
        try app.performAccessibilityAudit { issue in
            // The Touch Bar is the system's, as the keyboard's prediction bar is on iOS; it sits
            // above the screen's top edge.
            if issue.element?.elementType == .touchBar { return true }
            if let frame = issue.element?.frame, frame.minY < 0 { return true }
            // Reported inside SwiftUI's hosted forms against an element with no frame and no
            // name — nothing the app builds, and nothing it could point to. Only that finding,
            // only with no element.
            if issue.element == nil, issue.compactDescription == "Parent/Child mismatch" { return true }
            if let sheetFrame, let frame = issue.element?.frame, !sheetFrame.insetBy(dx: -1, dy: -1).contains(frame) { return true }
            // SwiftUI's pop-up menus answer VoiceOver's press but not the audit's question
            // about it; only that finding, only on pop-ups.
            if issue.element?.elementType == .popUpButton, issue.compactDescription == "Action is missing" { return true }
            let element = issue.element.map { "\($0.elementType.rawValue) '\($0.label)' '\($0.value ?? "")' \($0.frame)" }
            issues.append("\(issue.compactDescription) — \(issue.detailedDescription) [\(element ?? "unnamed element")]")
            return true
        }
        XCTAssertTrue(issues.isEmpty, "\(screen)\n" + issues.joined(separator: "\n"))
    }

    func testTheMainWindowPassesAnAccessibilityAudit() throws {
        XCTAssertTrue(window.waitForExistence(timeout: 10))
        place("Tasks")
        addTask("audit me tomorrow p1")
        let row = window.outlines["Tasks"].staticTexts["audit me"]
        XCTAssertTrue(row.waitForExistence(timeout: 5), app.debugDescription)
        row.click()
        try audit("tasks with details")
        place("Today")
        try audit("the day")
    }

    func testTheBlockFormAndEverySettingsTabPassAnAudit() throws {
        XCTAssertTrue(window.outlines["The day"].waitForExistence(timeout: 5))
        app.typeKey("n", modifierFlags: [.command, .shift])
        XCTAssertTrue(app.textFields["Name"].waitForExistence(timeout: 5))
        try audit("block form")
        app.typeKey(.escape, modifierFlags: [])

        app.typeKey(",", modifierFlags: .command)
        // By identifier: the window takes each tab's name as its title.
        let settings = app.windows["settings"]
        XCTAssertTrue(settings.waitForExistence(timeout: 5))
        for tab in ["General", "Planning", "Devices", "Backups", "Export and Import"] {
            settings.toolbars.buttons[tab].click()
            if tab != "Devices" {
                // One level: a scroll area named for the page, not a group around one.
                XCTAssertTrue(settings.scrollViews[tab].waitForExistence(timeout: 5), "\(tab): \(settings.debugDescription)")
                XCTAssertFalse(settings.groups[tab].exists, tab)
            }
            try audit("settings, \(tab)", in: settings)
        }
    }
}
