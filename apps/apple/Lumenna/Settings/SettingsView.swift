import LumennaApp
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Settings: a short list of pages, the core's — what syncs, what is this device's alone, and
/// getting data in and out.
///
/// Pages rather than one long form: a VoiceOver user reaches backups without swiping through
/// every weekday, and a page pushed over the tab bar hides it, so no row is ever read through
/// its glass — which the accessibility audit, rightly, cannot tell from unreadable text.
final class SettingsViewController: AppListViewController {
    init(core: Core) {
        super.init(core: core, title: "Settings")
    }

    // Each page has its own; this list is somewhere to go.
    override var offersUndo: Bool { false }

    override func appear() {
        // How syncing is going, the network's, for the Devices row; and the settings, so a page
        // with none this device has is left out.
        core.app.send(.devices(.show))
        core.app.send(.settings(.open))
    }

    /// The core's pages, in its order and words, those with something here: General holds what
    /// a platform keeps running in the background, which an iPhone does not offer.
    private var pages: [LumennaApp.SettingsPage] {
        let view = core.app.view.settings
        let here = Set(view?.settings.map(\.setting.key) ?? [])
        return (view?.pages ?? LumennaApp.settingsPages(ownClock: true, sentence: false)).filter { page in
            !page.buttons.isEmpty || page.id == "export" || page.keys.contains(where: here.contains)
        }
    }

    override func rows() -> (items: [AppItem], count: String)? {
        let status = core.app.view.devices?.status ?? ""
        return (pages.map { page in
            // The Devices row says how syncing is going, rather than what the page is.
            let detail = page.id == "devices" && !status.isEmpty ? status : page.description
            return AppItem(key: page.id, title: page.title, detail: detail.isEmpty ? nil : detail)
        }, "")
    }

    override func open(_ item: AppItem) {
        let next: UIViewController = item.key == "devices"
            ? DevicesViewController(core: core)
            : SettingsPageViewController(core: core, page: item.key, title: item.title)
        showBeside(next)
    }
}

final class SettingsPageViewController: UIHostingController<SettingsPageForm> {
    private let model: SettingsModel

    /// The page `page` (the core's id), titled `title`.
    init(core: Core, page: String, title: String) {
        let model = SettingsModel(core: core)
        self.model = model
        super.init(rootView: SettingsPageForm(model: model, id: page))
        self.title = title
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
    /// Writes an export, as the app makes it, to a file named for the day and offers it to
    /// share — AirDrop, Files, Mail. It holds the present state only, nothing from the trash.
    func export(_ format: LumennaApp.ExportFormat) {
        exporting(format) { [weak self] made in
            guard let self else { return }
            let file = FileManager.default.temporaryDirectory.appendingPathComponent(made.name)
            do {
                try Data(made.bytes).write(to: file)
                let share = UIActivityViewController(activityItems: [file], applicationActivities: nil)
                share.popoverPresentationController?.sourceView = self.host?.view
                self.host?.present(share, animated: true)
            } catch {
                self.failure = error.localizedDescription
            }
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
        importFile(at: url)
    }
}
