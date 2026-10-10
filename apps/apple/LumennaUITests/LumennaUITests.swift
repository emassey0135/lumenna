import UIKit
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

    private var pad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    /// A tab on iPhone; on iPad, the sidebar's place of that name. Browse is the iPhone's way
    /// to projects, labels, filters, blocks and the trash, which the iPad's sidebar lists
    /// itself, so there it is nowhere to go (`browse`).
    private func tab(_ name: String) {
        if !pad {
            app.tabBars.buttons[name].tap()
            return
        }
        if name == "Browse" {
            // The sidebar is what lists projects, blocks and the trash: shown, so the test's
            // next tap finds them there.
            let shown = app.buttons["Hide Sidebar"].firstMatch
            if !(shown.exists && shown.isHittable) { app.buttons["Show Sidebar"].firstMatch.tap() }
            return
        }
        // A hidden sidebar's rows still exist, off the screen: only one that can be tapped will do.
        let place = app.cells.matching(NSPredicate(format: "label == %@", name)).firstMatch
        if !(place.waitForExistence(timeout: 2) && place.isHittable) { app.buttons["Show Sidebar"].firstMatch.tap() }
        place.tap()
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
        // Down the list first, then back up for a row above. On iPad over the list's column:
        // the middle of the screen is the page open beside it.
        let list = pad ? app.coordinate(withNormalizedOffset: CGVector(dx: 0.2, dy: 0.6)) : nil
        for step in 0..<14 where !found.exists || !found.isHittable {
            if let list {
                list.press(forDuration: 0.05, thenDragTo: list.withOffset(CGVector(dx: 0, dy: step < 6 ? -300 : 300)))
            } else if step < 6 { app.swipeUp() } else { app.swipeDown() }
        }
        return found
    }

    /// Shows a row's swipe actions, as VoiceOver lists them.
    private func reveal(actionsOf row: XCUIElement, leading: Bool = false) {
        if leading { row.swipeRight() } else { row.swipeLeft() }
    }

    /// Opens a row's long-press menu, which holds every one of its actions, and chooses one.
    private func choose(_ action: String, fromMenuOf row: XCUIElement) {
        row.press(forDuration: 1.2)
        let item = app.buttons[action].firstMatch
        XCTAssertTrue(item.waitForExistence(timeout: 5), "the menu offers \(action)")
        item.tap()
    }

    /// Presses an alert's button. On iPad the first tap on an alert whose text field has had
    /// nothing typed into it only ends editing, and the button needs a second; the alert going
    /// is what says it was pressed.
    private func press(alertButton name: String) {
        let alert = app.alerts.firstMatch
        for _ in 0..<2 where alert.exists {
            alert.buttons[name].tap()
            _ = alert.waitForNonExistence(timeout: 1)
        }
    }

    /// Back to the list a page was opened from. On iPad the list is still beside the page,
    /// and the first bar button there is the sidebar's.
    private func goBack() {
        if !pad { app.navigationBars.buttons.element(boundBy: 0).tap() }
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
        // The core works "today" out in the phone's own time zone; in UTC it is already
        // tomorrow every evening in the Americas, and the row would say "due yesterday".
        let cell = row("water the plants")
        XCTAssertTrue(cell.waitForExistence(timeout: 5))
        XCTAssertTrue(
            (cell.value as? String ?? "").contains("due today"),
            "the row says \(cell.value ?? "nothing")"
        )
    }

    func testCompletingRemovesTheTaskAndUndoBringsItBack() {
        add("review PR")
        let cell = row("review PR")
        XCTAssertTrue(cell.waitForExistence(timeout: 5))

        reveal(actionsOf: cell, leading: true)
        app.buttons["Mark Done"].tap()
        XCTAssertTrue(cell.waitForNonExistence(timeout: 5))

        app.buttons["Undo"].tap()
        if !row("review PR").waitForExistence(timeout: 5) {
            print("UNDOTREE\n\(app.debugDescription)")
            XCTFail("the task did not come back")
        }
    }

    func testATasksLongPressMenuHoldsEveryActionItHasAndRunsThem() {
        add("water the plants")
        let task = row("water the plants")
        XCTAssertTrue(task.waitForExistence(timeout: 5))
        task.press(forDuration: 1.2)
        for action in ["Mark Done", "Edit Details", "Put in a Block", "Move to Project", "Make Subtask Of", "Wait For", "Move to Trash"] {
            XCTAssertTrue(app.buttons[action].firstMatch.waitForExistence(timeout: 5), "the menu offers \(action)")
        }
        app.buttons["Mark Done"].firstMatch.tap()
        XCTAssertTrue(task.waitForNonExistence(timeout: 5), "done from the menu, so no longer listed")
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
        goBack()
        XCTAssertTrue(row("draft the essay").waitForExistence(timeout: 5))
    }

    func testATasksTitleIsOneLineAndReturnSavesIt() {
        add("draft")
        row("draft").tap()
        let title = app.textFields["Title"]
        XCTAssertTrue(title.waitForExistence(timeout: 5))
        title.doubleTap()
        // Return saves rather than starting a second line.
        title.typeText("draft the essay\n")
        XCTAssertEqual(title.value as? String, "draft the essay")
        goBack()
        XCTAssertTrue(row("draft the essay").waitForExistence(timeout: 5))
    }

    func testATasksProjectIsChosenFromTheProjects() throws {
        try XCTSkipIf(pad, "the iPad makes projects from its sidebar (SidebarUITests)")
        tab("Browse")
        cell(containing: "Projects").tap()
        app.buttons["Add project"].tap()
        answer("Work", with: "Add")
        XCTAssertTrue(cell(containing: "Work").waitForExistence(timeout: 5))
        tab("Tasks")
        add("draft")
        row("draft").tap()
        let project = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Project'")).firstMatch
        XCTAssertTrue(project.waitForExistence(timeout: 5), "no Project in:\n\(app.debugDescription)")
        project.tap()
        // The projects, in a list of their own: the Inbox and Work.
        let work = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Work'")).firstMatch
        XCTAssertTrue(work.waitForExistence(timeout: 5), "no Work in:\n\(app.debugDescription)")
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Inbox'")).firstMatch.exists)
        try audit(.all, "project list")
        work.tap()
        XCTAssertTrue(project.waitForExistence(timeout: 5))
        app.buttons["Save"].tap()
        // Saved: the task's page, opened again, has it in Work.
        goBack()
        row("draft").tap()
        XCTAssertTrue(project.waitForExistence(timeout: 5))
        let said = "\(project.label) \(project.value as? String ?? "")"
        XCTAssertTrue(said.contains("Work"), said)
    }

    /// Runs the audit and fails once with every issue it found, each with the element it
    /// objects to. Left to itself the audit stops at the first, which hides the rest.
    /// `namedRows` is for forms built from `NamedRow` or `NamedDatePicker`, whose visible field names are hidden
    /// from VoiceOver on purpose — the field carries the name — and which the audit reports
    /// as possibly inaccessible text. Only that finding, only on those screens.
    /// Whether a scroll view's edge cuts through `frame`, part inside and part outside, or it
    /// has scrolled under a navigation bar, which blurs it.
    private func cutByAScrollEdge(_ frame: CGRect) -> Bool {
        let scrolling = app.collectionViews.allElementsBoundByIndex + app.scrollViews.allElementsBoundByIndex
            + app.tables.allElementsBoundByIndex
        let cut = scrolling.contains { view in
            let visible = view.frame.intersection(frame)
            return !visible.isNull && visible.height > 0 && visible.height < frame.height - 1
        }
        let underABar = app.navigationBars.allElementsBoundByIndex.contains { bar in
            let under = bar.frame.intersection(frame)
            return !under.isNull && under.height > 0
        }
        return cut || underABar
    }

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
            // Text a scrolling form's edge cuts through, half drawn, or scrolled under the bar:
            // the audit measures it against whatever lies outside the form — on iPad the dimmed
            // page around a sheet — or the bar's blur. Only contrast, only an element cut that
            // way: the same text shown whole is judged elsewhere (the settings pages' captions).
            if issue.auditType == .contrast, let element = issue.element, self.cutByAScrollEdge(element.frame) {
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
            let finding = "\(issue.compactDescription) — \(issue.detailedDescription) [\(element ?? "unnamed element")]"
            issues.append(finding)
            // Printed as well: with AUDIT_ATTACH the audit fails the test itself, before the
            // assertion below can say which element it was.
            print("AUDIT \(screen): \(finding)")
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
        // The iPad keyboard's suggestion bar is the system's, and the audit finds unnamed text
        // in it; the iPhone's is excused by name (TUIPredictionViewCell). Hidden, the sheet
        // is what is judged.
        if pad {
            let hide = app.keyboards.buttons.matching(NSPredicate(format: "label CONTAINS[c] 'keyboard'")).firstMatch
            if hide.exists { hide.tap() }
        }
        try audit()
    }

    // MARK: - The day

    func testGoToDayTakesATypedDayAndSaysOneItCannotRead() throws {
        tab("Today")
        app.buttons["Go to Day"].tap()
        let field = app.textViews["Day"]
        XCTAssertTrue(field.waitForExistence(timeout: 5), "no Day field in:\n\(app.debugDescription)")
        // Contrast and touch targets only: the system's own inline calendar fails the Dynamic
        // Type and text checks by itself, its day numbers fixed in size.
        try audit([.contrast, .hitRegion], "go to day")
        field.tap()
        field.typeText("someday soon\n")
        // Not a day: the sheet stays, saying why.
        XCTAssertTrue(field.exists)
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format: "label CONTAINS[c] 'someday soon'")).firstMatch.waitForExistence(timeout: 5))
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 20) + "12 October\n")
        XCTAssertTrue(app.navigationBars.matching(NSPredicate(format: "identifier CONTAINS %@ OR identifier CONTAINS %@", "12 October", "October 12")).firstMatch.waitForExistence(timeout: 5),
                      "the day typed, shown:\n\(app.debugDescription)")
    }

    /// The start a new block today opens at: the next whole hour, never before the day starts
    /// (8 AM in a new store) and never past 11 PM, as the device's clock shows it.
    private func nextHour(after now: Date) -> String {
        let hour = max(8, min(Calendar.current.component(.hour, from: now) + 1, 23))
        let time = Calendar.current.date(bySettingHour: hour, minute: 0, second: 0, of: now) ?? now
        return time.formatted(date: .omitted, time: .shortened).replacingOccurrences(of: "\u{202F}", with: " ")
    }

    func testANewBlockStartsAtTheNextHour() {
        tab("Today")
        let before = Date.now
        app.buttons["Add block"].tap()
        let picker = app.datePickers["Starts at"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 5))
        let shown = (picker.descendants(matching: .any).firstMatch.value as? String)?
            .replacingOccurrences(of: "\u{202F}", with: " ") ?? ""
        // Either side of the hour, should it turn while the form opens.
        XCTAssertTrue([nextHour(after: before), nextHour(after: .now)].contains(shown), "starts at \(shown)")
    }

    func testAnotherDayAsksTheStartAgainUntilTheStartIsSet() {
        tab("Today")
        app.buttons["Add block"].tap()
        let day = app.datePickers["Day"].firstMatch
        XCTAssertTrue(day.waitForExistence(timeout: 5))
        let start = app.datePickers["Starts at"].firstMatch
        func shown() -> String {
            (start.descendants(matching: .any).firstMatch.value as? String)?.replacingOccurrences(of: "\u{202F}", with: " ") ?? ""
        }
        // Another day of this month, so the calendar shows it without paging: the 1st, or the
        // 2nd when today is the 1st.
        let calendar = Calendar.current
        var parts = calendar.dateComponents([.year, .month, .day], from: .now)
        parts.day = parts.day == 1 ? 2 : 1
        let other = calendar.date(from: parts) ?? .now
        day.tap()
        let choice = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", other.formatted(.dateTime.month(.wide).day()))).firstMatch
        XCTAssertTrue(choice.waitForExistence(timeout: 5), app.debugDescription)
        choice.tap()
        app.navigationBars["New Block"].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertTrue(choice.waitForNonExistence(timeout: 5), "the calendar stayed open")
        // Another day starts when the day starts: 8 AM in a new store.
        XCTAssertEqual(shown(), "8:00 AM", "another day's start")

        // Set by the person, it stays whatever day is chosen.
        setStart((hour: "3", minute: "00", period: "PM"))
        day.tap()
        let today = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", Date.now.formatted(.dateTime.month(.wide).day()))).firstMatch
        XCTAssertTrue(today.waitForExistence(timeout: 5))
        today.tap()
        app.navigationBars["New Block"].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertTrue(today.waitForNonExistence(timeout: 5))
        XCTAssertEqual(shown(), "3:00 PM", "the start the person set")
    }

    func testTheDaySaysWhatItHoldsAndShowsFreeTime() throws {
        tab("Today")
        app.buttons["Add block"].tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        try audit(.all, "block form", namedRows: true)
        name.tap()
        name.typeText("Deep work")
        app.buttons["Save"].tap()

        let summary = app.staticTexts.containing(NSPredicate(format: "label BEGINSWITH 'Today. 1 block'"))
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
        // Its own start, not the form's next hour: the same whenever the test runs.
        setStart((hour: "11", minute: "00", period: "PM"))
        app.buttons["Break"].tap()
        let takes = app.switches["Takes tasks"]
        if !takes.isHittable { app.swipeUp() }
        // Still for a moment first: a tap while the form still glides after the swipe only
        // stops it, and on a runner the break was saved taking no tasks.
        var last = CGRect.null
        for _ in 0..<20 where takes.frame != last {
            last = takes.frame
            usleep(150_000)
        }
        XCTAssertEqual(takes.value as? String, "0", "a break takes no tasks until set apart")
        takes.switches.firstMatch.tap()
        let on = NSPredicate(format: "value == '1'")
        XCTAssertEqual(XCTWaiter.wait(for: [expectation(for: on, evaluatedWith: takes)], timeout: 3), .completed,
                       "Takes tasks was not turned on")
        app.buttons["Save"].tap()

        let block = app.cells.containing(NSPredicate(format: "value CONTAINS 'takes tasks'")).firstMatch
        XCTAssertTrue(block.waitForExistence(timeout: 5), "the details say what differs from the kind")
        reveal(actionsOf: block)
        app.buttons["Assign a Task"].tap()
        app.cells.matching(NSPredicate(format: "label == 'read the paper'")).firstMatch.tap()
        XCTAssertTrue(app.alerts["Planned Length"].waitForExistence(timeout: 5))
        press(alertButton: "Save")

        let sitting = row("read the paper")
        let says = { [app] (text: String) in
            app!.cells.containing(NSPredicate(format: "label == 'read the paper' AND value CONTAINS %@", text)).firstMatch
                .waitForExistence(timeout: 5)
        }
        XCTAssertTrue(sitting.waitForExistence(timeout: 5))
        for (action, state) in [("Start Timer", "in progress"), ("Pause Timer", "paused"), ("Resume Timer", "in progress"),
                                ("Pause Timer", "paused"), ("Stop Timer", "worked")] {
            reveal(actionsOf: sitting)
            app.buttons[action].tap()
            XCTAssertTrue(says(state), "\(action) leaves it \(state)")
        }

        // A block with sittings under it folds, so they can be skipped past.
        reveal(actionsOf: block)
        app.buttons["Collapse"].tap()
        XCTAssertTrue(sitting.waitForNonExistence(timeout: 5), "collapsing hides the sittings")
        let collapsed = app.cells.containing(NSPredicate(format: "value CONTAINS 'collapsed'")).firstMatch
        XCTAssertTrue(collapsed.waitForExistence(timeout: 5), "the block says it is collapsed")
        reveal(actionsOf: collapsed)
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
        // Its own start, not the form's next hour: the same whenever the test runs.
        setStart((hour: "11", minute: "00", period: "PM"))
        app.buttons["Save"].tap()

        let block = cell(containing: "Writing")
        XCTAssertTrue(block.waitForExistence(timeout: 5))
        reveal(actionsOf: block)
        app.buttons["Assign a Task"].tap()
        app.cells.matching(NSPredicate(format: "label == 'write the chapter'")).firstMatch.tap()
        XCTAssertTrue(app.alerts["Planned Length"].waitForExistence(timeout: 5), "asks how long, and can be left empty")
        press(alertButton: "Save")

        let sitting = app.cells.matching(NSPredicate(format: "label == 'write the chapter'")).firstMatch
        XCTAssertTrue(sitting.waitForExistence(timeout: 5))
        reveal(actionsOf: sitting)
        app.buttons["Start Timer"].tap()
        XCTAssertTrue(
            app.cells.containing(NSPredicate(format: "value CONTAINS 'in progress'")).firstMatch
                .waitForExistence(timeout: 5)
        )
    }

    // MARK: - Browse

    func testAProjectHoldsTheTasksAddedInIt() throws {
        try XCTSkipIf(pad, "the iPad makes projects, labels and filters from its sidebar (SidebarUITests)")
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

    func testABrowseRowSaysItsNameOnceAndItsCountOnce() throws {
        try XCTSkipIf(pad, "the iPad makes projects, labels and filters from its sidebar (SidebarUITests)")
        tab("Browse")
        let projects = app.cells.matching(NSPredicate(format: "label == 'Projects'")).firstMatch
        XCTAssertTrue(projects.waitForExistence(timeout: 5), "the label is the name alone")
        XCTAssertEqual(projects.value as? String, "1 project", "the count, once")
    }

    func testLabelsAndSavedFiltersCanBeMade() throws {
        try XCTSkipIf(pad, "the iPad makes projects, labels and filters from its sidebar (SidebarUITests)")
        tab("Browse")
        cell(containing: "Labels").tap()
        app.buttons["Add label"].tap()
        answer("calls", with: "Add")
        XCTAssertTrue(cell(containing: "calls").waitForExistence(timeout: 5))
        goBack()

        cell(containing: "Saved Filters").tap()
        app.buttons["Add filter"].tap()
        answer("Urgent", with: "Next")
        answer("p1", with: "Add")
        XCTAssertTrue(cell(containing: "Urgent").waitForExistence(timeout: 5))
        try audit()
    }

    func testATrashedTaskCanBeRestoredFromTheTrash() {
        add("throw me away")
        let task = row("throw me away")
        XCTAssertTrue(task.waitForExistence(timeout: 5))
        // A task has more swipe actions than a phone's row shows; Move to Trash, the last, is
        // the task page's too (the swipe actions' list itself is SwipeActionTests').
        task.tap()
        let trash = app.buttons["Move to Trash"]
        for _ in 0..<6 where !trash.isHittable { app.swipeUp() }
        // The page closes itself once its task is in the trash.
        trash.tap()
        XCTAssertTrue(task.waitForNonExistence(timeout: 5))

        tab("Browse")
        cell(containing: "Trash").tap()
        let trashed = row("throw me away")
        XCTAssertTrue(trashed.waitForExistence(timeout: 5))
        reveal(actionsOf: trashed)
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
            // Planning's times are `NamedDatePicker`s, whose visible names are hidden as a
            // `NamedRow`'s are: the picker carries the name.
            try audit(.all, page, namedRows: page == "Planning")
            goBack()
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
        try XCTSkipIf(pad, "on iPad the audit fails contrast on the form's About heading, which reads like the others (ROADMAP)")
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
            goBack()
        }
    }

    /// An empty code field asks for a code rather than reading the clipboard, and the
    /// system's Paste button beside it is how a copied code gets in.
    func testAnEmptyCodeAsksForOneAndThePasteButtonPastesIntoTheField() throws {
        // On the clipboard already: an empty field must not take it.
        let copied = "a code copied from somewhere else"
        UIPasteboard.general.string = copied
        tab("Settings")
        reveal("Devices and Sync").tap()
        app.buttons["Pair a device"].tap()
        let entry = app.textViews["Code from the other device"]
        XCTAssertTrue(entry.waitForExistence(timeout: 5))

        app.buttons["Pair Using This Code"].tap()
        let asked = app.alerts.staticTexts["Type or paste the code the other device shows."]
        XCTAssertTrue(asked.waitForExistence(timeout: 5), "no need_code: \(app.debugDescription)")
        XCTAssertEqual(entry.value as? String ?? "", "", "the clipboard was read")
        app.alerts.buttons["OK"].tap()
        XCTAssertTrue(app.alerts.firstMatch.waitForNonExistence(timeout: 5))

        // The system's Paste control, named as VoiceOver says it, beside the field.
        let paste = app.buttons["Paste"]
        XCTAssertTrue(paste.waitForExistence(timeout: 5), app.debugDescription)
        XCTAssertTrue(paste.isHittable)
        XCTAssertGreaterThanOrEqual(paste.frame.height, 44)
        XCTAssertEqual(paste.frame.midY, entry.frame.midY, accuracy: entry.frame.height, "not beside the field")
        let pasted = NSPredicate(format: "value == %@", copied)
        paste.tap()
        XCTAssertEqual(XCTWaiter.wait(for: [expectation(for: pasted, evaluatedWith: entry)], timeout: 5), .completed,
                       "Paste left the field holding \(entry.value ?? "nothing")")
        try audit(.all, "pairing, with a code pasted")
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
        // Pasted with the system's Paste button, as a code sent from the other device arrives.
        UIPasteboard.general.string = code
        app.buttons["Paste"].tap()
        let pasted = NSPredicate(format: "value == %@", code)
        XCTAssertEqual(XCTWaiter.wait(for: [expectation(for: pasted, evaluatedWith: entry)], timeout: 5), .completed,
                       "Paste left \(entry.value ?? "nothing")")
        app.buttons["Pair Using This Code"].tap()

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
    /// Replaces a field's text by deleting what is there first. Selecting it — a triple tap,
    /// or ⌘A — selected nothing on CI's simulators, and what was typed went in beside the old
    /// date.
    private func replace(_ field: XCUIElement, with text: String) {
        let old = (field.value as? String) ?? ""
        field.tap()
        for _ in old { field.typeKey(.rightArrow, modifierFlags: []) }
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: old.count) + text)
    }

    /// Sets the block form's Starts at through its wheels, and closes them.
    private func setStart(_ start: (hour: String, minute: String, period: String)) {
        let picker = app.datePickers["Starts at"].firstMatch
        XCTAssertTrue(picker.waitForExistence(timeout: 5), app.debugDescription)
        picker.tap()
        let wheels = app.pickerWheels
        XCTAssertTrue(wheels.firstMatch.waitForExistence(timeout: 5), app.debugDescription)
        wheels.element(boundBy: 0).adjust(toPickerWheelValue: start.hour)
        wheels.element(boundBy: 1).adjust(toPickerWheelValue: start.minute)
        // Hour, minute and AM/PM, as the simulator's US English clock has them.
        wheels.element(boundBy: 2).adjust(toPickerWheelValue: start.period)
        let set = "\(start.hour):\(start.minute) \(start.period)"
        // The time shown is on the control inside the named picker.
        let chosen = picker.descendants(matching: .any).firstMatch
        // The clock puts a narrow no-break space before AM and PM.
        let shown = (chosen.value as? String)?.replacingOccurrences(of: "\u{202F}", with: " ")
        XCTAssertEqual(shown, set, "the start was not set")
        // The wheels are in a popover, which the first tap outside only closes: on iPad
        // that tap was Save's, and the form stayed. Closed first, as a person would.
        // Its dismiss region lies over everything, so the tap goes on the form's own title bar:
        // away from the popover, and inside the sheet, where a tap outside it on iPad closed
        // the whole form.
        let outside = app.descendants(matching: .any)["PopoverDismissRegion"].firstMatch
        if outside.exists { app.navigationBars["New Block"].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap() }
        XCTAssertTrue(wheels.firstMatch.waitForNonExistence(timeout: 5), "the time's popover stayed open")
    }

    /// Adds a block today, starting where the form starts it (the next hour) unless `start`
    /// (11, 00, PM) says, for a test that needs the same block whenever it runs.
    private func addBlock(_ title: String, repeating: String? = nil, start: (hour: String, minute: String, period: String)? = nil) {
        tab("Today")
        app.buttons["Add block"].tap()
        let name = app.textFields["Name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText(title)
        if let start { setStart(start) }
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
        // Not one of the block's swipe actions, which are its primary ones: in its menu.
        choose("Cancel This Day", fromMenuOf: cell(containing: "Run"))
        let cancelled = app.cells.containing(NSPredicate(format: "value == 'cancelled for this day'")).firstMatch
        XCTAssertTrue(cancelled.waitForExistence(timeout: 5))
        try audit()
        reveal(actionsOf: cancelled)
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
        try XCTSkipIf(pad, "on iPad the block's Delete swipe action does not appear to the test yet (ROADMAP)")
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
        // The sheet's close button: "Cancel" on iPhone, an X called "Close" on iPad.
        app.buttons.matching(NSPredicate(format: "label IN {'Cancel', 'Close'}")).firstMatch.tap()
        reveal(actionsOf: standup)
        app.buttons["Delete Block"].tap()
        press(alertButton: "Delete Block")
        XCTAssertTrue(standup.waitForNonExistence(timeout: 5))
    }

    func testASittingsPlannedLengthIsAskedForShownAndCleared() {
        add("draft")
        addBlock("Focus")
        reveal(actionsOf: cell(containing: "Focus"))
        app.buttons["Assign a Task"].tap()
        app.cells.matching(NSPredicate(format: "label == 'draft'")).firstMatch.tap()
        let minutes = app.alerts.textFields["Planned length"]
        XCTAssertTrue(minutes.waitForExistence(timeout: 5))
        minutes.typeText("45m")
        press(alertButton: "Save")

        let planned = app.cells.containing(NSPredicate(format: "value CONTAINS 'planned for 45 minutes'")).firstMatch
        XCTAssertTrue(planned.waitForExistence(timeout: 5))
        choose("Planned Length", fromMenuOf: planned)
        // Emptied, for no planned length.
        let length = app.alerts.textFields["Planned length"]
        XCTAssertTrue(length.waitForExistence(timeout: 5))
        length.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 6))
        press(alertButton: "Save")
        XCTAssertTrue(planned.waitForNonExistence(timeout: 5))
        XCTAssertTrue(app.cells.matching(NSPredicate(format: "label == 'draft'")).firstMatch.exists)
    }

    func testUndoIsOfferedOnBrowseScreensToo() throws {
        try XCTSkipIf(pad, "the iPad makes projects, labels and filters from its sidebar (SidebarUITests)")
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
        // 11 PM for the default hour: Put in a Block leaves out blocks already over, and one
        // ending at midnight is never over on its own day.
        addBlock("Focus", start: (hour: "11", minute: "00", period: "PM"))
        tab("Tasks")
        row("draft").tap()
        // Far down the form, and SwiftUI makes rows only once they are on screen.
        let put = app.buttons["Put in a Block"]
        for _ in 0..<6 where !put.exists { app.swipeUp() }
        put.tap()
        let block = app.cells.containing(NSPredicate(format: "label CONTAINS 'Focus'")).firstMatch
        XCTAssertTrue(block.waitForExistence(timeout: 5), "the week's work blocks, listed")
        block.tap()
        XCTAssertTrue(app.alerts["Planned Length"].waitForExistence(timeout: 5))
        press(alertButton: "Save")
        XCTAssertTrue(app.alerts.firstMatch.waitForNonExistence(timeout: 5))
        let state = app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'ready'")).firstMatch
        XCTAssertTrue(state.waitForExistence(timeout: 5))
        XCTAssertFalse(state.label.contains("unassigned"), state.label)
        // The task's page hides the tab bar; back to the list first.
        goBack()
        tab("Today")
        XCTAssertTrue(app.cells.matching(NSPredicate(format: "label == 'draft'")).firstMatch.waitForExistence(timeout: 5))
    }
}
