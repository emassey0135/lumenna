import Foundation
import LumennaApp
import XCTest
@testable import Lumenna

/// The store's move into the App Group container (`AppGroup`, `ProfileMove`), over folders of
/// the test's own: a profile as a store leaves it, moved whole, never over another, and a move
/// cut short made again.
final class ProfileMoveTests: XCTestCase {
    private let manager = FileManager.default
    private var root: URL!
    /// Where builds before the group kept the store, and the group's container.
    private var own: URL!
    private var group: URL!

    override func setUpWithError() throws {
        root = manager.temporaryDirectory.appendingPathComponent("move-\(UUID().uuidString)", isDirectory: true)
        own = root.appendingPathComponent("Application Support/lumenna", isDirectory: true)
        group = root.appendingPathComponent("Group", isDirectory: true)
        try manager.createDirectory(at: group, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? manager.removeItem(at: root)
    }

    /// The files a profile holds: the store with its journal, and the device's own settings.
    private static let files = ["lumenna.sqlite3", "lumenna.sqlite3-wal", "lumenna.sqlite3-shm", "device-settings"]

    private func makeProfile(at directory: URL, saying text: String = "old") throws {
        try manager.createDirectory(at: directory, withIntermediateDirectories: true)
        for file in Self.files {
            try Data("\(text) \(file)".utf8).write(to: directory.appendingPathComponent(file))
        }
    }

    private func contents(_ directory: URL) throws -> [String: String] {
        var found: [String: String] = [:]
        for file in try manager.contentsOfDirectory(atPath: directory.path) {
            found[file] = try String(contentsOf: directory.appendingPathComponent(file), encoding: .utf8)
        }
        return found
    }

    private var shared: URL { group.appendingPathComponent("lumenna", isDirectory: true) }

    func testAProfileInTheAppsOwnContainerMovesWholeIntoTheGroup() throws {
        try makeProfile(at: own)
        let before = try contents(own)
        XCTAssertEqual(AppGroup.profile(movingFrom: own, into: group), shared)
        XCTAssertEqual(try contents(shared), before)
        XCTAssertFalse(manager.fileExists(atPath: own.path))
    }

    func testAMoveAcrossVolumesCopiesThenClearsUp() throws {
        try makeProfile(at: own)
        let before = try contents(own)
        XCTAssertTrue(try ProfileMove.move(from: own, to: shared, copying: true))
        XCTAssertEqual(try contents(shared), before)
        XCTAssertFalse(manager.fileExists(atPath: own.path))
        XCTAssertEqual(try manager.contentsOfDirectory(atPath: group.path), ["lumenna"])
    }

    func testAMoveCutShortIsMadeAgainFromTheWholeProfile() throws {
        try makeProfile(at: own)
        let before = try contents(own)
        // Interrupted while copying: half a copy beside where it goes, the profile still whole.
        let staging = group.appendingPathComponent(".lumenna.moving", isDirectory: true)
        try manager.createDirectory(at: staging, withIntermediateDirectories: true)
        try Data("half".utf8).write(to: staging.appendingPathComponent("lumenna.sqlite3"))
        XCTAssertTrue(try ProfileMove.move(from: own, to: shared, copying: true))
        XCTAssertEqual(try contents(shared), before)
        XCTAssertFalse(manager.fileExists(atPath: staging.path))
    }

    func testAProfileAlreadyInTheGroupIsNeverReplaced() throws {
        try makeProfile(at: own, saying: "old")
        try makeProfile(at: shared, saying: "new")
        XCTAssertEqual(AppGroup.profile(movingFrom: own, into: group), shared)
        XCTAssertEqual(try contents(shared)["device-settings"], "new device-settings")
        // Left as it is: it may be the person's too.
        XCTAssertEqual(try contents(own)["device-settings"], "old device-settings")
    }

    func testAnEmptyFolderInTheGroupIsNoProfile() throws {
        try makeProfile(at: own)
        try manager.createDirectory(at: shared, withIntermediateDirectories: true)
        XCTAssertTrue(try ProfileMove.move(from: own, to: shared))
        XCTAssertEqual(try contents(shared).count, Self.files.count)
    }

    func testMovingAgainOnceMovedDoesNothing() throws {
        try makeProfile(at: own)
        AppGroup.profile(movingFrom: own, into: group)
        let moved = try contents(shared)
        XCTAssertFalse(try ProfileMove.move(from: own, to: shared))
        XCTAssertEqual(AppGroup.profile(movingFrom: own, into: group), shared)
        XCTAssertEqual(try contents(shared), moved)
    }

    func testTheBackupsGoBesideTheProfileNeverIntoIt() throws {
        try makeProfile(at: own)
        let backups = own.deletingLastPathComponent().appendingPathComponent("lumenna-backups", isDirectory: true)
        try manager.createDirectory(at: backups, withIntermediateDirectories: true)
        try Data("backup".utf8).write(to: backups.appendingPathComponent("lumenna-2026-10-25.lumbak"))
        AppGroup.profile(movingFrom: own, into: group)
        XCTAssertEqual(
            try contents(group.appendingPathComponent("lumenna-backups"))["lumenna-2026-10-25.lumbak"], "backup"
        )
        XCTAssertFalse(manager.fileExists(atPath: shared.appendingPathComponent("lumenna-backups").path))
    }

    func testTheBackupsOfAProfileLeftBehindStayWithIt() throws {
        try makeProfile(at: own, saying: "old")
        try makeProfile(at: shared, saying: "new")
        let backups = own.deletingLastPathComponent().appendingPathComponent("lumenna-backups", isDirectory: true)
        try manager.createDirectory(at: backups, withIntermediateDirectories: true)
        AppGroup.profile(movingFrom: own, into: group)
        XCTAssertTrue(manager.fileExists(atPath: backups.path))
        XCTAssertFalse(manager.fileExists(atPath: group.appendingPathComponent("lumenna-backups").path))
    }

    func testWithoutAGroupTheStoreStaysWhereItWas() throws {
        try makeProfile(at: own)
        XCTAssertEqual(AppGroup.profile(movingFrom: own, into: nil), own)
        XCTAssertEqual(try contents(own).count, Self.files.count)
    }

    func testAStoreOpensAfterItsMove() throws {
        // A real store, written, closed, moved, and opened again from the group.
        let heads: String = try {
            let device = try Device(directory: own.path)
            _ = try device.addTask(text: "Water the plants")
            return try device.heads()
        }()
        XCTAssertEqual(AppGroup.profile(movingFrom: own, into: group), shared)
        let moved = try Device(directory: shared.path)
        XCTAssertEqual(try moved.heads(), heads)
    }
}

/// The glance file the widgets read (`GlanceFile`): the core's glance, written whole into a
/// folder of the test's own, and read back as the core reads it.
@MainActor
final class GlanceFileTests: XCTestCase {
    private var core: Core!
    private var directory: URL!

