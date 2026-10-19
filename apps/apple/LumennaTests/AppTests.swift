import Foundation
import LumennaApp
import XCTest

/// The Crux app, through BoltFFI's Swift, started over a store of its own by hand, as every
/// screen's `AppModel` starts it.
final class AppTests: XCTestCase {
    /// Turns as the app hands them on, from a thread of its own.
    private final class Turns: LumennaApp.Shell, @unchecked Sendable {
        private let lock = NSLock()
        private var turns: [Turn] = []

        func turn(turn: Turn) {
            lock.withLock { turns.append(turn) }
        }

        var last: Turn? { lock.withLock { turns.last } }
    }

    /// This device's times and dates, as the screens hand them to the app.
    private final class Writing: LumennaApp.Locale {
        func time(hour: Int8, minute: Int8) -> String { String(format: "%02d:%02d", hour, minute) }

        func date(year: Int16, month: Int8, day: Int8, withYear: Bool) -> String {
            withYear ? "\(day)/\(month)/\(year)" : "\(day)/\(month)"
        }
    }

    func testTheAppStartsAndListsThePlaces() throws {
        let profile = FileManager.default.temporaryDirectory.appendingPathComponent("app-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: profile) }
        let device = try Device(directory: profile.path)
        let turns = Turns()
        let app = try App(device: device, platform: "ios", locale: Writing(), shell: turns)
        let deadline = Date().addingTimeInterval(10)
        while !(app.isStill() && turns.last?.view.ready == true), Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.02))
        }
        let view = try XCTUnwrap(turns.last?.view, "no turn arrived")
        XCTAssertTrue(view.ready)
        XCTAssertEqual(view.sidebar.first?.title, "Today")
        XCTAssertTrue(view.sidebar.contains { $0.title == "Projects" })
    }
}
