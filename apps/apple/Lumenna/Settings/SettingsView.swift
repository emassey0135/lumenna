import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Settings (§16.1, §3.10): a short list of pages — what syncs, what is this device's alone,
/// and getting data in and out (§9).
///
/// Pages rather than one long form: a VoiceOver user reaches backups without swiping through
/// every weekday, and a page pushed over the tab bar hides it, so no row is ever read through
/// its glass — which the accessibility audit, rightly, cannot tell from unreadable text.
final class SettingsViewController: ItemListViewController {
    init(core: Core) {
        super.init(core: core, title: "Settings")
    }

    override func load() throws -> (items: [Item], count: String) {
        ([
            Item(key: "devices", title: "Devices and Sync", detail: try core.lumenna.syncStatus().announcement),
            Item(key: "planning", title: "Planning", detail: "The day, the week, completing subtasks, announcements"),
            Item(key: "backups", title: "Backups", detail: "On this device only"),
            Item(key: "export", title: "Export and Import", detail: "JSON, Markdown, org, calendar"),
        ], "")
    }

    override func open(_ item: Item) {
        let next: UIViewController = switch item.key {
        case "devices": DevicesViewController(core: core)
        case "planning": SettingsPageViewController(core: core, page: .planning)
        case "backups": SettingsPageViewController(core: core, page: .backups)
        default: SettingsPageViewController(core: core, page: .export)
        }
        navigationController?.pushViewController(next, animated: true)
    }
}

/// One page of settings.
enum SettingsPage {
    case planning, backups, export

    var title: String {
        switch self {
        case .planning: "Planning"
        case .backups: "Backups"
        case .export: "Export and Import"
        }
    }
}

final class SettingsPageViewController: UIHostingController<SettingsView> {
    private let model: SettingsModel

    init(core: Core, page: SettingsPage) {
        let model = SettingsModel(core: core)
        self.model = model
        super.init(rootView: SettingsView(model: model, page: page))
        title = page.title
        model.host = self
        hidesBottomBarWhenPushed = true
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        model.load()
    }
}

final class SettingsModel: NSObject, ObservableObject, UIDocumentPickerDelegate {
    let core: Core
    weak var host: UIViewController?
    @Published var values: [String: String] = [:]
    @Published var failure: String?

    init(core: Core) {
        self.core = core
    }

    func load() {
        do {
            values = Dictionary(
                uniqueKeysWithValues: try core.lumenna.settings(key: nil).settings.map { ($0.key, $0.value) }
            )
        } catch {
            failure = error.sentence
        }
    }

    func set(_ key: String, _ value: String) {
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

    // MARK: - Getting data in and out (§9)

    func backUpNow() {
        do {
            let done = try core.lumenna.backup(to: nil)
            Announcer.say(done.announcement, notices: done.notices)
        } catch {
            failure = error.sentence
        }
    }

    /// Writes an export to a file named for the day and offers it to share — AirDrop, Files,
    /// Mail. It holds the present state only, nothing from the trash.
    func export(_ format: ExportFormat) {
        let suffix: String = switch format {
        case .json: "json"
        case .markdown: "md"
        case .org: "org"
        case .ics: "ics"
        }
        let file = FileManager.default.temporaryDirectory
            .appendingPathComponent("Lumenna \(Clock.isoDay(.now)).\(suffix)")
        do {
            let done = try core.lumenna.export(format: format, path: file.path, replace: true)
            Announcer.say(done.announcement, notices: done.notices)
            let share = UIActivityViewController(activityItems: [file], applicationActivities: nil)
            share.popoverPresentationController?.sourceView = host?.view
            host?.present(share, animated: true)
        } catch {
            failure = error.sentence
        }
    }

    /// Picks a JSON export or a backup to read in; the core tells them apart.
    func pickImport() {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.json, .data, .item], asCopy: true)
        picker.delegate = self
        host?.present(picker, animated: true)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else { return }
        do {
            let imported = try core.lumenna.import(path: url.path)
            switch imported {
            case let .export(done): Announcer.say(done.announcement, notices: done.notices)
            case let .backup(done): Announcer.say(done.announcement, notices: done.notices)
            }
            NotificationCenter.default.post(name: Core.changed, object: nil)
        } catch {
            failure = error.sentence
        }
    }

}

