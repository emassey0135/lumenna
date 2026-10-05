import SwiftUI
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// The settings core keeps, as both Apple apps edit them. Getting files in and out is
/// each platform's own — a share sheet and a document picker on iOS, save and open panels on
/// macOS — in an extension beside each app; everything else is here.
final class SettingsModel: NSObject, ObservableObject {
    let core: Core
    #if os(iOS)
    /// The screen showing these, for presenting a share sheet or a picker.
    weak var host: UIViewController?
    #else
    /// The window showing these, for its panels and sheets.
    weak var window: NSWindow?
    #endif
    @Published var values: [String: String] = [:]
    @Published var failure: String?

    init(core: Core) {
        self.core = core
        super.init()
        load()
        NotificationCenter.default.addObserver(self, selector: #selector(storeChanged), name: Core.changed, object: nil)
    }

    @objc private func storeChanged() { load() }

    func load() {
        do {
            values = Dictionary(uniqueKeysWithValues: try core.lumenna.settings(key: nil).settings.map { ($0.key, $0.value) })
        } catch {
            failure = error.sentence
        }
    }

    func set(_ key: String, _ value: String) {
        guard values[key] != value else { return }
        do {
            let change = try core.lumenna.setSetting(key: key, value: value)
            Announcer.say(change.announcement, notices: change.notices)
        } catch {
            failure = error.sentence
        }
        load()
    }

    func binding(_ key: String) -> Binding<String> {
        Binding(get: { self.values[key] ?? "" }, set: { self.set(key, $0) })
    }

    /// A time setting as a `Date` for a picker, written back as `HH:MM`.
    func time(_ key: String) -> Binding<Date> {
        Binding(
            get: {
                let parts = (self.values[key] ?? "08:00").split(separator: ":").compactMap { Int($0) }
                return Calendar.current.date(
                    bySettingHour: parts.first ?? 8, minute: parts.count > 1 ? parts[1] : 0, second: 0, of: .now
                ) ?? .now
            },
            set: { date in
                let parts = Calendar.current.dateComponents([.hour, .minute], from: date)
                self.set(key, String(format: "%02d:%02d", parts.hour ?? 0, parts.minute ?? 0))
            }
        )
    }

    func backUpNow() {
        do {
            let done = try core.lumenna.backup(to: nil)
            Announcer.say(done.announcement, notices: done.notices)
        } catch {
            failure = error.sentence
        }
    }

    /// The file name an export of `format` gets, dated.
    static func exportName(_ format: ExportFormat) -> String {
        let suffix = switch format {
        case .json: "json"
        case .markdown: "md"
        case .org: "org"
        case .ics: "ics"
        }
        return "Lumenna \(Clock.isoDay(.now)).\(suffix)"
    }

    /// Says what an import did and tells every view to read the store again.
    func imported(_ result: Imported) {
        switch result {
        case let .export(done): Announcer.say(done.announcement, notices: done.notices)
        case let .backup(done): Announcer.say(done.announcement, notices: done.notices)
        }
        NotificationCenter.default.post(name: Core.changed, object: nil)
    }
}

/// How a settings page sits on each platform, and its failure alert.
struct SettingsPage: ViewModifier {
    @ObservedObject var model: SettingsModel

    func body(content: Content) -> some View {
        content
            #if os(macOS)
            .formStyle(.grouped)
            .frame(width: 520)
            .fixedSize(horizontal: false, vertical: true)
            #endif
            .modifier(FailureAlert(failure: $model.failure))
    }
}

struct PlanningSettings: View {
    @ObservedObject var model: SettingsModel
    private static let weekdays = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]

    var body: some View {
        Form {
            Section {
                Labelled("Completing a task completes its subtasks") {
                    Toggle("Completing a task completes its subtasks", isOn: Binding(
                        get: { model.values["cascade-complete-subtasks"] == "true" },
                        set: { model.set("cascade-complete-subtasks", $0 ? "true" : "false") }
                    ))
                }
                Labelled("Day starts") {
                    DatePicker("Day starts", selection: model.time("day-start"), displayedComponents: .hourAndMinute)
                }
                Labelled("Day ends") {
                    DatePicker("Day ends", selection: model.time("day-end"), displayedComponents: .hourAndMinute)
                }
                Labelled("All-day reminders at") {
                    DatePicker("All-day reminders at", selection: model.time("all-day-reminder-hour"), displayedComponents: .hourAndMinute)
                }
            } header: {
                FormParts.heading("Planning")
            } footer: {
                FormParts.caption("These sync to all your devices.")
            }
            ChoiceSection("Announcements", selection: model.binding("verbosity"), choices: [
                ("Full sentences", "full"), ("Terse", "terse"),
            ])
            ChoiceSection("Week starts on", selection: model.binding("week-start"), choices: Self.weekdays.map { ($0.capitalized, $0) })
        }
        .modifier(SettingsPage(model: model))
    }
}

struct BackupSettings: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            ChoiceSection("Automatic backups", selection: model.binding("backup-every"), choices: [
                ("Every 12 hours", "12h"), ("Every day", "1d"), ("Every week", "7d"), ("Off", "off"),
            ])
            Section {
                Stepper(value: Binding(
                    get: { Int(model.values["backup-keep"] ?? "10") ?? 10 },
                    set: { model.set("backup-keep", String($0)) }
                ), in: 1...100) {
                    Text("Keep \(model.values["backup-keep"] ?? "10") backups")
                }
                #if os(macOS)
                Named("Backups go to") {
                    HStack {
                        Text(model.values["backup-dir"] ?? "").textSelection(.enabled).lineLimit(2)
                        Button("Choose…") { model.chooseBackupFolder() }
                    }
                }
                #endif
                Button("Back Up Now") { model.backUpNow() }
                #if os(macOS)
                Button("Restore From a Backup…") { model.importFile() }
                #endif
            } footer: {
                FormParts.caption("A backup holds your whole history, including every task you deleted, so the store can be rebuilt from it. It stays on this device, as these settings do.")
            }
        }
        .modifier(SettingsPage(model: model))
    }
}

struct ExportSettings: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Section {
                Button("Export JSON, Complete, Can Be Imported…") { model.export(.json) }
                Button("Export a Markdown Checklist…") { model.export(.markdown) }
                Button("Export an Org Outline…") { model.export(.org) }
                Button("Export a Calendar File of Your Blocks…") { model.export(.ics) }
                Button("Import or Restore…") { model.importFile() }
            } header: {
                FormParts.heading("Export and import")
            } footer: {
                FormParts.caption("An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this device lacks and removes nothing.")
            }
        }
        .modifier(SettingsPage(model: model))
    }
}
