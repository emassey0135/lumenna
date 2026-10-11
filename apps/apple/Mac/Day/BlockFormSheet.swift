import AppKit
import SwiftUI

extension BlockFormModel {
    /// Shows the shared block form (`Shared/Forms/BlockForm.swift`) as a sheet on `window`.
    func present(on window: NSWindow, saved: @escaping (Change) -> Void) {
        self.saved = saved
        let host = HostedForm(heading, rootView: BlockForm(model: self))
        let sheet = NSWindow(contentViewController: host)
        sheet.title = heading
        close = { [weak window, weak sheet] in
            if let sheet { window?.endSheet(sheet) }
        }
        window.beginSheet(sheet)
    }
}

extension ProposalModel {
    /// Shows Replan from Now or Plan the Day (`Shared/Forms/ProposalForm.swift`) as a sheet on
    /// `window`, open until nothing more is proposed or it is cancelled.
    func present(on window: NSWindow) {
        let host = HostedForm(words.title, rootView: ProposalForm(model: self))
        let sheet = NSWindow(contentViewController: host)
        sheet.title = words.title
        close = { [weak window, weak sheet] in
            if let sheet { window?.endSheet(sheet) }
        }
        window.beginSheet(sheet)
    }
}
