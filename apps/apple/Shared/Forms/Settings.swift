import LumennaApp
import SwiftUI
#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// The settings, as the app has them open (`ViewModel.settings`) — some sync, some are this
/// device's own — edited by the Apple apps. Applying one is the app's, which says what changed or
/// why not, and keeps a refused line beside its reason (`problem`). Getting files in and out is
/// each platform's own — a share sheet and a document picker on iOS, save and open panels on
/// macOS — in an extension beside each app; everything else is here. A watch has nowhere to save
/// a file or pick one, so backups and exports are the phone's.
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
    @Published var settings: [LumennaApp.Setting] = []
    /// Why a line was refused, by its key, the app's.
    @Published private(set) var problems: [String: String] = [:]
    /// The exports offered, the app's: each format's button and the file it is named.
    @Published private(set) var exports: [LumennaApp.ExportChoice] = []
    /// What is done to the store as a whole, the app's: Erase Everything Deleted.
    @Published private(set) var offered: [LumennaApp.Offered] = []
    /// What the core says under them: what erasing takes.
    @Published private(set) var offeredCaption = ""
    /// The pages the settings are on, the core's, in its order, each setting on exactly one.
    @Published private(set) var pages: [LumennaApp.SettingsPage] = LumennaApp.settingsPages(ownClock: true, sentence: false)
    /// What a setting's own control says: a time's hint and example, a number's.
    @Published private(set) var words = LumennaApp.settingWords()
    @Published var failure: String?
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        super.init()
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.follow()
        }
        load()
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// Opens the settings in the app, which asks the store for them: as a page appears.
    func load() {
        core.app.send(.settings(.open))
        follow()
    }

    /// The app's settings, into the controls.
    private func follow() {
        guard let shown = core.app.view.settings else { return }
        settings = shown.settings.map(\.setting)
        values = Dictionary(shown.settings.map { ($0.setting.key, $0.setting.value) }, uniquingKeysWith: { first, _ in first })
        problems = Dictionary(shown.settings.compactMap { setting in setting.problem.map { (setting.setting.key, $0) } }, uniquingKeysWith: { first, _ in first })
        exports = shown.exports
        offered = shown.offered
        offeredCaption = shown.offeredCaption
        if pages != shown.pages { pages = shown.pages }
        if words != shown.words { words = shown.words }
    }

    /// The page `id`, the core's.
    func page(_ id: String) -> LumennaApp.SettingsPage? {
        pages.first { $0.id == id }
    }

    /// The settings on `page`, in its order, those this device has.
    func settings(on page: LumennaApp.SettingsPage) -> [LumennaApp.Setting] {
        page.keys.compactMap { key in settings.first { $0.key == key } }
    }

    /// One of the store's own actions, handed back to the app, which asks its question first.
    func act(_ action: LumennaApp.Offered) {
        core.app.act(action, on: .settings)
    }

    func set(_ key: String, _ value: String) {
        guard values[key] != value else { return }
        core.app.send(.settings(.set(key: key, value: value)))
        // Turning reminders on is a moment to ask to show them, if never asked.
        if key == "reminders", value == "true" { Reminders.askIfUndecided() }
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

    /// A backup now, beside the automatic ones, in this device's backup folder.
    func backUpNow() {
        core.backUpNow { [weak self] failed in self?.failure = failed }
    }

    /// The file name an export of `format` gets, the app's, dated.
    func exportName(_ format: LumennaApp.ExportFormat) -> String {
        exports.first { $0.format == format }?.name ?? "Lumenna"
    }

    /// Asks the app for an export of `format`, handing its file to `saved` once it is made.
    func exporting(_ format: LumennaApp.ExportFormat, saved: @escaping (LumennaApp.SaveFile) -> Void) {
        core.app.file(from: .settings(.export(format)), then: saved)
    }

    /// Reads a JSON export or a backup the person chose; the app tells them apart by its name,
    /// and says what it added.
    func importFile(at url: URL) {
        do {
            let bytes = try Data(contentsOf: url)
            core.app.send(.settings(.import(name: url.lastPathComponent, bytes: bytes)))
        } catch {
            failure = error.localizedDescription
        }
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
    let setting: LumennaApp.Setting

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
            let number = Binding(
                get: { Int(model.values[setting.key] ?? "") ?? 1 },
                set: { model.set(setting.key, String($0)) }
            )
            #if os(macOS)
            // The stepper says its name and number, one stop; the text beside it is for sight.
            // SwiftUI's said "9.1%" before them. Not in `Named`: SwiftUI does not count an
            // AppKit control as a child of its own group, which the audit finds.
            HStack {
                Text(setting.title).accessibilityHidden(true)
                Spacer()
                Text(model.values[setting.key] ?? "").accessibilityHidden(true)
                SpokenStepper(value: number, in: 1...100, name: setting.title) { String($0) }
            }
            #else
            Stepper(value: number, in: 1...100) {
                Text("\(setting.title): \(model.values[setting.key] ?? "")")
            }
            #endif
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
    let setting: LumennaApp.Setting
    @State private var text = ""
    @FocusState private var focused: Bool
    /// Whether a line was just committed, so its reading back replaces it though the field
    /// keeps focus, as a Mac's does after Return.
    @State private var committed = false

    /// Why the line was refused, the app's, which it has said.
    private var problem: String? { model.problems[setting.key] }

    /// Whether the line is long enough to wrap beside its name on a narrow phone (the urgency
    /// weights): then the name goes above it, and it reads from the leading edge.
    private var long: Bool { (model.values[setting.key] ?? setting.value).count > 24 }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Named(setting.title, above: long) {
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
            if !focused || committed, problem == nil { text = value ?? "" }
            committed = false
        }
        #if os(watchOS)
        // A watch's line comes whole from the system's input screen, with no keystrokes and
        // no Return: it is committed as it arrives.
        .onChange(of: text) { _, _ in if text != model.values[setting.key] { commit() } }
        #endif
    }

    /// The field itself, named as `namedField` names one, so focus can be followed. Its
    /// placeholder says what empty means, in a grey that reads: an empty field with no
    /// placeholder left the iPad's audit an empty, unnamed text element whose size is fixed.
    @ViewBuilder private var field: some View {
        #if os(macOS)
        TextField(setting.title, text: $text, prompt: Text("None"))
        #elseif os(iOS)
        // It wraps, so a large text size shows all of it (a one-line field scrolls sideways
        // and is clipped); so Return arrives as a line break, and commits instead.
        TextField("", text: Binding(get: { text }, set: { typed in
            guard typed.contains(where: \.isNewline) else { text = typed; return }
            text = typed.split(whereSeparator: \.isNewline).joined(separator: " ")
            // Leaving the field commits it.
            focused = false
        }), prompt: example("None"), axis: .vertical)
        .multilineTextAlignment(long ? .leading : .trailing)
        #else
        TextField("", text: $text, prompt: example("None"))
        #endif
    }

    private func commit() {
        guard text != model.values[setting.key] else { return }
        committed = true
        model.set(setting.key, text)
    }
}

