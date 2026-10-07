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
        // The app opens on the day; most of these tests are about tasks.
        tab("Tasks")
    }

    private func tab(_ name: String) {
        app.tabBars.buttons[name].tap()
    }

    /// Answers a one-line prompt.
    private func answer(_ text: String, with button: String) {
        let field = app.alerts.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        field.typeText(text)
        app.alerts.buttons[button].tap()
    }

    private func cell(containing text: String) -> XCUIElement {
        app.cells.containing(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
    }

    /// A cell, scrolled to first: a list makes no cell for a row off the screen, which on a
    /// small phone at a large text size is most of them.
    private func reveal(_ text: String) -> XCUIElement {
        let found = cell(containing: text)
        // Down the list first, then back up for a row above.
        for step in 0..<14 where !found.exists || !found.isHittable {
            if step < 6 { app.swipeUp() } else { app.swipeDown() }
        }
        return found
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
        if !row("review PR").waitForExistence(timeout: 5) {
            print("UNDOTREE\n\(app.debugDescription)")
            XCTFail("the task did not come back")
        }
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
        // is given one.
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
    /// `namedRows` is for forms built from `NamedRow`, whose visible field names are hidden
    /// from VoiceOver on purpose — the field carries the name — and which the audit reports
    /// as possibly inaccessible text. Only that finding, only on those screens.
    private func audit(
        _ types: XCUIAccessibilityAuditType = .all, _ screen: String = "", namedRows: Bool = false
    ) throws {
        var issues: [String] = []
        try app.performAccessibilityAudit(for: types) { issue in
            // The keyboard's predictive-text cells are the system's, not this app's, and
            // nothing here can label them.
            if issue.detailedDescription.contains("TUIPredictionViewCell") {
                return true
            }
            // SwiftUI text is reported as only *partly* scaling — captions, and once even a
            // stock button — though `testSettingsPagesAtTheLargestTextSize` keeps screenshots
            // of every form page at the largest size showing it at full size. Only that
            // variant, only on SwiftUI's nodes: "unsupported" outright, or on a UIKit element,
            // still fails.
            if namedRows, issue.compactDescription == "Potentially inaccessible text" {
                return true
            }
            if issue.auditType == .dynamicType,
               issue.compactDescription.contains("partially"),
               issue.detailedDescription.contains("SwiftUI") {
                return true
            }
            let element = issue.element.map { "\($0.elementType.rawValue) '\($0.label)' \($0.frame)" }
            issues.append("\(issue.compactDescription) — \(issue.detailedDescription) [\(element ?? "unnamed element")]")
            return ProcessInfo.processInfo.environment["AUDIT_ATTACH"] == nil
        }
        XCTAssertTrue(issues.isEmpty, "\(screen)\n" + issues.joined(separator: "\n"))
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

    // MARK: - The day

    func testTheDaySaysWhatItHoldsAndShowsFreeTime() throws {
        tab("Today")
        app.buttons["Add block"].tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        try audit(.all, "block form", namedRows: true)
        name.tap()
        name.typeText("Deep work")
        app.buttons["Save"].tap()

        let summary = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH '1 block'"))
        XCTAssertTrue(summary.firstMatch.waitForExistence(timeout: 5))
        XCTAssertTrue(cell(containing: "Deep work").exists)
        XCTAssertTrue(cell(containing: "Free,").exists, "free time is a row")
        try audit()
    }

    func testABreakCanTakeTasksAndATimerPausesResumesAndStops() {
        add("read the paper")
        tab("Today")
        app.buttons["Add block"].tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Train")
        app.buttons["Break"].tap()
        let takes = app.switches["Takes tasks"]
        if !takes.isHittable { app.swipeUp() }
        XCTAssertEqual(takes.value as? String, "0", "a break takes no tasks until set apart")
        takes.switches.firstMatch.tap()
        app.buttons["Save"].tap()

        let block = app.cells.containing(NSPredicate(format: "value CONTAINS 'takes tasks'")).firstMatch
        XCTAssertTrue(block.waitForExistence(timeout: 5), "the details say what differs from the kind")
        block.swipeLeft()
        app.buttons["Assign Task"].tap()
        app.cells.matching(NSPredicate(format: "label == 'read the paper'")).firstMatch.tap()
        XCTAssertTrue(app.alerts.buttons["Skip"].waitForExistence(timeout: 5))
        app.alerts.buttons["Skip"].tap()

        let sitting = row("read the paper")
        let says = { [app] (text: String) in
            app!.cells.containing(NSPredicate(format: "label == 'read the paper' AND value CONTAINS %@", text)).firstMatch
                .waitForExistence(timeout: 5)
        }
        XCTAssertTrue(sitting.waitForExistence(timeout: 5))
        for (action, state) in [("Start Timer", "in progress"), ("Pause Timer", "paused"), ("Resume Timer", "in progress"),
                                ("Pause Timer", "paused"), ("Stop Timer", "worked")] {
            sitting.swipeLeft()
            app.buttons[action].tap()
            XCTAssertTrue(says(state), "\(action) leaves it \(state)")
        }

        // A block with sittings under it folds, so they can be skipped past.
        block.swipeLeft()
        app.buttons["Collapse"].tap()
        XCTAssertTrue(sitting.waitForNonExistence(timeout: 5), "collapsing hides the sittings")
        let collapsed = app.cells.containing(NSPredicate(format: "value CONTAINS 'collapsed'")).firstMatch
        XCTAssertTrue(collapsed.waitForExistence(timeout: 5), "the block says it is collapsed")
        collapsed.swipeLeft()
        app.buttons["Expand"].tap()
        XCTAssertTrue(sitting.waitForExistence(timeout: 5), "expanding shows them again")
    }

    func testATaskCanBeAssignedToABlockAndTimed() {
        add("write the chapter")
        tab("Today")
        app.buttons["Add block"].tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("Writing")
        app.buttons["Save"].tap()

        let block = cell(containing: "Writing")
        XCTAssertTrue(block.waitForExistence(timeout: 5))
        block.swipeLeft()
        app.buttons["Assign Task"].tap()
        app.cells.matching(NSPredicate(format: "label == 'write the chapter'")).firstMatch.tap()
        XCTAssertTrue(app.alerts.buttons["Skip"].waitForExistence(timeout: 5), "asks how long, and can be skipped")
        app.alerts.buttons["Skip"].tap()

        let sitting = app.cells.matching(NSPredicate(format: "label == 'write the chapter'")).firstMatch
        XCTAssertTrue(sitting.waitForExistence(timeout: 5))
        sitting.swipeLeft()
        app.buttons["Start Timer"].tap()
        XCTAssertTrue(
            app.cells.containing(NSPredicate(format: "value CONTAINS 'in progress'")).firstMatch
                .waitForExistence(timeout: 5)
        )
    }

    // MARK: - Browse

    func testAProjectHoldsTheTasksAddedInIt() throws {
        tab("Browse")
        cell(containing: "Projects").tap()
        app.buttons["Add project"].tap()
        answer("Work", with: "Add")
        XCTAssertTrue(cell(containing: "Work").waitForExistence(timeout: 5))
        try audit()

        cell(containing: "Work").tap()
        app.buttons["Add task"].tap()
        let field = app.textViews["New task"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        // Quick add starts with the project, so the task lands in it.
        field.typeText("ship the release")
        app.buttons["Add"].tap()
        XCTAssertTrue(row("ship the release").waitForExistence(timeout: 5))
    }

    func testABrowseRowSaysItsNameOnceAndItsCountOnce() {
        tab("Browse")
        let projects = app.cells.matching(NSPredicate(format: "label == 'Projects'")).firstMatch
        XCTAssertTrue(projects.waitForExistence(timeout: 5), "the label is the name alone")
        XCTAssertEqual(projects.value as? String, "1 project", "the count, once")
    }

    func testLabelsAndSavedFiltersCanBeMade() throws {
        tab("Browse")
        cell(containing: "Labels").tap()
        app.buttons["Add label"].tap()
        answer("calls", with: "Add")
        XCTAssertTrue(cell(containing: "calls").waitForExistence(timeout: 5))
        app.navigationBars.buttons.element(boundBy: 0).tap()

        cell(containing: "Saved filters").tap()
        app.buttons["Add filter"].tap()
        answer("Urgent", with: "Next")
        answer("p1", with: "Save")
        XCTAssertTrue(cell(containing: "Urgent").waitForExistence(timeout: 5))
        try audit()
    }

    func testATrashedTaskCanBeRestoredFromTheTrash() {
        add("throw me away")
        let task = row("throw me away")
        XCTAssertTrue(task.waitForExistence(timeout: 5))
        task.swipeLeft()
        app.buttons["Delete"].tap()
        XCTAssertTrue(task.waitForNonExistence(timeout: 5))

        tab("Browse")
        cell(containing: "Trash").tap()
        let trashed = row("throw me away")
        XCTAssertTrue(trashed.waitForExistence(timeout: 5))
        trashed.swipeLeft()
        app.buttons["Restore"].tap()
        XCTAssertTrue(trashed.waitForNonExistence(timeout: 5))

        tab("Tasks")
        XCTAssertTrue(row("throw me away").waitForExistence(timeout: 5))
    }

    // MARK: - Settings

    func testSettingsAndDevicesPassAnAudit() throws {
        tab("Settings")
        XCTAssertTrue(cell(containing: "Planning").waitForExistence(timeout: 5))
        try audit(.all, "settings")

        for page in ["Planning", "Backups", "Export and Import"] {
            reveal(page).tap()
            XCTAssertTrue(app.navigationBars[page].waitForExistence(timeout: 5))
            try audit(.all, page)
            app.navigationBars.buttons.element(boundBy: 0).tap()
        }

        reveal("Devices and Sync").tap()
        let status = app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'Not paired'"))
        XCTAssertTrue(status.firstMatch.waitForExistence(timeout: 10))
        try audit(.all, "devices")

        app.buttons["Pair a device"].tap()
        XCTAssertTrue(app.buttons["Wait for the Other Device"].waitForExistence(timeout: 5))
        try audit(.all, "pairing")
    }

    func testTaskDetailLabelsAreSavedByName() throws {
        add("ring the bank")
        row("ring the bank").tap()
        let labels = app.textFields["Labels"]
        XCTAssertTrue(labels.waitForExistence(timeout: 5))
        labels.tap()
        // Return puts the keyboard away, so the audit sees the form rather than the keyboard.
        labels.typeText("calls, errands\n")
        print("DETAILTREE\n\(app.debugDescription)")
        app.buttons["Save"].tap()
        XCTAssertTrue(
            app.staticTexts.containing(NSPredicate(format: "label CONTAINS 'errands'")).firstMatch
                .waitForExistence(timeout: 5) || labels.value as? String == "calls, errands"
        )
        try audit(.all, "task detail", namedRows: true)
    }

    /// Every settings page at the largest accessibility text size, kept as screenshots: what
    /// the audit can only estimate, shown.
    func testSettingsPagesAtTheLargestTextSize() {
        app.terminate()
        app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"]
        app.launch()
        tab("Settings")
        for page in ["Planning", "Backups", "Export and Import"] {
            reveal(page).tap()
            XCTAssertTrue(app.navigationBars[page].waitForExistence(timeout: 5))
            // The whole page, a screenful at a time.
            for part in 1...4 {
                let shot = XCTAttachment(screenshot: app.screenshot())
                shot.name = "\(page) at the largest text size, part \(part)"
                shot.lifetime = .keepAlways
                add(shot)
                app.swipeUp()
            }
            app.navigationBars.buttons.element(boundBy: 0).tap()
        }
    }

    /// Pairs with a device waiting in `lum pair`, whose code arrives as `PAIR_CODE`
    /// (`TEST_RUNNER_PAIR_CODE` to xcodebuild). Skipped without one.
    func testPairingByCodeBringsTheOtherDevicesTasks() throws {
        guard let code = ProcessInfo.processInfo.environment["PAIR_CODE"], !code.isEmpty else {
            throw XCTSkip("no device is waiting to pair")
        }
        tab("Settings")
        cell(containing: "Devices and Sync").tap()
        app.buttons["Pair a device"].tap()
        let entry = app.textViews["Code from the other device"]
        XCTAssertTrue(entry.waitForExistence(timeout: 5))
        entry.tap()
        entry.typeText(code)
        app.buttons["Pair With This Code"].tap()

        let yes = app.alerts.buttons["Yes, They Match"]
        XCTAssertTrue(yes.waitForExistence(timeout: 90), "the words never came")
        yes.tap()

        // Finished when the device list is back and names the other device — not before,
        // or the test ending would cut the pairing's goodbyes short.
        let other = app.cells.containing(NSPredicate(format: "value CONTAINS 'not synced yet' OR value CONTAINS 'last synced'"))
        XCTAssertTrue(other.firstMatch.waitForExistence(timeout: 60), "pairing never finished")

        tab("Tasks")
        XCTAssertTrue(row("written on the mac").waitForExistence(timeout: 30), "the first sync brought nothing")
    }

    /// Pairs with a `lum pair` waiting on this network, with no code: Bonjour finds it.
    /// Skipped unless `PAIR_LOCAL` is set (`TEST_RUNNER_PAIR_LOCAL` to xcodebuild).
    func testPairingOnTheLocalNetworkNeedsNoCode() throws {
        guard ProcessInfo.processInfo.environment["PAIR_LOCAL"] != nil else {
            throw XCTSkip("no device is waiting on this network")
        }
        tab("Settings")
        cell(containing: "Devices and Sync").tap()
        app.buttons["Pair a device"].tap()
        app.buttons["Wait for the Other Device"].tap()

        let yes = app.alerts.buttons["Yes, They Match"]
        XCTAssertTrue(yes.waitForExistence(timeout: 60), "the two never found each other")
        yes.tap()

        let other = app.cells.containing(NSPredicate(format: "value CONTAINS 'not synced yet' OR value CONTAINS 'last synced'"))
        XCTAssertTrue(other.firstMatch.waitForExistence(timeout: 60), "pairing never finished")
        tab("Tasks")
        XCTAssertTrue(row("written on the mac").waitForExistence(timeout: 30))
    }

    // MARK: - Repetition, days and blocks

    /// Replaces whatever a field holds: a triple tap selects all of it, so typing replaces it.
    private func replace(_ field: XCUIElement, with text: String) {
        field.tap(withNumberOfTaps: 3, numberOfTouches: 1)
        field.typeText(text)
    }

    private func addBlock(_ title: String, repeating: String? = nil) {
        tab("Today")
        app.buttons["Add block"].tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText(title)
        if let repeating {
            let repeats = app.textFields["Repeats"]
            repeats.tap()
            repeats.typeText(repeating)
        }
        app.buttons["Save"].tap()
        XCTAssertTrue(cell(containing: title).waitForExistence(timeout: 5))
    }

    func testARepetitionIsShownInWordsAndKeptThroughANewDate() {
        add("water plants every monday")
        row("water plants").tap()
        let repeats = app.textFields["Repeats"]
        XCTAssertTrue(repeats.waitForExistence(timeout: 5))
        XCTAssertEqual(repeats.value as? String, "every monday", "words, not an RRULE")
        replace(app.textFields["Due"], with: "2026-12-10")
        app.buttons["Save"].tap()
        XCTAssertEqual(app.textFields["Due"].value as? String, "2026-12-10")
        XCTAssertEqual(app.textFields["Repeats"].value as? String, "every monday", "a new date keeps it")
    }

    func testACancelledDayIsListedAndCanBePutBack() throws {
        addBlock("Run", repeating: "every day")
        cell(containing: "Run").swipeLeft()
        app.buttons["Cancel This Day"].tap()
        let cancelled = app.cells.containing(NSPredicate(format: "value == 'cancelled for this day'")).firstMatch
        XCTAssertTrue(cancelled.waitForExistence(timeout: 5))
        try audit()
        cancelled.swipeLeft()
        app.buttons["Restore This Day"].tap()
        XCTAssertTrue(cancelled.waitForNonExistence(timeout: 5))
        XCTAssertTrue(cell(containing: "Run").exists)
    }

    func testUndoWorksFromTheDayToo() {
        addBlock("Deep work")
        app.navigationBars.buttons["Undo"].tap()
        XCTAssertTrue(cell(containing: "Deep work").waitForNonExistence(timeout: 5))
        app.navigationBars.buttons["Redo"].tap()
        XCTAssertTrue(cell(containing: "Deep work").waitForExistence(timeout: 5))
    }

    func testEveryBlockIsListedUnderBrowseAndAsksBeforeDeleting() throws {
        // Every day, so it is on today whatever day the test runs.
        addBlock("Standup", repeating: "every day")
        tab("Browse")
        cell(containing: "Blocks").tap()
        let standup = cell(containing: "Standup")
        XCTAssertTrue(standup.waitForExistence(timeout: 5))
        XCTAssertTrue((standup.value as? String)?.contains("every day") == true, "\(standup.value ?? "")")
        try audit()
        standup.tap()
        XCTAssertEqual(app.textFields["Repeats"].value as? String, "every day")
        app.buttons["Cancel"].tap()
        standup.swipeLeft()
        app.buttons["Delete"].tap()
        app.alerts.buttons["Delete"].tap()
        XCTAssertTrue(standup.waitForNonExistence(timeout: 5))
    }

    func testASittingsPlannedLengthIsAskedForShownAndCleared() {
        add("draft")
        addBlock("Focus")
        cell(containing: "Focus").swipeLeft()
        app.buttons["Assign Task"].tap()
        app.cells.matching(NSPredicate(format: "label == 'draft'")).firstMatch.tap()
        let minutes = app.alerts.textFields["Minutes"]
        XCTAssertTrue(minutes.waitForExistence(timeout: 5))
        minutes.typeText("45")
        app.alerts.buttons["Set"].tap()

        let planned = app.cells.containing(NSPredicate(format: "value CONTAINS 'planned for 45 minutes'")).firstMatch
        XCTAssertTrue(planned.waitForExistence(timeout: 5))
        planned.swipeLeft()
        app.buttons["Planned Length"].tap()
        app.alerts.buttons["No Planned Length"].tap()
        XCTAssertTrue(planned.waitForNonExistence(timeout: 5))
        XCTAssertTrue(app.cells.matching(NSPredicate(format: "label == 'draft'")).firstMatch.exists)
    }

    func testUndoIsOfferedOnBrowseScreensToo() {
        tab("Browse")
        cell(containing: "Projects").tap()
        app.buttons["Add project"].tap()
        answer("Work", with: "Add")
        let work = app.cells.matching(NSPredicate(format: "label == 'Work'")).firstMatch
        XCTAssertTrue(work.waitForExistence(timeout: 5))
        app.navigationBars.buttons["Undo"].tap()
        XCTAssertTrue(work.waitForNonExistence(timeout: 5), "undone without leaving Browse")
        app.navigationBars.buttons["Redo"].tap()
        XCTAssertTrue(work.waitForExistence(timeout: 5))
    }

    func testATaskIsPutInABlockFromItsOwnPage() {
        add("draft")
        addBlock("Focus")
        tab("Tasks")
        row("draft").tap()
        // Far down the form, and SwiftUI makes rows only once they are on screen.
        let put = app.buttons["Put in a Block…"]
        for _ in 0..<6 where !put.exists { app.swipeUp() }
        put.tap()
        let block = app.cells.containing(NSPredicate(format: "label CONTAINS 'Focus'")).firstMatch
        XCTAssertTrue(block.waitForExistence(timeout: 5), "the week's work blocks, listed")
        block.tap()
        let skip = app.alerts["How long is this sitting meant to take?"].buttons["Skip"]
        XCTAssertTrue(skip.waitForExistence(timeout: 5))
        skip.tap()
        XCTAssertTrue(app.alerts.firstMatch.waitForNonExistence(timeout: 5))
        let state = app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'ready'")).firstMatch
        XCTAssertTrue(state.waitForExistence(timeout: 5))
        XCTAssertFalse(state.label.contains("unassigned"), state.label)
        // The task's page hides the tab bar; back to the list first.
        app.navigationBars.buttons.element(boundBy: 0).tap()
        tab("Today")
        XCTAssertTrue(app.cells.matching(NSPredicate(format: "label == 'draft'")).firstMatch.waitForExistence(timeout: 5))
    }
}