    override func setUp() async throws {
        setenv("LUMENNA_TEST_PROFILE", "unit-\(UUID().uuidString)", 1)
        core = try Core()
        directory = FileManager.default.temporaryDirectory.appendingPathComponent("glance-\(UUID().uuidString)", isDirectory: true)
    }

    override func tearDown() async throws {
        try? FileManager.default.removeItem(at: directory)
    }

    func testAStoreUnderTestWritesNoGlanceIntoThePersonsGroup() {
        XCTAssertNil(core.glance)
    }

    func testTheFileHoldsTheCoresGlanceAndReadsBack() throws {
        _ = try core.app.device.addTask(text: "Pay the rent today")
        let app = core.app
        // What was written, as the core gave it: a second glance would be of a later now.
        var given: [LumennaApp.GlanceEntry] = []
        let file = GlanceFile(directory: directory) {
            given = try app.glance(hours: GlanceFile.hours)
            return given
        }
        try file.writeNow()
        let written = try String(contentsOf: directory.appendingPathComponent(GlanceFile.name), encoding: .utf8)
        let entries = try LumennaApp.glanceFromFile(contents: written)
        XCTAssertFalse(given.isEmpty)
        XCTAssertEqual(entries, given)
        XCTAssertEqual(entries.first?.moment.due, 1)
        XCTAssertFalse(entries.first?.spoken.isEmpty ?? true)
    }

    func testWritingAgainReplacesTheFileAndCallsBack() async throws {
        let app = core.app
        let file = GlanceFile(directory: directory) { try app.glance(hours: GlanceFile.hours) }
        try file.writeNow()
        _ = try core.app.device.addTask(text: "Call the bank today")
        let written = expectation(description: "written")
        file.write { written.fulfill() }
        await fulfillment(of: [written], timeout: 10)
        let text = try String(contentsOf: file.url, encoding: .utf8)
        XCTAssertEqual(try LumennaApp.glanceFromFile(contents: text).first?.moment.due, 1)
    }
}
