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
            // A short drag held at its end, which cannot fling: a swipe, even a slow one, could
            // carry the form two screens on and past what was looked for.
            case (true, let up): drag(by: up ? 90 : -90)
            case (false, _): XCUIDevice.shared.rotateDigitalCrown(delta: upward ? -0.15 : 0.15)
            }
        }
        XCTAssertTrue(element.exists, "never scrolled to \(element)")
        return element
    }

    private func drag(by distance: CGFloat) {
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: distance < 0 ? 0.75 : 0.35))
        start.press(forDuration: 0.05, thenDragTo: start.withOffset(CGVector(dx: 0, dy: distance)), withVelocity: .slow, thenHoldForDuration: 0.2)
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
        // Each picker's name above it, and its parts still named for what each sets.
        for (picker, part) in [("Day", "Month"), ("Starts at", "Hour")] {
            let row = app.cells.containing(.staticText, identifier: picker).containing(NSPredicate(format: "label BEGINSWITH %@", part)).firstMatch
            XCTAssertTrue(row.exists || reveal(row, form: true).exists, "no \(picker) picker: \(app.debugDescription)")
        }
        type("Deep work", into: name)
        reveal(app.buttons["Save"], form: true).tap()
        let block = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Deep work'")).firstMatch
        XCTAssertTrue(block.waitForExistence(timeout: 5) || reveal(block).exists)
    }

    /// The day's heading offers the day's own actions; Plan the Day opens the phone's screen,
    /// each sitting a switch named by its sentence, and planning closes it when nothing more is
    /// proposed.
    func testTheDaysHeadingOffersPlanTheDayWhichPlansWhatIsChecked() throws {
        add("write the report p1")
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        app.buttons["Today"].tap()
        // Tomorrow, so its block is still to come whenever the test runs.
        reveal(app.buttons["Next Day"]).tap()
        reveal(app.buttons["Add Block"]).tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        type("Writing", into: name)
        reveal(app.buttons["Save"], form: true).tap()
        let heading = app.buttons["day.heading"]
        XCTAssertTrue(heading.waitForExistence(timeout: 5) || reveal(heading, upward: true).exists)
        reveal(heading, upward: true).tap()
        reveal(app.buttons["Plan the Day"]).tap()
        let sitting = app.switches.matching(NSPredicate(format: "label BEGINSWITH 'write the report goes in Writing'")).firstMatch
        XCTAssertTrue(sitting.waitForExistence(timeout: 5), app.debugDescription)
        try audit("plan the day")
        reveal(app.buttons["Plan the Ones Checked"], form: true).tap()
        XCTAssertTrue(sitting.waitForNonExistence(timeout: 5), "nothing more to plan, so it closes")
        let planned = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'write the report'")).firstMatch
        XCTAssertTrue(planned.waitForExistence(timeout: 5) || reveal(planned).exists)
    }

    /// A block's Reminders, from what its row offers: the default listed, a phrase the core
    /// reads added, and one it refuses kept with the core's reason.
    func testABlocksRemindersTakeAPhraseAndARefusedOneSaysWhy() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        app.buttons["Today"].tap()
        reveal(app.buttons["Add Block"]).tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        type("Deep work", into: name)
        reveal(app.buttons["Save"], form: true).tap()
        let block = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Deep work'")).firstMatch
        XCTAssertTrue(block.waitForExistence(timeout: 5) || reveal(block).exists)
        reveal(block).tap()
        reveal(app.buttons["Reminders"]).tap()
        let standing = app.staticTexts.matching(NSPredicate(format: "label CONTAINS '5 minutes before it starts'")).firstMatch
        XCTAssertTrue(standing.waitForExistence(timeout: 5), app.debugDescription)
        try audit("reminders")
        let field = app.textFields["When"]
        type("when it feels right", into: field)
        reveal(app.buttons["Add Reminder"], form: true).tap()
        let reason = app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'is not a reminder'")).firstMatch
        XCTAssertTrue(reason.waitForExistence(timeout: 5) || reveal(reason, form: true).exists, "the core's reason, with the field")
        type("at end", into: field, clearing: "when it feels right".count)
        reveal(app.buttons["Add Reminder"], form: true).tap()
        let own = app.buttons.matching(NSPredicate(format: "label CONTAINS 'when it ends'")).firstMatch
        XCTAssertTrue(own.waitForExistence(timeout: 5) || reveal(own, form: true, upward: true).exists, app.debugDescription)
    }

    func testSettingsShowWhatSyncsAndPassAnAudit() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        reveal(app.buttons["Settings"]).tap()
        XCTAssertTrue(app.buttons["Planning"].waitForExistence(timeout: 5))
        app.buttons["Planning"].tap()
        XCTAssertTrue(app.switches["Completing a task completes its subtasks"].waitForExistence(timeout: 5))
        try audit("planning settings")
        // The default reminders, phrases, and whether this watch gives them.
        XCTAssertTrue(reveal(app.textFields["Reminders for a block"], form: true).exists)
        XCTAssertTrue(reveal(app.switches["Reminders on this device"], form: true).exists)
        try audit("planning settings, reminders")
        back()
        app.buttons["Devices"].tap()
        XCTAssertTrue(app.staticTexts["No devices are paired yet. Pair one to sync with it."].waitForExistence(timeout: 5), "the synced list, empty in a fresh store")
        try audit("devices")
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
        // The question's own button, which answers it, over the action that asked it.
        let answer = app.buttons.matching(NSPredicate(format: "label == 'Rename'")).allElementsBoundByIndex.last { $0.isHittable }
        XCTAssertNotNil(answer, "the question answers with Rename")
        answer?.tap()
        let yard = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Yard'")).firstMatch
        XCTAssertTrue(reveal(yard).exists, "renamed, and back among the places")
    }

    func testAHeadingFoldsWhatIsUnderItAndSaysWhichItIs() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        let projects = reveal(app.buttons["Projects"])
        XCTAssertEqual(projects.value as? String, "expanded")
        projects.tap()
        XCTAssertEqual(app.buttons["Projects"].value as? String, "collapsed")
        XCTAssertFalse(app.buttons["New Project"].exists, "what was under it is folded away")
        app.buttons["Projects"].tap()
        XCTAssertTrue(reveal(app.buttons["New Project"]).exists)
    }

    func testTheDayGoesToAChosenDayAndBack() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        app.buttons["Today"].tap()
        reveal(app.buttons["Go to Day"]).tap()
        // Its field first; the picker and Go are further down, a watch list holding only
        // the rows on screen.
        XCTAssertTrue(app.textFields["Day"].waitForExistence(timeout: 5))
        // The picker's name above it, and its parts still named for what each sets.
        let picker = app.cells.containing(.staticText, identifier: "Day").containing(NSPredicate(format: "label BEGINSWITH 'Month'")).firstMatch
        XCTAssertTrue(picker.exists || reveal(picker).exists, "the day's picker is unnamed: \(app.debugDescription)")
        try audit("go to day")
        reveal(app.buttons["Go"]).tap()
        XCTAssertTrue(app.navigationBars["Today"].waitForExistence(timeout: 5), "today chosen is today")
    }

    func testTheDayGoesToADayTypedInWords() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        app.buttons["Today"].tap()
        reveal(app.buttons["Go to Day"]).tap()
        let field = app.textFields["Day"]
        XCTAssertTrue(field.waitForExistence(timeout: 5), app.debugDescription)
        type("someday soon", into: field)
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS[c] 'someday soon'")).firstMatch.waitForExistence(timeout: 5),
                      "a day it cannot read, said: \(app.debugDescription)")
        type("12 October", into: field, clearing: "someday soon".count)
        // Within a day of 12 October the day is said as Yesterday, Today or Tomorrow instead.
        let today = Calendar.current.dateComponents([.month, .day], from: .now)
        let near = today.month == 10 ? ["13": "Yesterday", "12": "Today", "11": "Tomorrow"][String(today.day ?? 0)] : nil
        let names = ["12 October", "October 12"] + [near].compactMap { $0 }
        let named = NSPredicate(format: "identifier IN %@ OR identifier CONTAINS %@ OR identifier CONTAINS %@", names, names[0], names[1])
        let said = NSPredicate(format: "label CONTAINS %@ OR label CONTAINS %@ OR label BEGINSWITH %@", names[0], names[1], (near ?? names[0]) + ".")
        XCTAssertTrue(app.navigationBars.matching(named).firstMatch.waitForExistence(timeout: 5)
            || app.staticTexts.matching(said).firstMatch.exists,
                      app.debugDescription)
    }

    func testATasksProjectIsChosenFromTheProjects() throws {
        XCTAssertTrue(app.buttons["Today"].waitForExistence(timeout: 10))
        reveal(app.buttons["New Project"]).tap()
        type("Garden", into: app.textFields["New Project"])
        reveal(app.buttons["Add"]).tap()
        reveal(app.buttons["New Task"].firstMatch, upward: true)
        add("water plants")
        app.buttons["Tasks"].tap()
        app.buttons["water plants"].tap()
        let project = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Project'")).firstMatch
        reveal(project, form: true).tap()
        let garden = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Garden'")).firstMatch
        XCTAssertTrue(garden.waitForExistence(timeout: 5), app.debugDescription)
        try audit("the projects to choose from")
        reveal(garden).tap()
        reveal(app.buttons["Save"], form: true).tap()
        XCTAssertTrue(reveal(project, form: true, upward: true).exists)
        let said = "\(project.label) \(project.value as? String ?? "")"
        XCTAssertTrue(said.contains("Garden"), said)
    }
}
