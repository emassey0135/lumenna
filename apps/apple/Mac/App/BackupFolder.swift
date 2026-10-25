import Foundation

/// The backup folder the person chose, reachable again at every launch in the App Store
/// edition. The sandbox lets the app into a folder chosen in an open panel only until it quits,
/// so the choice is kept as a security-scoped bookmark (in the app's own defaults: this Mac's,
/// as the folder setting is) and taken up again before the first backup. The direct download is
/// not sandboxed and reaches the folder by its path, as `lum` does, so it keeps nothing here.
///
/// The default folder needs none of this: beside the profile, in the App Group's container.
enum BackupFolder {
    #if APP_STORE
    private static let key = "BackupFolderBookmark"
    /// The folder the app is in now, let go of when another is chosen.
    private static var holding: URL?
    #endif

    /// `url` was chosen for backups in an open panel: kept for the launches to come.
    static func chosen(_ url: URL) {
        #if APP_STORE
        guard let bookmark = try? url.bookmarkData(options: .withSecurityScope, includingResourceValuesForKeys: nil, relativeTo: nil) else {
            return
        }
        UserDefaults.standard.set(bookmark, forKey: key)
        hold(url)
        #endif
    }

    /// At launch, before any backup: back into the folder last chosen, if one was.
    static func resume() {
        #if APP_STORE
        guard let bookmark = UserDefaults.standard.data(forKey: key) else { return }
        var stale = false
        guard let url = try? URL(resolvingBookmarkData: bookmark, options: .withSecurityScope, relativeTo: nil, bookmarkDataIsStale: &stale) else {
            return
        }
        hold(url)
        // Moved or renamed since: kept again as it is now, while the app is in it.
        if stale { chosen(url) }
        #endif
    }

    #if APP_STORE
    private static func hold(_ url: URL) {
        guard url != holding else { return }
        holding?.stopAccessingSecurityScopedResource()
        holding = url.startAccessingSecurityScopedResource() ? url : nil
    }
    #endif
}
