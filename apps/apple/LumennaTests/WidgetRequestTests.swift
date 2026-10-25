import Foundation
import LumennaApp
import XCTest
@testable import Lumenna

/// The Mac direct download's Mark Done, handed from its widget to the app through the group
/// (`Shared/WidgetRequests.swift`): what the widget writes the app runs through the core and
/// removes, any other operation is removed unrun, and a run that fails leaves the request to be
/// tried again, a few times only.
@MainActor
final class WidgetRequestTests: XCTestCase {
    private var core: Core!
    private var container: URL!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
        container = FileManager.default.temporaryDirectory.appendingPathComponent("widget-requests-\(UUID().uuidString)", isDirectory: true)
        let container = container!
        addTeardownBlock { try? FileManager.default.removeItem(at: container) }
    }

    func testARequestTheWidgetWritesIsRunByTheAppAndThenRemoved() throws {
        let store = AssistantStore(device: core.app.device)
        _ = try store.assist("add_task", ["text": .text("Pay the rent")])
        let task = try XCTUnwrap(store.tasks(try store.assist("list_tasks", ["query": .text("")])).first)

        let file = try WidgetRequests.hand(.init(operation: "complete_task", arguments: ["id": task.id]), into: container)
        XCTAssertEqual(try names(), [file.lastPathComponent], "only the whole request is left, no partial one")

        let requests = WidgetRequests(container: container)
        let device = core.app.device
        XCTAssertTrue(requests.take { _ = try device.assist(operation: $0, arguments: $1) })
        XCTAssertEqual(try names(), [])
        XCTAssertTrue(try XCTUnwrap(store.task(task.id)).state.contains("completed"))
        XCTAssertFalse(requests.take { _, _ in XCTFail("nothing is left to run") })
    }

    func testTwoRequestsNeverShareAFileAndAreTakenInTheOrderMade() throws {
        let first = try WidgetRequests.hand(.init(operation: "complete_task", arguments: ["id": "a"]), into: container)
        let second = try WidgetRequests.hand(.init(operation: "complete_task", arguments: ["id": "b"]), into: container)
        XCTAssertNotEqual(first, second)
        var ran: [String] = []
        WidgetRequests(container: container).take { _, arguments in
            if case let .text(id) = arguments.first?.value { ran.append(id) }
        }
        XCTAssertEqual(ran, ["a", "b"])
    }

    func testAnyOtherOperationOrAFileThatDoesNotReadIsRemovedUnrun() throws {
        let store = AssistantStore(device: core.app.device)
        _ = try store.assist("add_task", ["text": .text("Keep this")])
        let task = try XCTUnwrap(store.tasks(try store.assist("list_tasks", ["query": .text("")])).first)

        try WidgetRequests.hand(.init(operation: "erase_task_permanently", arguments: ["id": task.id, "confirm": "true"]), into: container)
        try WidgetRequests.hand(.init(operation: "complete_task", arguments: [:]), into: container)
        try Data("not a request".utf8).write(to: directory.appendingPathComponent("0-garbage.json"))

        let device = core.app.device
        let ran = WidgetRequests(container: container).take { id, arguments in
            XCTFail("\(id) ran")
            _ = try device.assist(operation: id, arguments: arguments)
        }
        XCTAssertFalse(ran)
        XCTAssertEqual(try names(), [])
        XCTAssertNotNil(try store.task(task.id), "the task is still there")
    }

    func testCompleteATaskIsGivenTheTaskAloneWhateverElseTheFileNames() throws {
        try WidgetRequests.hand(.init(operation: "complete_task", arguments: ["id": "a", "confirm": "true"]), into: container)
        var given: [LumennaApp.Argument] = []
        WidgetRequests(container: container).take { _, arguments in given = arguments }
        XCTAssertEqual(given.map(\.name), ["id"])
    }

    func testARunThatFailsLeavesTheRequestToBeTriedAgainAFewTimesOnly() throws {
        try WidgetRequests.hand(.init(operation: "complete_task", arguments: ["id": "a"]), into: container)
        let requests = WidgetRequests(container: container)
        let failing: (String, [LumennaApp.Argument]) throws -> Void = { _, _ in throw CocoaError(.fileWriteUnknown) }
        for _ in 1 ..< WidgetRequests.tries {
            XCTAssertFalse(requests.take(running: failing))
            XCTAssertEqual(try names().count, 1, "a failed run leaves the request")
        }
        XCTAssertFalse(requests.take(running: failing))
        XCTAssertEqual(try names(), [], "given up after the last try")
    }

    func testARequestThatFailedOnceIsRemovedWhenItRuns() throws {
        try WidgetRequests.hand(.init(operation: "complete_task", arguments: ["id": "a"]), into: container)
        let requests = WidgetRequests(container: container)
        XCTAssertFalse(requests.take { _, _ in throw CocoaError(.fileWriteUnknown) })
        XCTAssertEqual(try names().count, 1)
        XCTAssertTrue(requests.take { _, _ in })
        XCTAssertEqual(try names(), [])
    }

    func testAPartialRequestIsNeverTaken() throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data("{\"operation\":\"complete_task\",".utf8).write(to: directory.appendingPathComponent(".1-x.partial"))
        XCTAssertFalse(WidgetRequests(container: container).take { _, _ in XCTFail("a partial request ran") })
        XCTAssertEqual(try names(), [".1-x.partial"])
    }

    private var directory: URL {
        container.appendingPathComponent(WidgetRequests.folder, isDirectory: true)
    }

    private func names() throws -> [String] {
        try FileManager.default.contentsOfDirectory(atPath: directory.path).sorted()
    }
}
