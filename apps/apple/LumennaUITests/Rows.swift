import XCTest

extension XCUIApplication {
    /// A list's row by its title: a SwiftUI list's row is a button in its cell, named by the
    /// title, where a UIKit list's was the cell itself.
    func row(_ title: String) -> XCUIElement {
        descendants(matching: .any).matching(NSPredicate(
            format: "label == %@ AND (elementType == %d OR elementType == %d)", title,
            XCUIElement.ElementType.cell.rawValue, XCUIElement.ElementType.button.rawValue
        )).firstMatch
    }
}


extension XCUIAccessibilityAuditIssue {
    /// A contrast finding the audit makes of a SwiftUI text on the iPad by its whole row: the
    /// element it reports starts at the column's edge and is the row's width, where the text is
    /// a short line inside it. The task list's count failed so in the full text colour, pure
    /// black on white, which cannot fail, with nothing over it; the same text passes on the
    /// iPhone.
    /// A navigation bar's own title, found clipped: cut short where a record's name makes it
    /// long ("Reminders for Deep work"), or where the bar's buttons leave it little room, as
    /// the iPad's narrow column does once a list's bar holds Undo, Redo, New and Place Actions.
    /// The bar's to shorten, and UIKit shows it whole in the large content viewer at the
    /// accessibility sizes. Only clipping, only the title of a bar it names.
    func clipsABarsOwnTitle(in app: XCUIApplication) -> Bool {
        guard auditType == .textClipped, let element, !element.label.isEmpty else { return false }
        let bar = app.navigationBars.matching(identifier: element.label).firstMatch
        return bar.exists && bar.frame.contains(element.frame)
    }

    var judgesAPadRowNotItsText: Bool {
        guard UIDevice.current.userInterfaceIdiom == .pad, auditType == .contrast,
              detailedDescription.contains("SwiftUI"), let element, element.elementType == .staticText
        else { return false }
        return element.frame.minX == 0 && element.frame.width >= 300
    }
}
