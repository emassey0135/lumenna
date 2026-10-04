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
