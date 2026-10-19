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
    var judgesAPadRowNotItsText: Bool {
        guard UIDevice.current.userInterfaceIdiom == .pad, auditType == .contrast,
              detailedDescription.contains("SwiftUI"), let element, element.elementType == .staticText
        else { return false }
        return element.frame.minX == 0 && element.frame.width >= 300
    }
}
