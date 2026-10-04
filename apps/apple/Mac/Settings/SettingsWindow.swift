import AppKit
import ServiceManagement
import SwiftUI
import UniformTypeIdentifiers

/// Settings (§3.10, §16.1), in the Mac's own shape: a window of tabs, opened with ⌘,.
final class SettingsWindowController: NSWindowController {
    init(core: Core) {
        let tabs = NSTabViewController()
        tabs.tabStyle = .toolbar
        let model = SettingsModel(core: core)
        func tab(_ controller: NSViewController, _ label: String, _ symbol: String) -> NSTabViewItem {
            let item = NSTabViewItem(viewController: controller)
            item.label = label
            controller.view.setAccessibilityLabel(label)
            item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
            return item
        }
        tabs.tabViewItems = [
            tab(NSHostingController(rootView: GeneralSettings(model: model)), "General", "gearshape"),
            tab(NSHostingController(rootView: PlanningSettings(model: model)), "Planning", "calendar"),
            tab(DevicesViewController(core: core), "Devices", "laptopcomputer.and.iphone"),
            tab(NSHostingController(rootView: BackupSettings(model: model)), "Backups", "externaldrive"),
            tab(NSHostingController(rootView: ExportSettings(model: model)), "Export and Import", "square.and.arrow.up"),
        ]
        let window = NSWindow(contentViewController: tabs)
        window.title = "Settings"
        window.identifier = NSUserInterfaceItemIdentifier("settings")
        window.styleMask = [.titled, .closable]
        super.init(window: window)
        model.window = window
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }
}

final class SettingsModel: ObservableObject {
    let core: Core
    weak var window: NSWindow?
    @Published var values: [String: String] = [:]
    @Published var failure: String?

    init(core: Core) {
        self.core = core
        load()
        NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.load()
        }
    }

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
                return Calendar.current.date(bySettingHour: parts.first ?? 8, minute: parts.count > 1 ? parts[1] : 0, second: 0, of: .now) ?? .now
            },
            set: { date in
                let parts = Calendar.current.dateComponents([.hour, .minute], from: date)
                self.set(key, String(format: "%02d:%02d", parts.hour ?? 0, parts.minute ?? 0))
            }
        )
    }

    // MARK: - Shortcuts from anywhere (§16.2)

    @Published var shortcuts: [HotKeys.Kind: String] = Dictionary(
        uniqueKeysWithValues: HotKeys.Kind.allCases.map { ($0, HotKeys.description($0)) }
    )

    private func refreshShortcuts() {
        shortcuts = Dictionary(uniqueKeysWithValues: HotKeys.Kind.allCases.map { ($0, HotKeys.description($0)) })
    }

    func recordShortcut(_ kind: HotKeys.Kind) {
        guard let window else { return }
        HotKeys.record(kind, on: window) { [weak self] in self?.refreshShortcuts() }
    }

    func toggleShortcut(_ kind: HotKeys.Kind) {
        HotKeys.set(kind, to: HotKeys.shortcut(kind) == nil ? kind.standard : nil)
        refreshShortcuts()
        Announcer.say("\(kind.name) is \(HotKeys.description(kind))")
    }

    // MARK: - Opening at login (§16.2)

    var opensAtLogin: Bool {
        get { SMAppService.mainApp.status == .enabled }
        set {
            do {
                if newValue { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
                Announcer.say(newValue ? "Lumenna opens at login" : "Lumenna no longer opens at login")
            } catch {
                failure = error.localizedDescription
            }
            objectWillChange.send()
        }
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

    func chooseBackupFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Back Up Here"
        panel.message = "Backups hold your whole history, including everything you deleted."
        guard let window else { return }
        panel.beginSheetModal(for: window) { response in
            if response == .OK, let url = panel.url { self.set("backup-dir", url.path) }
        }
    }

    /// Writes an export where the person chooses. It holds the present state only, nothing
    /// from the trash.
    func export(_ format: ExportFormat) {
        let suffix = switch format {
        case .json: "json"
        case .markdown: "md"
        case .org: "org"
        case .ics: "ics"
        }
        let panel = NSSavePanel()
        panel.nameFieldStringValue = "Lumenna \(Clock.isoDay(.now)).\(suffix)"
        panel.canCreateDirectories = true
        guard let window else { return }
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            do {
                // The panel already asked about replacing an existing file.
                let done = try self.core.lumenna.export(format: format, path: url.path, replace: true)
                Announcer.say(done.announcement, notices: done.notices)
            } catch {
                self.failure = error.sentence
            }
        }
    }

    /// Reads a JSON export or a backup in; the core tells them apart.
    func importFile() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json, UTType(filenameExtension: "lumbak") ?? .data]
        panel.prompt = "Import"
        guard let window else { return }
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            do {
                switch try self.core.lumenna.import(path: url.path) {
                case let .export(done): Announcer.say(done.announcement, notices: done.notices)
                case let .backup(done): Announcer.say(done.announcement, notices: done.notices)
                }
                NotificationCenter.default.post(name: Core.changed, object: nil)
            } catch {
                self.failure = error.sentence
            }
        }
    }
}

/// Shows a model's failure as an alert, on any settings page.
private struct Failures: ViewModifier {
    @ObservedObject var model: SettingsModel

