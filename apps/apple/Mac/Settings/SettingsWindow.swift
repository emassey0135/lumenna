import AppKit
import LumennaApp
import SwiftUI
import UniformTypeIdentifiers

/// Settings, in the Mac's own shape: a window of tabs, opened with ⌘,.
final class SettingsWindowController: NSWindowController {
    private let tabs: NSTabViewController

    init(core: Core) {
        let tabs = NSTabViewController()
        self.tabs = tabs
        tabs.tabStyle = .toolbar
        let model = SettingsModel(core: core)
        // The core's pages, in its order and words; what each draws beyond its settings is this
        // app's: the login item and the shortcuts on General, the device list on Devices.
        tabs.tabViewItems = model.pages.map { page in
            let controller: NSViewController = switch page.id {
            case "general": HostedForm(page.title, rootView: GeneralSettings(model: model))
            // With a status line of its own: what the app says of a device, a refusal included.
            case "devices": StatusLineController(content: DevicesViewController(core: core))
            default: HostedForm(page.title, rootView: SettingsPageForm(model: model, id: page.id))
            }
            let item = NSTabViewItem(viewController: controller)
            item.identifier = page.id
            item.label = page.title
            item.image = NSImage(systemSymbolName: Self.symbol(page.id), accessibilityDescription: page.title)
            return item
        }
        let window = NSWindow(contentViewController: tabs)
        window.title = "Settings"
        window.identifier = NSUserInterfaceItemIdentifier("settings")
        window.styleMask = [.titled, .closable]
        super.init(window: window)
        model.window = window
    }

    /// Each page's toolbar symbol, by what the page is.
    private static func symbol(_ page: String) -> String {
        switch page {
        case "general": "gearshape"
        case "planning": "calendar"
        case "reminders": "bell"
        case "devices": "laptopcomputer.and.iphone"
        case "backups": "externaldrive"
        case "export": "square.and.arrow.up"
        default: "circle"
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    /// Shows the page `id` (the core's), wherever it sits among the tabs.
    func show(_ id: String) {
        let index = tabs.tabView.indexOfTabViewItem(withIdentifier: id)
        if index != NSNotFound { tabs.selectedTabViewItemIndex = index }
    }
}

extension SettingsModel {
    // MARK: - Shortcuts from anywhere

    func shortcutDescription(_ kind: HotKeys.Kind) -> String {
        HotKeys.description(kind)
    }

    func recordShortcut(_ kind: HotKeys.Kind) {
        guard let window else { return }
        HotKeys.record(kind, on: window) { [weak self] in self?.objectWillChange.send() }
    }

    func toggleShortcut(_ kind: HotKeys.Kind) {
        objectWillChange.send()
        HotKeys.set(kind, to: HotKeys.shortcut(kind) == nil ? kind.standard : nil)
        Announcer.say("\(kind.name) is \(HotKeys.description(kind))")
    }

    // MARK: - Opening at login

    /// Opening at login, as the app offers it among this device's settings, in its words: the
    /// system holds its value, which the app asks it to change (`LoginItem`).
    var openingAtLogin: LumennaApp.Setting? {
        settings.first { $0.key == "background" }
    }

    // MARK: - Files

    func chooseBackupFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Back Up Here"
        panel.message = "Backups hold your whole history, including everything you deleted."
        guard let window else { return }
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            BackupFolder.chosen(url)
            self.set("backup-dir", url.path)
        }
    }

    /// Writes an export, as the app makes it, where the person chooses. It holds the present
    /// state only, nothing from the trash.
    func export(_ format: LumennaApp.ExportFormat) {
        let panel = NSSavePanel()
        panel.nameFieldStringValue = exportName(format)
        panel.canCreateDirectories = true
        guard let window else { return }
        panel.beginSheetModal(for: window) { response in
            guard response == .OK, let url = panel.url else { return }
            self.exporting(format) { made in
                do {
                    // The panel already asked about replacing an existing file.
                    try Data(made.bytes).write(to: url)
                } catch {
                    self.failure = error.localizedDescription
                }
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
            self.importFile(at: url)
        }
    }

}

struct GeneralSettings: View {
    @ObservedObject var model: SettingsModel
    /// The shortcuts' words, the core's. Its caption is about a PC's keys, so the Mac keeps its
    /// own, about Control-Command and VoiceOver.
    private let words = LumennaApp.hotkeyWords()

    var body: some View {
        Form {
            if let login = model.openingAtLogin {
                Section {
                    Labelled(login.title) {
                        Toggle(login.title, isOn: Binding(
                            get: { model.values[login.key] == "true" },
                            set: { model.set(login.key, $0 ? "true" : "false") }
                        ))
                    }
                    FormParts.caption(login.hint)
                }
            }
            Section {
                ForEach(HotKeys.Kind.allCases, id: \.self) { kind in
                    Named(kind.name) {
                        HStack {
                            Text(model.shortcutDescription(kind))
                            Button(words.change + "…") { model.recordShortcut(kind) }
                                .accessibilityLabel("\(words.change), \(kind.name)")
                            let toggle = HotKeys.shortcut(kind) == nil ? words.turnOn : words.turnOff
                            Button(toggle) { model.toggleShortcut(kind) }
                                .accessibilityLabel("\(toggle), \(kind.name)")
                        }
                    }
                }
            } header: {
                FormParts.heading(words.heading)
            } footer: {
                Text("These work in any app, so they take their keys from whatever app is in front. Control-Command, because Control-Option is VoiceOver's.")
                    .font(.footnote).foregroundStyle(Color.quietLabel)
            }
        }
        .modifier(SettingsPage(model: model))
    }
}
