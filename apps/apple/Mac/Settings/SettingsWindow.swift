import AppKit
import LumennaApp
import SwiftUI
import UniformTypeIdentifiers

/// Settings, in the Mac's own shape: a window of tabs, opened with ⌘,.
final class SettingsWindowController: NSWindowController {
    /// A page, by what it is rather than where it sits among the tabs.
    enum Page: String {
        case general, planning, devices, backups, export
    }

    private let tabs = NSTabViewController()

    init(core: Core) {
        let tabs = tabs
        tabs.tabStyle = .toolbar
        let model = SettingsModel(core: core)
        func tab(_ controller: NSViewController, _ label: String, _ symbol: String, _ page: Page) -> NSTabViewItem {
            let item = NSTabViewItem(viewController: controller)
            item.identifier = page.rawValue
            item.label = label
            item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
            return item
        }
        tabs.tabViewItems = [
            tab(HostedForm("General", rootView: GeneralSettings(model: model)), "General", "gearshape", .general),
            tab(HostedForm("Planning", rootView: PlanningSettings(model: model)), "Planning", "calendar", .planning),
            // With a status line of its own: what the app says of a device, a refusal included.
            tab(StatusLineController(content: DevicesViewController(core: core)), "Devices", "laptopcomputer.and.iphone", .devices),
            tab(HostedForm("Backups", rootView: BackupSettings(model: model)), "Backups", "externaldrive", .backups),
            tab(HostedForm("Export and Import", rootView: ExportSettings(model: model)), "Export and Import", "square.and.arrow.up", .export),
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

    /// Shows `page`, wherever it sits among the tabs.
    func show(_ page: Page) {
        let index = tabs.tabView.indexOfTabViewItem(withIdentifier: page.rawValue)
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
                            Button("Change…") { model.recordShortcut(kind) }
                                .accessibilityLabel("Change shortcut for \(kind.name)")
                            Button(model.shortcutDescription(kind) == "Off" ? "Turn On" : "Turn Off") {
                                model.toggleShortcut(kind)
                            }
                            .accessibilityLabel("\(model.shortcutDescription(kind) == "Off" ? "Turn on" : "Turn off") shortcut for \(kind.name)")
                        }
                    }
                }
            } header: {
                FormParts.heading("Shortcuts from Anywhere")
            } footer: {
                Text("These work in any app, so they take their keys from whatever app is in front. Control-Command, because Control-Option is VoiceOver's.")
                    .font(.footnote).foregroundStyle(Color.quietLabel)
            }
        }
        .modifier(SettingsPage(model: model))
    }
}
