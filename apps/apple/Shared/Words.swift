import Foundation
import LumennaApp
#if os(iOS)
import UIKit
#endif

/// Every fixed word this app draws, the core's (`shellWords`): asked for once, in the title case
/// Apple's platforms write, for the device this is. A word missing from it goes in the core's
/// `ShellWords`, never in Swift.
let Wording = LumennaApp.shellWords(capitals: .title, device: ThisDeviceKind.current)

/// The core's commands, by the ids its menus name them with (`menus`, `trayMenu`): what each is
/// called, and what each menu is.
enum Commands {
    private static let menus = LumennaApp.menus(sentence: false)
    private static let items = menus.flatMap(\.items) + LumennaApp.trayMenu(sentence: false)

    /// The command's name: "New Task".
    static func title(_ id: String) -> String {
        items.first { $0.id == id }?.title ?? id
    }

    /// Its name in a menu or on a button, "…" after one that asks something first, as Apple's
    /// platforms mark it.
    static func asking(_ id: String) -> String {
        let item = items.first { $0.id == id }
        return (item?.title ?? id) + (item?.asks == true ? "…" : "")
    }

    /// A menu's name: "File".
    static func menu(_ id: String) -> String {
        menus.first { $0.id == id }?.title ?? id
    }
}

/// The core's settings pages (`settingsPages`), for a screen or button named as its page is
/// before the app has shown settings.
enum Pages {
    private static let pages = LumennaApp.settingsPages(ownClock: true, sentence: false)

    /// The page's title: "Devices and Sync".
    static func title(_ id: String) -> String {
        pages.first { $0.id == id }?.title ?? id
    }

    /// One of its buttons, by id: "Pair a Device".
    static func button(_ page: String, _ id: String) -> String {
        pages.first { $0.id == page }?.buttons.first { $0.id == id }?.title ?? id
    }

    /// The same, "…" after it where it asks something first, as Apple's platforms mark it.
    static func asking(_ page: String, _ id: String) -> String {
        let button = pages.first { $0.id == page }?.buttons.first { $0.id == id }
        return (button?.title ?? id) + (button?.asks == true ? "…" : "")
    }
}

/// What this device is, as the core names it in its sentences ("this iPad").
enum ThisDeviceKind {
    static var current: LumennaApp.ThisDevice {
        #if os(iOS)
        UIDevice.current.userInterfaceIdiom == .pad ? .iPad : .iPhone
        #elseif os(macOS)
        .mac
        #else
        .watch
        #endif
    }
}

/// The core counts text in UTF-8 bytes, as Rust strings do; Apple's text APIs count UTF-16.
enum TextOffsets {
    /// A UTF-16 offset as the byte offset the core wants.
    static func bytes(_ utf16: Int, in text: String) -> UInt32 {
        let index = String.Index(utf16Offset: max(0, min(utf16, text.utf16.count)), in: text)
        return UInt32(text[..<index].utf8.count)
    }

    /// A byte offset from the core as a UTF-16 offset, never splitting a character.
    static func utf16(_ bytes: UInt32, in text: String) -> Int {
        let prefix = Array(text.utf8.prefix(Int(bytes)))
        return String(decoding: prefix, as: UTF8.self).utf16.count
    }
}

/// Putting a completion the core offered in place of what it completes.
enum CompletionText {
    /// `text` with `candidate` in place of the bytes from `start` to `end` (the core counts
    /// UTF-8 bytes), and a space after it so the next word can follow.
    static func insert(_ candidate: String, into text: String, start: UInt32, end: UInt32) -> (text: String, cursor: Int) {
        let bytes = Array(text.utf8)
        let from = min(Int(start), bytes.count)
        let to = min(max(Int(end), from), bytes.count)
        let before = String(decoding: bytes[..<from], as: UTF8.self)
        let after = String(decoding: bytes[to...], as: UTF8.self)
        let inserted = candidate + (after.hasPrefix(" ") ? "" : " ")
        return (before + inserted + after, (before + inserted).utf16.count)
    }
}

extension Error {
    /// The sentence the core wrote, which is already phrased to be read aloud.
    var sentence: String {
        if let error = self as? LumennaApp.LumennaError {
            let message: String
            switch error {
            case let .failed(text), let .syncElsewhere(text): message = text
            }
            return message.prefix(1).uppercased() + message.dropFirst()
        }
        return localizedDescription
    }
}
