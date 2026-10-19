import AppKit
import LumennaApp
import SwiftUI

/// Pairing this Mac with another of the person's devices: the shared pairing screen
/// (`PairingScreen` over `PairingModel`) in a sheet, hosted by a view that steps out of
/// VoiceOver's tree.
final class PairingSheet: NSViewController {
    private let model: PairingModel

    private init(core: Core) {
        model = PairingModel(core: core, thisDevice: "this Mac", name: Host.current().localizedName ?? "Mac")
        super.init(nibName: nil, bundle: nil)
        title = model.words.title
        model.failed = { [weak self] message in self?.view.window?.showFailure(message) }
        model.finished = { [weak self] in self?.endSheet() }
        model.ask = { [weak self] words, answer in self?.ask(words, answer: answer) }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    static func present(on window: NSWindow, core: Core) {
        let screen = PairingSheet(core: core)
        let sheet = NSWindow(contentViewController: screen)
        sheet.title = screen.model.words.title
        window.beginSheet(sheet)
    }

    override func loadView() {
        view = FlatHostingView(rootView: PairingScreen(model: model, close: { [weak self] in self?.endSheet() }))
        model.open()
    }

    /// Asks whether the words match. The pairing waits for the answer.
    private func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        guard let window = view.window else { return answer(false) }
        let alert = NSAlert()
        let said = model.words
        alert.messageText = said.matchTitle
        alert.informativeText = "\(said.matchMessage) \(words.joined(separator: ", "))"
        // No is first, so the default: a stray Return never pairs.
        alert.addButton(withTitle: said.matchNo)
        alert.addButton(withTitle: said.matchYes)
        alert.beginSheetModal(for: window) { response in answer(response == .alertSecondButtonReturn) }
    }

    private func endSheet() {
        guard let sheet = view.window, let parent = sheet.sheetParent else { return }
        parent.endSheet(sheet)
    }
}
