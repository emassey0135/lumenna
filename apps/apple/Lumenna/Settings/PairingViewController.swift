import LumennaApp
import SwiftUI
import UIKit

/// Pairing this phone with another of the person's devices: the shared pairing screen
/// (`PairingScreen` over `PairingModel`, the Mac's too) under UIKit's bar, pushed from Devices.
final class PairingViewController: UIHostingController<PairingScreen> {
    private let model: PairingModel

    init(core: Core) {
        let model = PairingModel(core: core, thisDevice: "this \(UIDevice.current.model)", name: UIDevice.current.name)
        self.model = model
        super.init(rootView: PairingScreen(model: model))
        title = model.words.title
        model.failed = { [weak self] message in self?.showFailure(message) }
        model.finished = { [weak self] in
            guard let self, self.navigationController?.topViewController === self else { return }
            self.navigationController?.popViewController(animated: true)
        }
        model.ask = { [weak self] words, answer in self?.ask(words, answer: answer) }
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        model.open()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        // Leaving the screen gives up, which ends the wait for the other device, then the screen.
        if isMovingFromParent { model.cancel() }
    }

    /// Asks whether the words match, in the system's alert. The pairing waits for the answer.
    private func ask(_ words: [String], answer: @escaping (Bool) -> Void) {
        let said = model.words
        let alert = UIAlertController(
            title: said.matchTitle,
            message: "\(said.matchMessage) \(words.joined(separator: ", "))",
            preferredStyle: .alert
        )
        alert.addAction(UIAlertAction(title: said.matchNo, style: .cancel) { _ in answer(false) })
        alert.addAction(UIAlertAction(title: said.matchYes, style: .default) { _ in answer(true) })
        present(alert, animated: true)
    }
}