/// One of the core's settings pages (`SettingsView.pages`): its settings in its order, a choice
/// as a section of its own, its buttons, and its caption beneath, the core's words. What a page
/// holds beyond the settings — the exports, the store's own actions — is drawn on the page the
/// core puts it on.
struct SettingsPageForm: View {
    @ObservedObject var model: SettingsModel
    let id: String

    var body: some View {
        Form {
            if let page = model.page(id) {
                let settings = model.settings(on: page)
                let plain = settings.filter { $0.kind != .choice }
                if !plain.isEmpty || !buttons(page).isEmpty {
                    Section {
                        ForEach(plain, id: \.key) { setting in
                            SettingControl(model: model, setting: setting)
                        }
                        ForEach(buttons(page), id: \.id) { button in
                            Button(named(button)) { press(button) }
                        }
                    } footer: {
                        if !page.caption.isEmpty, id != "export" { FormParts.caption(page.caption) }
                    }
                }
                ForEach(settings.filter { $0.kind == .choice }, id: \.key) { setting in
                    SettingControl(model: model, setting: setting)
                }
                #if !os(watchOS)
                if id == "export" {
                    Section {
                        // Each format's button, the app's words.
                        ForEach(model.exports, id: \.name) { choice in
                            Button(choice.label + "…") { model.export(choice.format) }
                        }
                        ForEach(page.buttons, id: \.id) { button in
                            Button(named(button)) { press(button) }
                        }
                    } footer: {
                        FormParts.caption(page.caption)
                    }
                    StoreActions(model: model)
                }
                #endif
            }
        }
        .modifier(SettingsPage(model: model))
    }

    /// The page's buttons drawn with its settings: a backup's. Syncing and pairing are the
    /// Devices page's own screen, the export page's beside its formats, and a watch has
    /// nowhere to save a file or pick one.
    private func buttons(_ page: LumennaApp.SettingsPage) -> [LumennaApp.SettingsButton] {
        #if os(watchOS)
        []
        #else
        id == "backups" ? page.buttons : []
        #endif
    }

    /// A button's name, ending in "…" on the Mac where it asks something first.
    private func named(_ button: LumennaApp.SettingsButton) -> String {
        #if os(macOS)
        button.asks ? button.title + "…" : button.title
        #else
        button.title
        #endif
    }

    private func press(_ button: LumennaApp.SettingsButton) {
        #if !os(watchOS)
        switch button.id {
        case "back-up": model.backUpNow()
        case "restore-backup", "import": model.importFile()
        default: break
        }
        #endif
    }
}

#if !os(watchOS)
/// What is done to the store as a whole (`SettingsView.offered`), a destructive button each,
/// drawn as the task form's are, beside the backups whose history it reaches, the core's caption
/// beneath. What it does is said again by the app's question, asked before anything is done.
struct StoreActions: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        if !model.offered.isEmpty {
            Section {
                ForEach(model.offered, id: \.self) { action in
                    #if os(macOS)
                    // On the Mac a button that asks ends in "…".
                    WarningButton(action.asksFirst ? action.title + "…" : action.title) { model.act(action) }
                    #else
                    WarningButton(action.title) { model.act(action) }
                    #endif
                }
            } footer: {
                if !model.offeredCaption.isEmpty { FormParts.caption(model.offeredCaption) }
            }
        }
    }
}
#endif