struct SettingsView: View {
    @ObservedObject var model: SettingsModel
    let page: SettingsPage

    private static let weekdays = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]

    var body: some View {
        Form {
            switch page {
            case .planning: planning
            case .backups: backups
            case .export: export
            }
        }
        // SwiftUI's own accent, which the window's UIKit tint does not reach.
        .tint(Color(uiColor: .lumennaTint))
        .alert(
            "Could not do that",
            isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }),
            presenting: model.failure
        ) { _ in
            Button("OK") {}
        } message: { failure in
            Text(failure)
        }
    }

    @ViewBuilder private var planning: some View {
            // Pickers are drawn inline, a row per choice with the chosen one checked: a pop-up
            // menu's value is clipped at large text sizes, and a row reads plainly.
            Section {
                Toggle("Completing a task completes its subtasks", isOn: Binding(
                    get: { model.values["cascade-complete-subtasks"] == "true" },
                    set: { model.set("cascade-complete-subtasks", $0 ? "true" : "false") }
                ))
                DatePicker("Day starts", selection: model.time("day-start"), displayedComponents: .hourAndMinute)
                DatePicker("Day ends", selection: model.time("day-end"), displayedComponents: .hourAndMinute)
                DatePicker("All-day reminders at", selection: model.time("all-day-reminder-hour"), displayedComponents: .hourAndMinute)
            } header: {
                FormParts.caption("Planning")
            } footer: {
                FormParts.caption("These sync to all your devices.")
            }
            Section {
                Picker("Announcements", selection: model.binding("verbosity")) {
                    Text("Full sentences").tag("full")
                    Text("Terse").tag("terse")
                }
                .pickerStyle(.inline)
                .labelsHidden()
            } header: {
                FormParts.caption("Announcements")
            }
            Section {
                Picker("Week starts on", selection: model.binding("week-start")) {
                    ForEach(Self.weekdays, id: \.self) { day in
                        Text(day.capitalized).tag(day)
                    }
                }
                .pickerStyle(.inline)
                .labelsHidden()
            } header: {
                FormParts.caption("Week starts on")
            }
    }

    @ViewBuilder private var backups: some View {
            Section {
                Picker("Automatic backups", selection: model.binding("backup-every")) {
                    Text("Every 12 hours").tag("12h")
                    Text("Every day").tag("1d")
                    Text("Every week").tag("7d")
                    Text("Off").tag("off")
                }
                .pickerStyle(.inline)
                .labelsHidden()
            } header: {
                FormParts.caption("Automatic backups on this device")
            }
            Section {
                Stepper(value: Binding(
                    get: { Int(model.values["backup-keep"] ?? "10") ?? 10 },
                    set: { model.set("backup-keep", String($0)) }
                ), in: 1...100) {
                    Text("Keep \(model.values["backup-keep"] ?? "10") backups")
                }
                Button("Back Up Now") { model.backUpNow() }
            } footer: {
                FormParts.caption("A backup holds your whole history, including every task you deleted, so the store can be rebuilt from it. It stays on this device.")
            }
    }

    @ViewBuilder private var export: some View {
            Section {
                Menu("Export") {
                    Button("JSON, complete, can be imported") { model.export(.json) }
                    Button("Markdown checklist") { model.export(.markdown) }
                    Button("Org outline") { model.export(.org) }
                    Button("Calendar file of your blocks") { model.export(.ics) }
                }
                Button("Import or Restore") { model.pickImport() }
            } header: {
                FormParts.caption("Export and import")
            } footer: {
                FormParts.caption("An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this device lacks and removes nothing.")
            }
    }
}
