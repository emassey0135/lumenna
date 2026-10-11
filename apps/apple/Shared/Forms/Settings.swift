import SwiftUI
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// The settings core keeps, as the Apple apps edit them. Getting files in and out is
/// each platform's own — a share sheet and a document picker on iOS, save and open panels on
/// macOS — in an extension beside each app; everything else is here. A watch has nowhere to
/// save a file or pick one, so backups and exports are the phone's.
final class SettingsModel: NSObject, ObservableObject {
    let core: Core
    #if os(iOS)
    /// The screen showing these, for presenting a share sheet or a picker.
    weak var host: UIViewController?
    #elseif os(macOS)
    /// The window showing these, for its panels and sheets.
    weak var window: NSWindow?
    #endif
    @Published var values: [String: String] = [:]
    /// Every setting as the core describes it — its name, its control, what it can be, whether
    /// it syncs — in the core's order.
    @Published var settings: [Setting] = []
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
            settings = try core.lumenna.settings(key: nil).settings
            values = Dictionary(uniqueKeysWithValues: settings.map { ($0.key, $0.value) })
        } catch {
            failure = error.sentence
        }
    }

    func set(_ key: String, _ value: String) {
        if let problem = commit(key, value) { failure = problem }
    }

    /// Sets `key`, returning the core's reason when it refuses the value: a line's refusal is
    /// shown with its field rather than as an alert.
    @discardableResult
    func commit(_ key: String, _ value: String) -> String? {
        guard values[key] != value else { return nil }
        var problem: String?
        do {
            let change = try core.lumenna.setSetting(key: key, value: value)
            Announcer.say(change.announcement, notices: change.notices)
            // Every view reads the store again, and reminders are scheduled anew: the
            // defaults, the all-day hour and whether reminders are on all change what fires.
            NotificationCenter.default.post(name: Core.changed, object: nil)
            // Turning reminders on is a moment to ask to show them, if never asked.
            if key == "reminders", value == "true" { Reminders.askIfUndecided() }
        } catch {
            problem = error.sentence
        }
        load()
        return problem
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

/// One setting, with the control its kind asks for, under the core's name for it. A choice is
/// a section of its own; the rest sit in the section around them.
struct SettingControl: View {
    @ObservedObject var model: SettingsModel
    let setting: Setting

    var body: some View {
        control.modifier(SettingHint(text: setting.kind == .choice ? "" : setting.hint))
    }

    @ViewBuilder private var control: some View {
        switch setting.kind {
        case .toggle:
            Labelled(setting.title) {
                Toggle(setting.title, isOn: Binding(
                    get: { model.values[setting.key] == "true" },
                    set: { model.set(setting.key, $0 ? "true" : "false") }
                ))
            }
        case .time:
            NamedDatePicker(setting.title, selection: model.time(setting.key), displayedComponents: .hourAndMinute)
        case .number:
            Stepper(value: Binding(
                get: { Int(model.values[setting.key] ?? "") ?? 1 },
                set: { model.set(setting.key, String($0)) }
            ), in: 1...100) {
                Text("\(setting.title): \(model.values[setting.key] ?? "")")
            }
        case .folder:
            #if os(macOS)
            Named(setting.title) {
                HStack {
                    Text(model.values[setting.key] ?? "").textSelection(.enabled).lineLimit(2)
                    Button("Choose…") { model.chooseBackupFolder() }
                }
            }
            #else
            // A folder of this device's is chosen where there is a file system to choose from.
            EmptyView()
            #endif
        case .line:
            LineSetting(model: model, setting: setting)
        case .choice:
            ChoiceSection(
                setting.title,
                selection: model.binding(setting.key),
                choices: options,
                footer: setting.hint.isEmpty ? nil : setting.hint
            )
        }
    }

    /// Its options, and the value it has if that is not among them: a newer build may have set it.
    private var options: [(label: String, value: String)] {
        let listed = setting.options.map { (label: $0.title, value: $0.id) }
        let value = model.values[setting.key] ?? setting.value
        return listed.contains { $0.value == value } || value.isEmpty ? listed : listed + [(label: value, value: value)]
    }
}

/// What a setting takes, the core's words: said by VoiceOver after a pause on iOS and the
/// watch, a tooltip on the Mac. A choice says it under its section instead.
private struct SettingHint: ViewModifier {
    let text: String

    func body(content: Content) -> some View {
        if text.isEmpty {
            content
        } else {
            #if os(macOS)
            content.help(text)
            #else
            content.accessibilityHint(text)
            #endif
        }
    }
}

/// A line the core reads, such as the default reminders' phrases: saved when it is committed
/// (Return, or leaving the field), never on each keystroke, which would send what is half
/// typed. A refusal stays with its field, in the core's words, and is said.
struct LineSetting: View {
    @ObservedObject var model: SettingsModel
    let setting: Setting
    @State private var text = ""
    @State private var problem: String?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Named(setting.title) {
                field
                    .focused($focused)
                    .onSubmit(commit)
                    #if os(iOS) || os(watchOS)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    #endif
            }
            if let problem {
                Text(problem)
                    .font(.footnote)
                    .foregroundStyle(Color.warningLabel)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onAppear { text = model.values[setting.key] ?? setting.value }
        .onChange(of: focused) { _, now in if !now { commit() } }
        .onChange(of: model.values[setting.key]) { _, value in
            if !focused, problem == nil { text = value ?? "" }
        }
        #if os(watchOS)
        // A watch's line comes whole from the system's input screen, with no keystrokes and
        // no Return: it is committed as it arrives.
        .onChange(of: text) { _, _ in if text != model.values[setting.key] { commit() } }
        #endif
    }

    /// The field itself, named as `namedField` names one, so focus can be followed.
    private var field: some View {
        #if os(macOS)
        TextField(setting.title, text: $text)
        #elseif os(iOS)
        // It wraps, so a large text size shows all of it (a one-line field scrolls sideways
        // and is clipped); so Return arrives as a line break, and commits instead.
        TextField("", text: Binding(get: { text }, set: { typed in
            guard typed.contains(where: \.isNewline) else { text = typed; return }
            text = typed.split(whereSeparator: \.isNewline).joined(separator: " ")
            // Leaving the field commits it.
            focused = false
        }), axis: .vertical)
        .multilineTextAlignment(.trailing)
        #else
        TextField("", text: $text)
        #endif
    }

    private func commit() {
        guard text != model.values[setting.key] else {
            problem = nil
            return
        }
        problem = model.commit(setting.key, text)
        if let problem {
            Announcer.say(problem)
        } else {
            text = model.values[setting.key] ?? text
        }
    }
}

extension SettingsModel {
    /// The settings that sync, of `kinds`, in the core's order.
    func synced(_ kinds: [SettingKind]) -> [Setting] {
        settings.filter { $0.syncs && kinds.contains($0.kind) }
    }

    /// What reminders are given, in the core's order: the defaults and the all-day hour,
    /// which sync, then whether this device gives them at all, which is its own.
    var reminderSettings: [Setting] {
        settings.filter { $0.key.contains("reminder") && ($0.syncs || $0.key == "reminders") }
    }

    /// This device's backup settings: what the core keeps apart from what syncs, but the clock,
    /// which the Apple apps take from the system.
    func backups(_ kinds: [SettingKind]) -> [Setting] {
        settings.filter { !$0.syncs && $0.key.hasPrefix("backup-") && kinds.contains($0.kind) }
    }
}

/// The settings that sync, as the core describes them. Which page each goes on is the app's.
struct PlanningSettings: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            Section {
                ForEach(model.synced([.toggle, .time, .number]).filter { !$0.key.contains("reminder") }, id: \.key) { setting in
                    SettingControl(model: model, setting: setting)
                }
            } header: {
                FormParts.heading("Planning")
            } footer: {
                FormParts.caption("These sync to all your devices.")
            }
            Section {
                ForEach(model.reminderSettings, id: \.key) { setting in
                    SettingControl(model: model, setting: setting)
                }
            } header: {
                FormParts.heading("Reminders")
            } footer: {
                FormParts.caption("The defaults sync to all your devices. Whether reminders are given here is this device's alone.")
            }
            ForEach(model.synced([.choice]), id: \.key) { setting in
                SettingControl(model: model, setting: setting)
            }
        }
        .modifier(SettingsPage(model: model))
    }
}

#if !os(watchOS)
struct BackupSettings: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        Form {
            ForEach(model.backups([.choice]), id: \.key) { setting in
                SettingControl(model: model, setting: setting)
            }
            Section {
                ForEach(model.backups([.toggle, .time, .number, .folder]), id: \.key) { setting in
                    SettingControl(model: model, setting: setting)
                }
                Button("Back Up Now") { model.backUpNow() }
                #if os(macOS)
                Button("Restore From a Backup…") { model.importFile() }
                #endif
            } footer: {
                FormParts.caption("These settings are this device's alone.")
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
                FormParts.heading("Export and Import")
            } footer: {
                FormParts.caption("An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this device lacks and removes nothing.")
            }
        }
        .modifier(SettingsPage(model: model))
    }
}
#endif
