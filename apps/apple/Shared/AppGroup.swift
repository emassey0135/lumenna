import Foundation

/// The App Group an app shares with its extensions (widgets, intents), which run as processes
/// of their own and see nothing of the app's own container. Its container holds the store on the
/// iPhone, the watch and the Mac App Store edition, and the glance file on every Apple app.
///
/// Its name is the build's (`LumennaAppGroup` in the Info.plist, from project.yml's
/// `LUMENNA_APP_GROUP`), so the app and each extension read the same one.
enum AppGroup {
    /// The group's identifier, as the build names it.
    static var identifier: String? {
        Bundle.main.object(forInfoDictionaryKey: "LumennaAppGroup") as? String
    }

    /// The group's container, or `nil` where the build has no entitlement to it (on iOS and
    /// watchOS an unsigned build, as CI's) or under a UI test, whose store is a fresh one of its
    /// own and which must not touch the person's container.
    static var container: URL? {
        guard ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] == nil, let identifier else { return nil }
        return FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: identifier)
    }

    /// The store's directory in the group's container, where the iPhone, the watch and the Mac
    /// App Store edition keep it, and where a widget finds it: `lumenna`, as builds before the
    /// group named it in Application Support.
    static var store: URL? {
        container?.appendingPathComponent("lumenna", isDirectory: true)
    }

    /// Where the store lives in a build that keeps it in the group, `own` being where builds
    /// before the group kept it: the group's `lumenna`, moved there from `own` first if `own`
    /// holds a profile and the group none (`ProfileMove`), with the backups beside it. Without
    /// the group, or if the move failed, `own`, as before: the store stays whole where it is, and
    /// the next launch tries again.
    static func profile(movingFrom own: URL) -> URL {
        profile(movingFrom: own, into: container)
    }

    /// `profile(movingFrom:)` into `container`, which tests name.
    @discardableResult
    static func profile(movingFrom own: URL, into container: URL?, manager: FileManager = .default) -> URL {
        guard let container else { return own }
        let shared = container.appendingPathComponent(own.lastPathComponent, isDirectory: true)
        do {
            try ProfileMove.move(from: own, to: shared, manager: manager)
        } catch {
            return own
        }
        // Backups stay beside the profile, never in it, where the core looks for them by
        // default: moved once the profile has gone, so a group that had a profile of its own
        // before never takes the backups of the one left behind.
        if !manager.fileExists(atPath: own.path) {
            let backups = own.lastPathComponent + "-backups"
            _ = try? ProfileMove.move(
                from: own.deletingLastPathComponent().appendingPathComponent(backups, isDirectory: true),
                to: container.appendingPathComponent(backups, isDirectory: true),
                manager: manager
            )
        }
        return shared
    }
}

/// Moving a profile directory — the SQLite file with its `-wal` and `-shm`, the device
/// settings, everything in it — to another place, before anything has it open.
///
/// Safe to interrupt and run again. Within one volume, as an app's container and its group's
/// are, the whole directory is renamed at once: it is either still where it was or already where
/// it goes. Across volumes it is copied beside where it goes under a name of its own, renamed
/// into place once whole, and only then removed from where it was; a copy cut short is thrown
/// away and made again, and a profile already in place is never touched again. A profile where
/// it goes that holds anything is kept, and the one it would have replaced left as it is, since
/// either may be the person's.
enum ProfileMove {
    /// Moves `old` to `new` if `old` exists and `new` holds nothing. Returns whether it moved.
    /// `copying` takes the way across volumes whatever the volumes are, for tests.
    @discardableResult
    static func move(from old: URL, to new: URL, manager: FileManager = .default, copying: Bool = false) throws -> Bool {
        let parent = new.deletingLastPathComponent()
        let staging = parent.appendingPathComponent(".\(new.lastPathComponent).moving", isDirectory: true)
        // A copy left by a move cut short: never the profile, which is renamed into place whole.
        if manager.fileExists(atPath: staging.path) { try manager.removeItem(at: staging) }
        guard manager.fileExists(atPath: old.path) else { return false }
        if manager.fileExists(atPath: new.path) {
            guard try manager.contentsOfDirectory(atPath: new.path).isEmpty else { return false }
            try manager.removeItem(at: new)
        }
        try manager.createDirectory(at: parent, withIntermediateDirectories: true)
        if try !copying && sameVolume(old, parent) {
            try manager.moveItem(at: old, to: new)
        } else {
            try manager.copyItem(at: old, to: staging)
            try manager.moveItem(at: staging, to: new)
            // In place and whole: what is left where it was is a copy, cleared up if it can be.
            try? manager.removeItem(at: old)
        }
        return true
    }

    /// Whether a rename can take `item` into `directory`.
    private static func sameVolume(_ item: URL, _ directory: URL) throws -> Bool {
        let from = try item.resourceValues(forKeys: [.volumeIdentifierKey]).volumeIdentifier
        let to = try directory.resourceValues(forKeys: [.volumeIdentifierKey]).volumeIdentifier
        guard let from = from as? NSObject, let to = to as? NSObject else { return false }
        return from.isEqual(to)
    }
}
