import LumennaApp
import UIKit
import XCTest
@testable import Lumenna

/// The task form over the app's details: each field to the app as it changes, and Save sending
/// only what changed.
@MainActor
final class TaskFormTests: XCTestCase {
    private var core: Core!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
    }

    private func settle(_ until: () -> Bool = { true }) {
        let deadline = Date().addingTimeInterval(10)
        repeat {
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.02))
        } while !(core.app.isStill && until()) && Date() < deadline
    }

    /// The form of a task added through the surface, once the app has its details.
    private func form(_ text: String) throws -> TaskDetailModel {
        let id = try XCTUnwrap(core.lumenna.addTask(text: text).task?.id)
        core.app.tick()
        settle()
        let model = TaskDetailModel(core: core, id: id)
        settle { model.task != nil }
        return model
    }

    func testAProjectChosenAndSavedIsTheTasksProject() throws {
        _ = try core.lumenna.addProject(name: "Work", parent: nil)
        let model = try form("draft")
        settle { model.projects.contains { $0.id == "Work" } }
        XCTAssertEqual(model.projects.map(\.id), ["Inbox", "Work"])
        model.project = "Work"
        settle()
        model.save()
        settle()
        let id = try XCTUnwrap(model.task?.id)
        XCTAssertEqual(try core.lumenna.showTask(id: id).task.project, "Work")
        XCTAssertEqual(model.project, "Work", "the form shows what was saved")
    }

    func testATitleTypedIsSavedAndNothingElseIsSent() throws {
        let model = try form("water plants tomorrow p2")
        model.title = "water the plants"
        settle()
        model.save()
        settle()
        let id = try XCTUnwrap(model.task?.id)
        let saved = try core.lumenna.showTask(id: id).task
        XCTAssertEqual(saved.title, "water the plants")
        XCTAssertEqual(saved.priority, 2, "what was not typed stays")
    }
}
