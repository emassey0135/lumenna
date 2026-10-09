import XCTest

/// The keyboard commands every other app has, from a hardware keyboard: the iPad's menu bar,
/// and the iPhone's key commands. On iPad the commands exist only in the menu bar, so these
/// keys working there is the menu bar working; XCUITest cannot see the bar itself while it
/// is hidden. And on iPad, what is opened from a list shows beside it.
final class KeyboardUITests: XCTestCase {
    private var app: XCUIApplication!
    private var pad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = UUID().uuidString
        app.launch()
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


    /// A tab on iPhone; on iPad, the sidebar's place of that name.
    private func tab(_ name: String) {
        if !pad {
            app.tabBars.buttons[name].tap()
            return
        }
        // A hidden sidebar's rows still exist, off the screen: only one that can be tapped will do.
        let place = app.cells.matching(NSPredicate(format: "label == %@", name)).firstMatch
        if !(place.waitForExistence(timeout: 2) && place.isHittable) { app.buttons["Show Sidebar"].firstMatch.tap() }
        place.tap()
    }

    private func press(_ key: String, _ modifiers: XCUIElement.KeyModifierFlags = .command) {
        app.typeKey(key, modifierFlags: modifiers)
    }

    private func row(_ title: String) -> XCUIElement {
        app.cells.matching(NSPredicate(format: "label == %@", title)).firstMatch
    }

    private func add(_ text: String) {
        press("n")
        let field = app.textViews["New task"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText(text)
        app.buttons["Add"].tap()
        XCTAssertTrue(row(text).waitForExistence(timeout: 5))
    }

    private var title: String { app.navigationBars.firstMatch.identifier }

    func testCommandNOpensQuickAddFromAnotherTab() {
        tab("Settings")
        press("n")
        XCTAssertTrue(app.textViews["New task"].waitForExistence(timeout: 5))
    }

    func testCommandZUndoesAndShiftCommandZRedoes() {
        add("water the plants")
        press("z")
        XCTAssertTrue(row("water the plants").waitForNonExistence(timeout: 5))
        press("z", [.command, .shift])
        XCTAssertTrue(row("water the plants").waitForExistence(timeout: 5))
    }

    func testCommandFPutsTheFilterInHand() throws {
        // In the iPad simulator ⌘F never reaches the app: no responder is asked to do
        // anything with it, where ⌘3 is. Filter Tasks is still in the Edit menu.
        try XCTSkipIf(pad, "iPadOS keeps ⌘F from the app in the simulator")
        tab("Today")
        press("f")
        let filter = app.textViews["Filter"]
        let focused = expectation(for: NSPredicate(format: "hasKeyboardFocus == true"), evaluatedWith: filter)
        wait(for: [focused], timeout: 10)
    }

    func testCommandThreeAndFourOpenBlocksAndTheTrash() {
        press("3")
        XCTAssertTrue(app.navigationBars["Blocks"].waitForExistence(timeout: 5))
        press("4")
        XCTAssertTrue(app.navigationBars["Trash"].waitForExistence(timeout: 5))
        press("1")
        XCTAssertTrue(app.buttons["Next Day"].waitForExistence(timeout: 5))
    }

    func testCommandBracketsChangeTheDayAndCommandTComesBackToNow() {
        tab("Today")
        XCTAssertTrue(app.buttons["Next Day"].waitForExistence(timeout: 5))
        let today = title
        press("]")
        XCTAssertTrue(app.navigationBars.matching(NSPredicate(format: "identifier != %@", today)).firstMatch.waitForExistence(timeout: 5))
        press("t")
        XCTAssertTrue(app.navigationBars[today].waitForExistence(timeout: 5))
    }

    // MARK: - iPad

    func testOnIPadATaskOpensBesideItsListAndCommandKMarksItDone() throws {
        try XCTSkipUnless(pad, "the iPhone opens a task in place of its list")
        add("plan the trip")
        row("plan the trip").tap()
        XCTAssertTrue(app.buttons["Save"].waitForExistence(timeout: 5))
        // The list is still there, beside the task.
        XCTAssertTrue(row("plan the trip").exists)
        press("k")
        XCTAssertTrue(row("plan the trip").waitForNonExistence(timeout: 5))
    }
}


