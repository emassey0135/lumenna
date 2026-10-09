import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Settings: a short list of pages — what syncs, what is this device's alone, and getting
/// data in and out.
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
        showBeside(next)
    }
}

/// One page of settings.
enum SettingsTopic {
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

    init(core: Core, page: SettingsTopic) {
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

extension SettingsModel: UIDocumentPickerDelegate {
    /// Writes an export to a file named for the day and offers it to share — AirDrop, Files,
    /// Mail. It holds the present state only, nothing from the trash.
    func export(_ format: ExportFormat) {
        let file = FileManager.default.temporaryDirectory.appendingPathComponent(Self.exportName(format))
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
    func importFile() {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.json, .data, .item], asCopy: true)
        picker.delegate = self
        host?.present(picker, animated: true)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else { return }
        do {
            imported(try core.lumenna.import(path: url.path))
        } catch {
            failure = error.sentence
        }
    }
}

/// One page, from the shared ones in `Shared/Forms/Settings.swift`.
struct SettingsView: View {
    @ObservedObject var model: SettingsModel
    let page: SettingsTopic

    var body: some View {
        switch page {
        case .planning: PlanningSettings(model: model)
        case .backups: BackupSettings(model: model)
        case .export: ExportSettings(model: model)
        }
    }
}