    func body(content: Content) -> some View {
        content
            .formStyle(.grouped)
            .tint(.lumennaTint)
            .frame(width: 520)
            .fixedSize(horizontal: false, vertical: true)
            .alert(
                "Could not do that",
                isPresented: Binding(get: { model.failure != nil }, set: { if !$0 { model.failure = nil } }),
                presenting: model.failure
            ) { _ in
                Button("OK") {}
            } message: { Text($0) }
    }
}

struct GeneralSettings: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Section {
                Toggle("Open Lumenna at login", isOn: Binding(get: { model.opensAtLogin }, set: { model.opensAtLogin = $0 }))
                Text("Lumenna stays running in the menu bar when its window is closed, so your devices stay in sync. Quit it from the menu bar or the Lumenna menu.")
                    .font(.footnote).foregroundStyle(Color.quietLabel)
            }
            Section {
                ForEach(HotKeys.Kind.allCases, id: \.self) { kind in
                    LabeledContent(kind.name) {
                        HStack {
                            Text(model.shortcuts[kind] ?? "")
                            Button("Change…") { model.recordShortcut(kind) }
                                .accessibilityLabel("Change shortcut for \(kind.name)")
                            Button(model.shortcuts[kind] == "Off" ? "Turn On" : "Turn Off") {
                                model.toggleShortcut(kind)
                            }
                            .accessibilityLabel("\(model.shortcuts[kind] == "Off" ? "Turn on" : "Turn off") shortcut for \(kind.name)")
                        }
                    }
                }
            } header: {
                Text("Shortcuts from anywhere")
            } footer: {
                Text("These work in any app, so they take their keys from whatever app is in front. Control-Command, because Control-Option is VoiceOver's.")
                    .font(.footnote).foregroundStyle(Color.quietLabel)
            }
        }
        .modifier(Failures(model: model))
    }
}

struct PlanningSettings: View {
    @ObservedObject var model: SettingsModel
    private static let weekdays = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]

    var body: some View {
        Form {
            Section {
                Toggle("Completing a task completes its subtasks", isOn: Binding(
                    get: { model.values["cascade-complete-subtasks"] == "true" },
                    set: { model.set("cascade-complete-subtasks", $0 ? "true" : "false") }
                ))
                Named("Day starts") { DatePicker("Day starts", selection: model.time("day-start"), displayedComponents: .hourAndMinute) }
                Named("Day ends") { DatePicker("Day ends", selection: model.time("day-end"), displayedComponents: .hourAndMinute) }
                Named("All-day reminders at") { DatePicker("All-day reminders at", selection: model.time("all-day-reminder-hour"), displayedComponents: .hourAndMinute) }
                Named("Announcements") {
                    Picker("Announcements", selection: model.binding("verbosity")) {
                        Text("Full sentences").tag("full")
                        Text("Terse").tag("terse")
                    }
                }
                Named("Week starts on") {
                    Picker("Week starts on", selection: model.binding("week-start")) {
                        ForEach(Self.weekdays, id: \.self) { Text($0.capitalized).tag($0) }
                    }
                }
            } footer: {
                Text("These sync to all your devices.").font(.footnote).foregroundStyle(Color.quietLabel)
            }
        }
        .modifier(Failures(model: model))
    }
}

struct BackupSettings: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Section {
                Named("Automatic backups") {
                    Picker("Automatic backups", selection: model.binding("backup-every")) {
                        Text("Every 12 hours").tag("12h")
                        Text("Every day").tag("1d")
                        Text("Every week").tag("7d")
                        Text("Off").tag("off")
                    }
                }
                Stepper(value: Binding(
                    get: { Int(model.values["backup-keep"] ?? "10") ?? 10 },
                    set: { model.set("backup-keep", String($0)) }
                ), in: 1...100) {
                    Text("Keep \(model.values["backup-keep"] ?? "10") backups")
                }
                LabeledContent("Backups go to") {
                    HStack {
                        Text(model.values["backup-dir"] ?? "").textSelection(.enabled).lineLimit(2)
                        Button("Choose…") { model.chooseBackupFolder() }
                    }
                }
                HStack {
                    Button("Back Up Now") { model.backUpNow() }
                    Button("Restore From a Backup…") { model.importFile() }
                }
            } footer: {
                Text("A backup holds your whole history, including every task you deleted, so the store can be rebuilt from it. These settings are this Mac's alone. Restoring adds what this Mac lacks and removes nothing.")
                    .font(.footnote).foregroundStyle(Color.quietLabel)
            }
        }
        .modifier(Failures(model: model))
    }
}

struct ExportSettings: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Section {
                Button("Export JSON, complete, can be imported…") { model.export(.json) }
                Button("Export a Markdown checklist…") { model.export(.markdown) }
                Button("Export an org outline…") { model.export(.org) }
                Button("Export a calendar file of your blocks…") { model.export(.ics) }
                Button("Import or Restore…") { model.importFile() }
            } footer: {
                Text("An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this Mac lacks and removes nothing.")
                    .font(.footnote).foregroundStyle(Color.quietLabel)
            }
        }
        .modifier(Failures(model: model))
    }
}
