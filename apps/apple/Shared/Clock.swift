import Foundation

/// Times and durations as a person says them, in their own locale.
///
/// The core sends `HH:MM` and minutes, which are components; how a time is spoken —
/// "9:00 AM" or "09:00" — is this device's convention, so it is decided here.
enum Clock {
    private static let parser: DateFormatter = {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = "HH:mm"
        return formatter
    }()

    private static let speaker: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    private static let lengths: DateComponentsFormatter = {
        let formatter = DateComponentsFormatter()
        formatter.unitsStyle = .full
        formatter.allowedUnits = [.hour, .minute]
        return formatter
    }()

    /// `14:30` as this device says it: "2:30 PM", or "14:30".
    static func time(_ clock: String) -> String {
        parser.date(from: clock).map(speaker.string(from:)) ?? clock
    }

    /// "1 hour, 30 minutes".
    static func length(_ minutes: UInt32) -> String {
        lengths.string(from: TimeInterval(minutes) * 60) ?? "\(minutes) minutes"
    }

    /// A day as the core reads it back: an ISO date in the phone's own time zone.
    static func isoDay(_ date: Date) -> String {
        date.formatted(Date.ISO8601FormatStyle(timeZone: .current).year().month().day())
    }

    /// An ISO date as a person would say it: "Monday 5 October".
    static func spokenDay(_ iso: String) -> String {
        guard let date = try? Date.ISO8601FormatStyle(timeZone: .current).year().month().day().parse(iso)
        else { return iso }
        if Calendar.current.isDateInToday(date) { return "Today" }
        if Calendar.current.isDateInTomorrow(date) { return "Tomorrow" }
        if Calendar.current.isDateInYesterday(date) { return "Yesterday" }
        return date.formatted(.dateTime.weekday(.wide).day().month(.wide))
    }
}
