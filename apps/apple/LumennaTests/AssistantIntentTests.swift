import AppIntents
import LumennaApp
import XCTest
@testable import Lumenna

/// A parameter of an intent, of whatever type, for what it says about itself.
private protocol Declared {
    var isOptional: Bool { get }
}

extension IntentParameter: Declared {}

/// Every assistant operation is an App Intent, worded as the core words it: App Intents' words
/// must be literals for the build to read, so this is what keeps them the table's
/// (`assistantOperations()`), and what Siri, Shortcuts and Apple Intelligence offer what
/// `lum mcp` does.
final class AssistantIntentTests: XCTestCase {
    private let operations = assistantOperations()

    private func intent(_ id: String) -> (any OperationIntent.Type)? {
        assistantIntents.first { $0.persistentIdentifier == id }
    }

    /// An intent's parameters by name, and whether each may be left out.
    private func parameters(of intent: any OperationIntent.Type) -> [String: Bool] {
        var found: [String: Bool] = [:]
        for child in Mirror(reflecting: intent.init()).children {
            guard let label = child.label, let parameter = child.value as? Declared else { continue }
            found[String(label.dropFirst())] = parameter.isOptional
        }
        return found
    }

    func testEveryOperationIsAnIntentInTheTablesOrderAndNoIntentIsAnOperationTheTableLacks() {
        XCTAssertEqual(assistantIntents.map { $0.persistentIdentifier }, operations.map(\.id))
    }

    func testEveryIntentIsTitledDescribedAndFoundByTheTablesWords() throws {
        for operation in operations {
            let intent = try XCTUnwrap(intent(operation.id), "no intent for \(operation.id)")
            XCTAssertEqual(intent.title.key, operation.title, operation.id)
            let description = try XCTUnwrap(intent.description, operation.id)
            XCTAssertEqual(description.descriptionText.key, operation.description, operation.id)
            XCTAssertEqual(description.searchKeywords.map(\.key), operation.keywords, operation.id)
        }
    }

    func testIntentsShareACategoryExactlyWhereTheirOperationsDo() throws {
        for first in operations {
            for second in operations {
                let one = try XCTUnwrap(intent(first.id)?.description?.categoryName?.key, first.id)
                let other = try XCTUnwrap(intent(second.id)?.description?.categoryName?.key, second.id)
                XCTAssertEqual(one == other, first.category == second.category, "\(first.id) and \(second.id)")
            }
        }
    }

    /// Asking first is the intent's own question, and a proposal is the one the core makes as
    /// it applies (`ApplyReplanIntent`, `ApplyDayPlanIntent`), so neither is a parameter.
    func testEveryParameterIsTakenByNameAndWhatTheTableRequiresTheIntentRequires() throws {
        for operation in operations {
            let intent = try XCTUnwrap(intent(operation.id), operation.id)
            let taken = parameters(of: intent)
            let given = operation.parameters.filter { $0.name != "confirm" && $0.name != "proposal" }
            for parameter in given {
                let optional = try XCTUnwrap(taken[parameter.name], "\(operation.id) takes no \(parameter.name)")
                // Accepting a proposal's changes may be left at all of them, said before applying.
                if !parameter.optional && parameter.name != "accepted" {
                    XCTAssertFalse(optional, "\(operation.id)'s \(parameter.name) is required")
                }
            }
            // Apply a Day Plan names the day it proposes for, as Propose a Plan for a Day does.
            let extra = Set(taken.keys).subtracting(given.map(\.name)).subtracting(operation.id == "apply_day_plan" ? ["date"] : [])
            XCTAssertEqual(extra, [], "\(operation.id) takes what the table does not")
        }
    }
}
