import Foundation

/// An instant as the core writes one (RFC 3339, to the nanosecond or to the second): a reminder's
/// moment, a glance's. One reading for the apps, the widgets and the tests alike, each of which
/// compiles this file.
enum Instant {
    static func parse(_ text: String) -> Date? {
        guard let dot = text.firstIndex(of: "."), let zone = text[dot...].firstIndex(where: { !$0.isNumber && $0 != "." }) else {
            return whole(text)
        }
        // Foundation reads at most milliseconds: the seconds whole, then their fraction.
        guard let seconds = whole(String(text[..<dot] + text[zone...])),
              let fraction = Double("0" + text[dot ..< zone]) else { return nil }
        return seconds.addingTimeInterval(fraction)
    }

    /// An instant to the second, in UTC or at an offset.
    private static func whole(_ text: String) -> Date? {
        (try? Date.ISO8601FormatStyle().parse(text)) ?? ISO8601DateFormatter().date(from: text)
    }

    /// `date` as the core reads an instant.
    static func rfc3339(_ date: Date) -> String {
        ISO8601DateFormatter().string(from: date)
    }
}
