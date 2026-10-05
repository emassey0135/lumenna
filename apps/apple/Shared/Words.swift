import Foundation

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
