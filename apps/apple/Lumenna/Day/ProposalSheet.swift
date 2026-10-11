import SwiftUI
import UIKit

/// Replan from Now or Plan the Day: the shared screen (`Shared/Forms/ProposalForm.swift`) in a
/// navigation sheet, Cancel in its bar, the core's words throughout.
final class ProposalViewController: UIHostingController<ProposalForm> {
    let model: ProposalModel

    init(model: ProposalModel) {
        self.model = model
        super.init(rootView: ProposalForm(model: model))
        title = model.words.title
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            title: model.words.cancel, primaryAction: UIAction { _ in model.close() }
        )
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }
}

extension UIViewController {
    /// Opens the screen a day's Replan from Now or Plan the Day asks for (`Question.form`),
    /// over whatever is in front. `applied` hears each change made, while it stays open.
    func presentProposal(for action: Action, core: Core, applied: @escaping (Change) -> Void = { _ in }) {
        do {
            guard let model = try ProposalModel(core: core, action: action) else { return }
            presentProposal(model, applied: applied)
        } catch {
            showFailure(error.sentence)
        }
    }

    /// Opens a proposal's screen in a sheet of its own, over whatever is in front.
    func presentProposal(_ model: ProposalModel, applied: @escaping (Change) -> Void = { _ in }) {
        let screen = ProposalViewController(model: model)
        let navigation = UINavigationController(rootViewController: screen)
        model.close = { [weak navigation] in navigation?.dismiss(animated: true) }
        model.applied = applied
        var front: UIViewController = self
        while let next = front.presentedViewController, !next.isBeingDismissed { front = next }
        front.present(navigation, animated: true)
    }
}
