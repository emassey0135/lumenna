import XCTest
@testable import Lumenna

/// A row's words: the core's parts, joined with the time in this device's clock.
final class RowSpeechTests: XCTestCase {
    private func task(due: String?, at time: String?, value: String?) -> RowView {
        RowView(
            row: 1, id: "0199", role: "task", depth: 0, index: 1, count: 1, checked: false, expanded: nil,
            title: "call the bank", state: [], due: due, dueTime: time, value: value, hint: nil, actions: []
        )
    }

    func testADueTimeIsSaidInThisDevicesClockThenThePriority() {
        let row = task(due: "due tomorrow", at: "15:00", value: "priority 1")
        XCTAssertEqual(RowSpeech.details(row), "due tomorrow at \(Clock.time("15:00")), priority 1")
        XCTAssertNotEqual(Clock.time("15:00"), "", "the clock says something")
    }

    func testADayWithoutATimeIsSaidAlone() {
        XCTAssertEqual(RowSpeech.details(task(due: "due Friday", at: nil, value: nil)), "due Friday")
        XCTAssertNil(RowSpeech.details(task(due: nil, at: nil, value: nil)))
    }
}
