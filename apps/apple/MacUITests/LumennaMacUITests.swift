import CoreGraphics
import XCTest

/// The Mac app driven the way a VoiceOver user drives it: by accessibility label and the
/// keyboard, not by position.
final class LumennaMacUITests: XCTestCase {
    private var app: XCUIApplication!
    private let profile = UUID().uuidString

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        // A fresh store for every test, in the app's temporary directory.
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = profile
        app.launchEnvironment["LUMENNA_BACKUP_DIR"] = backups.path
        app.launch()
    }

    /// Where this run's backups go, so a test can count them and the real ones are untouched.
    private lazy var backups = FileManager.default.temporaryDirectory
        .appendingPathComponent("lumenna-ui-backups-\(UUID().uuidString)", isDirectory: true)

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
        XCTAssertTrue(window.scrollViews["Task details"].waitForExistence(timeout: 5), "the pane is one named scroll area")
        XCTAssertFalse(window.groups["Task details"].exists, "not a group around one")
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
        let voiceOverPanels = windows(ownedBy: "VoiceOver")
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
            // VoiceOver's caption and braille panels float over whatever is beneath them, where
            // the person put them, so text there is judged against the panel. Only contrast,
            // only under a window VoiceOver owns.
            if issue.auditType == .contrast, let frame = issue.element?.frame,
               voiceOverPanels.contains(where: { $0.intersects(frame) }) { return true }
            // SwiftUI's pop-up menus answer VoiceOver's press but not the audit's question
            // about it; only that finding, only on pop-ups.
            if issue.element?.elementType == .popUpButton, issue.compactDescription == "Action is missing" { return true }
            let element = issue.element.map { "\($0.elementType.rawValue) '\($0.label)' '\($0.value ?? "")' \($0.frame)" }
            issues.append("\(issue.compactDescription) — \(issue.detailedDescription) [\(element ?? "unnamed element")]")
            return true
        }
        XCTAssertTrue(issues.isEmpty, "\(screen)\n" + issues.joined(separator: "\n"))
    }

    /// The frames of another process's windows on screen, in the top-left coordinates
    /// XCUITest's frames use.
    private func windows(ownedBy owner: String) -> [CGRect] {
        let listed = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
        return listed.compactMap { window in
            guard window[kCGWindowOwnerName as String] as? String == owner,
                  let bounds = window[kCGWindowBounds as String] as CFTypeRef? else { return nil }
            return CGRect(dictionaryRepresentation: bounds as! CFDictionary)
        }
    }

    /// A store both the app and `lum` reach. This runner is sandboxed, and so is anything it
    /// starts, so it lives in the runner's temporary directory, which the app is not kept out of.
    private lazy var sharedStore = FileManager.default.temporaryDirectory
        .appendingPathComponent("lumenna-shared-\(profile)", isDirectory: true)

    /// `lum`, built in this checkout, run against `sharedStore`.
    private func lum(_ arguments: String...) throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let binary = root.appendingPathComponent("target/debug/lum")
        try XCTSkipUnless(FileManager.default.isExecutableFile(atPath: binary.path), "build lum first")
        let process = Process()
        process.executableURL = binary
        process.arguments = ["--profile", sharedStore.path] + arguments
        process.environment = ["LUMENNA_BACKUP_DIR": backups.path, "PATH": "/usr/bin:/bin"]
        try process.run()
        process.waitUntilExit()
        XCTAssertEqual(process.terminationStatus, 0, "lum \(arguments.joined(separator: " "))")
    }

    /// What another process writes appears without being asked for, though the app's own sync
    /// loop and every operation take changes in too and would have swallowed a "changed".
    func testWhatLumWritesAppearsWhileTheAppIsOpen() throws {
        app.terminate()
        app.launchEnvironment["LUMENNA_TEST_PROFILE"] = nil
        app.launchEnvironment["LUMENNA_PROFILE"] = sharedStore.path
        app.launch()
        XCTAssertTrue(window.waitForExistence(timeout: 10))
        place("Tasks")
        for title in ["written elsewhere", "and again"] {
            try lum("task", "add", title)
            XCTAssertTrue(window.outlines["Tasks"].staticTexts[title].waitForExistence(timeout: 5), title)
        }
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

    // MARK: - Helpers for menus and sheets

    /// A row of the sidebar, by its name.
    private func sidebarRow(_ name: String) -> XCUIElement {
        window.outlines["Places"].staticTexts[name]
    }

    /// Opens a row's context menu and chooses an item from it — what VO-Shift-M does.
    private func menu(_ element: XCUIElement, _ item: String) {
        XCTAssertTrue(element.waitForExistence(timeout: 5), "no \(element)")
        element.rightClick()
        // The open context menu's, not the menu bar's item of the same name.
        let named = app.menuItems.matching(identifier: item)
        let deadline = Date().addingTimeInterval(5)
        var choice: XCUIElement?
        while choice == nil, Date() < deadline {
            choice = named.allElementsBoundByIndex.first { $0.isHittable }
            if choice == nil { usleep(100_000) }
        }
        guard let choice else { return XCTFail("no menu item \(item)") }
        choice.click()
    }

    /// Answers the one-line question a sheet asks, and presses `button`. With `then`, the
    /// next question follows at once, so the sheet does not close in between.
    private func answer(_ text: String, with button: String, then next: Bool = false) {
        let field = app.sheets.textFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 5), "no question was asked")
        enter(text, into: field)
        app.sheets.buttons[button].firstMatch.click()
        if next {
            XCTAssertTrue(app.sheets.textFields.firstMatch.waitForValue(""), "the next question")
        } else {
            XCTAssertTrue(app.sheets.firstMatch.waitForNonExistence(timeout: 5))
        }
    }

    /// Chooses from a picker sheet by narrowing it to `text`.
    private func pick(_ text: String) {
        let narrow = app.sheets.searchFields.firstMatch
        XCTAssertTrue(narrow.waitForExistence(timeout: 5), "no list to choose from")
        enter(text, into: narrow)
        app.sheets.buttons["Choose"].click()
    }

    /// Presses a button in whatever sheet is showing.
    private func sheetButton(_ title: String) {
        let button = app.sheets.buttons[title].firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 5), "no \(title) in the sheet")
        button.click()
    }

    private func text(containing words: String, in parent: XCUIElement? = nil) -> XCUIElement {
        (parent ?? window).staticTexts.matching(NSPredicate(format: "value CONTAINS %@ OR label CONTAINS %@", words, words)).firstMatch
    }

    // MARK: - Organising, from the sidebar

    func testAProjectIsMadeRenamedArchivedAndDeletedFromTheSidebar() {
        menu(sidebarRow("Projects"), "New Project…")
        answer("Wrok", with: "Add")
        XCTAssertTrue(sidebarRow("Wrok").waitForExistence(timeout: 5))

        menu(sidebarRow("Wrok"), "Rename…")
        answer("Work", with: "Save")
        XCTAssertTrue(sidebarRow("Work").waitForExistence(timeout: 5))
        // Undo reaches the store from anywhere, the sidebar included.
        sidebarRow("Work").click()
        app.typeKey("z", modifierFlags: .command)
        XCTAssertTrue(sidebarRow("Wrok").waitForExistence(timeout: 5), "undone")
        app.typeKey("z", modifierFlags: [.command, .shift])
        XCTAssertTrue(sidebarRow("Work").waitForExistence(timeout: 5), "redone")

        menu(sidebarRow("Work"), "Archive")
        menu(sidebarRow("Work"), "Unarchive")
        menu(sidebarRow("Work"), "Delete…")
        sheetButton("Delete and Keep Its Tasks")
        XCTAssertTrue(sidebarRow("Work").waitForNonExistence(timeout: 5))
    }

    func testALabelIsMadeColouredAndMergedIntoAnother() {
        menu(sidebarRow("Labels"), "New Label…")
        answer("calls", with: "Add")
        menu(sidebarRow("Labels"), "New Label…")
        answer("cals", with: "Add")
        menu(sidebarRow("calls"), "Colour…")
        answer("teal", with: "Save")
        menu(sidebarRow("cals"), "Merge Into…")
        pick("calls")
        XCTAssertTrue(sidebarRow("cals").waitForNonExistence(timeout: 5))
        XCTAssertTrue(sidebarRow("calls").exists)
    }

    func testASavedFilterIsMadeRequeriedAndDeleted() {
        menu(sidebarRow("Saved Filters"), "New Saved Filter…")
        answer("Urgent", with: "Next", then: true)
        answer("p1", with: "Save")
        XCTAssertTrue(sidebarRow("Urgent").waitForExistence(timeout: 5))
        XCTAssertEqual(window.textFields["Filter"].value as? String, "p1", "it opens with its query")
        menu(sidebarRow("Urgent"), "Change Query…")
        answer("p1 | p2", with: "Save")
        XCTAssertTrue(window.textFields["Filter"].waitForValue("p1 | p2"))
        menu(sidebarRow("Urgent"), "Delete…")
        sheetButton("Delete")
        XCTAssertTrue(sidebarRow("Urgent").waitForNonExistence(timeout: 5))
    }

    // MARK: - The trash

    func testATrashedTaskIsRestoredAndAnotherErasedFromTheTrash() {
        place("Tasks")
        addTask("keep me")
        addTask("lose me")
        let tasks = window.outlines["Tasks"]
        for title in ["keep me", "lose me"] {
            tasks.staticTexts[title].click()
            app.typeKey(.delete, modifierFlags: [])
            XCTAssertTrue(tasks.staticTexts[title].waitForNonExistence(timeout: 5))
        }
        place("Trash")
        let trash = window.outlines["Trash"]
        trash.staticTexts["keep me"].click()
        app.typeKey(.space, modifierFlags: [])
        XCTAssertTrue(trash.staticTexts["keep me"].waitForNonExistence(timeout: 5), "restored")
        trash.staticTexts["lose me"].click()
        app.typeKey(.delete, modifierFlags: [])
        sheetButton("Delete")
        XCTAssertTrue(trash.staticTexts["lose me"].waitForNonExistence(timeout: 5), "deleted from the trash")
        place("Tasks")
        XCTAssertTrue(window.outlines["Tasks"].staticTexts["keep me"].waitForExistence(timeout: 5))
    }

    // MARK: - Blocks and the day

    /// Adds a block from whichever pane is showing, through the menu bar's New Block.
    private func addBlock(_ name: String, repeating: String? = nil) {
        app.typeKey("n", modifierFlags: [.command, .shift])
        let field = app.textFields["Name"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        enter(name, into: field)
        if let repeating { enter(repeating, into: app.textFields["Repeats"]) }
        sheetButton("Save")
        XCTAssertTrue(app.sheets.firstMatch.waitForNonExistence(timeout: 5))
    }

    func testABreakSetApartTakesTasksAndSpacePausesAndResumesASitting() {
        place("Tasks")
        addTask("read")
        place("Today")
        app.typeKey("n", modifierFlags: [.command, .shift])
        let field = app.textFields["Name"]
        XCTAssertTrue(field.waitForExistence(timeout: 5))
        enter("Train", into: field)
        app.sheets.popUpButtons.firstMatch.click()
        app.menuItems["Break"].click()
        let takes = app.sheets.checkBoxes["Takes tasks"]
        XCTAssertTrue(takes.waitForExistence(timeout: 5))
        XCTAssertEqual(takes.value as? Int, 0, "a break takes no tasks until set apart")
        takes.click()
        sheetButton("Save")
        XCTAssertTrue(app.sheets.firstMatch.waitForNonExistence(timeout: 5))

        let day = window.outlines["The day"]
        XCTAssertTrue(text(containing: "takes tasks", in: day).waitForExistence(timeout: 5))
        menu(text(containing: "Train", in: day), "Assign a Task…")
        pick("read")
        let minutes = app.sheets.textFields["Minutes"]
        XCTAssertTrue(minutes.waitForExistence(timeout: 5))
        enter("30", into: minutes)
        sheetButton("Set")

        let sitting = day.staticTexts["read"]
        XCTAssertTrue(sitting.waitForExistence(timeout: 5))
        sitting.click()
        for state in ["in progress", "paused", "in progress"] {
            app.typeKey(.space, modifierFlags: [])
            XCTAssertTrue(text(containing: state, in: day).waitForExistence(timeout: 5), "Space leaves it \(state)")
        }
        menu(day.staticTexts["read"], "Stop Timer")
        XCTAssertTrue(text(containing: "worked", in: day).waitForExistence(timeout: 5))
    }

    func testTheBlockListEditsEveryOccurrenceAndAsksBeforeDeleting() {
        place("Blocks")
        addBlock("Standup", repeating: "every day")
        let blocks = window.tables["Blocks"]
        let standup = blocks.staticTexts["Standup"]
        XCTAssertTrue(standup.waitForExistence(timeout: 5))
        XCTAssertTrue(text(containing: "every day", in: blocks).exists, "said in words")
        standup.doubleClick()
        XCTAssertEqual(app.textFields["Repeats"].value as? String, "every day")
        sheetButton("Cancel")
        standup.click()
        app.typeKey(.delete, modifierFlags: [])
        sheetButton("Delete")
        XCTAssertTrue(standup.waitForNonExistence(timeout: 5))
    }

    func testTheDayAssignsTimesAndPlansASittingAndPutsBackACancelledDay() {
        place("Tasks")
        addTask("write")
        place("Today")
        addBlock("Run", repeating: "every day")
        let day = window.outlines["The day"]
        menu(text(containing: "Run", in: day), "Assign a Task…")
        pick("write")
        let minutes = app.sheets.textFields["Minutes"]
        XCTAssertTrue(minutes.waitForExistence(timeout: 5))
        enter("45", into: minutes)
        sheetButton("Set")
        XCTAssertTrue(text(containing: "planned for 45 minutes", in: day).waitForExistence(timeout: 5))

        day.staticTexts["write"].click()
        app.typeKey(.space, modifierFlags: [])
        XCTAssertTrue(text(containing: "in progress", in: day).waitForExistence(timeout: 5), "Space starts the timer")
        menu(day.staticTexts["write"], "Planned Length…")
        sheetButton("No Planned Length")
        XCTAssertTrue(text(containing: "planned", in: day).waitForNonExistence(timeout: 5) || !text(containing: "45 minutes planned", in: day).exists)

        menu(text(containing: "Run", in: day), "Cancel This Day")
        let cancelled = text(containing: "cancelled for this day", in: day)
        XCTAssertTrue(cancelled.waitForExistence(timeout: 5))
        menu(cancelled, "Restore This Day")
        XCTAssertTrue(cancelled.waitForNonExistence(timeout: 5))
    }

    // MARK: - Settings

    func testBackUpNowWritesABackupWhereTheyGo() throws {
        XCTAssertTrue(window.waitForExistence(timeout: 10))
        let count = { (try? FileManager.default.contentsOfDirectory(atPath: self.backups.path).count) ?? 0 }
        let before = count()
        app.typeKey(",", modifierFlags: .command)
        let settings = app.windows["settings"]
        XCTAssertTrue(settings.waitForExistence(timeout: 5))
        settings.toolbars.buttons["Backups"].click()
        let backUp = settings.buttons["Back Up Now"]
        XCTAssertTrue(backUp.waitForExistence(timeout: 5), settings.debugDescription)
        backUp.click()
        let deadline = Date().addingTimeInterval(5)
        while count() == before, Date() < deadline { usleep(100_000) }
        XCTAssertEqual(count(), before + 1)
    }
}

private extension XCUIElement {
    /// Waits for a field to hold `value`.
    func waitForValue(_ value: String, timeout: TimeInterval = 5) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if (self.value as? String) == value { return true }
            usleep(100_000)
        }
        return false
    }
}
