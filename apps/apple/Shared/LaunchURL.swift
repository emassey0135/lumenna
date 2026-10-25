import Foundation

/// A way into the app from outside it (`launchShortcuts()`, `records::launch`), as the URL a
/// widget opens the app with (`widgetURL`): `lumenna://launch/<id>`, under the scheme each app
/// registers (`CFBundleURLTypes`). The app hands the id back as `ShellEvent.launch`, which does
/// it, so what a launch opens is the core's.
enum LaunchURL {
    static let scheme = "lumenna"

    /// The URL that opens the app on launch `id`.
    static func of(_ id: String) -> URL {
        var parts = URLComponents()
        parts.scheme = scheme
        parts.host = "launch"
        parts.path = "/" + id
        return parts.url!
    }

    /// The launch `url` names, if it is one.
    static func id(in url: URL) -> String? {
        guard url.scheme == scheme, url.host == "launch" else { return nil }
        let id = url.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        return id.isEmpty ? nil : id
    }
}
