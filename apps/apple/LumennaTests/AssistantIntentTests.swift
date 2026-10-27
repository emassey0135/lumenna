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

/// A task as Siri, Shortcuts and Spotlight have it: its search document, the core's, made as the
/// task stands, so its figures are never older than its line.
@MainActor
final class TaskEntityTests: XCTestCase {
    private var core: Core!
    private var store: AssistantStore!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
        store = AssistantStore(device: core.app.device)
    }

    private func added(_ line: String) throws -> String {
        try XCTUnwrap(store.entity(try XCTUnwrap(store.assist("add_task", ["text": .text(line)]).task))).id
    }

    func testTasksBecomeEntitiesInTheOrderNamedDoneOnesTooAndAnUnknownOneLeftOut() throws {
        let rent = try added("Pay the rent p1")
        _ = try store.assist("edit_task", ["id": .text(rent), "estimate": .text("30m")])
        let call = try added("Call the bank")
        _ = try store.assist("complete_task", ["id": .text(call)])

        let entities = store.entities([call, "0190a000-0000-7000-8000-000000000000", rent])
        XCTAssertEqual(entities.map(\.title), ["Call the bank", "Pay the rent"])
        XCTAssertTrue(entities[0].done)
        XCTAssertEqual(entities[1].priority, 1)
        XCTAssertEqual(entities[1].estimate, 30)
        XCTAssertEqual(entities[1].launch, "task:\(rent)")
    }

    func testATaskChangedSinceIsTheEntityAsItIsNow() throws {
        let rent = try added("Pay the rent")
        _ = try store.assist("edit_task", ["id": .text(rent), "priority": .number(2)])
        XCTAssertEqual(store.entities([rent]).first?.priority, 2)
    }
}
