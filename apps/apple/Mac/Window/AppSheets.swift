import AppKit
import LumennaApp
import SwiftUI

/// The screens the app opens by itself — the block form, a proposal, a reminder list — as
/// sheets on the main window, shown when the app opens one and closed when the app lets it go.
/// Each is the shared SwiftUI form (`Shared/Forms`), with its own Cancel or Done.
final class AppSheets {
    private let core: Core
    private weak var window: NSWindow?
    private weak var blockForm: NSWindow?
    private weak var proposal: NSWindow?
    private weak var reminders: NSWindow?
    private var observer: NSObjectProtocol?

    init(core: Core, window: NSWindow) {
        self.core = core
        self.window = window
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.update()
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    private func update() {
        guard let window else { return }
        let view = core.app.view
        if view.blockForm != nil, blockForm == nil {
            let model = AppBlockFormModel(core: core)
            blockForm = present(HostedForm(model.heading, rootView: AppBlockForm(model: model)), on: window) { model.close = $0 }
        }
        if view.proposal != nil, proposal == nil {
            let model = AppProposalModel(core: core)
            proposal = present(HostedForm(model.title, rootView: AppProposalForm(model: model)), on: window) { model.close = $0 }
        }
        if view.reminders != nil, reminders == nil {
            let model = RemindersModel(core: core)
            reminders = present(HostedForm("Reminders", rootView: RemindersForm(model: model)), on: window) { model.close = $0 }
        }
    }

    /// Shows `form` as a sheet on `window`, handing its model the way to close it.
    private func present(_ form: NSViewController, on window: NSWindow, closing: (@escaping () -> Void) -> Void) -> NSWindow {
        let sheet = NSWindow(contentViewController: form)
        sheet.title = form.title ?? ""
        closing { [weak sheet] in
            if let sheet, let parent = sheet.sheetParent { parent.endSheet(sheet) }
        }
        window.front.beginSheet(sheet)
        return sheet
    }
}

extension NSWindow {
    /// The sheet in front on this window, or the window itself: where a question asked from a
    /// sheet goes, since a second sheet on the window would wait for the first to close.
    var front: NSWindow {
        var shown = self
        while let sheet = shown.attachedSheet { shown = sheet }
        return shown
    }
}
