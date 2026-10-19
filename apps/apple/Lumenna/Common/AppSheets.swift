import LumennaApp
import SwiftUI
import UIKit

/// The app's block form, in a navigation sheet with Cancel and the app's Save or Add in its bar.
final class AppBlockFormViewController: UIHostingController<AppBlockForm> {
    init(model: AppBlockFormModel) {
        super.init(rootView: AppBlockForm(model: model))
        title = model.heading
        // UIKit buttons, Save always enabled: a disabled one fails contrast, and saving a
        // block with no name says why it cannot.
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { _ in model.cancel() }
        )
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: model.yes, primaryAction: UIAction { _ in model.save() }
        )
        navigationItem.rightBarButtonItem?.style = .done
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }
}

/// The app's Replan from Now or Plan the Day, in a navigation sheet with Cancel in its bar.
final class AppProposalViewController: UIHostingController<AppProposalForm> {
    init(model: AppProposalModel) {
        super.init(rootView: AppProposalForm(model: model))
        title = model.title
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            title: model.proposal?.words.cancel ?? "Cancel", primaryAction: UIAction { _ in model.cancel() }
        )
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }
}

/// The screens the app opens by itself — the block form, a proposal, a reminder list — shown
/// when the app opens one, and closed when the app lets it go. One per window, at its root,
/// beside `AppQuestions`.
final class AppSheets {
    private let core: Core
    private weak var root: UIViewController?
    private weak var blockForm: UIViewController?
    private weak var proposal: UIViewController?
    private weak var reminders: UIViewController?
    /// Shows a reminder list where the window shows what was opened from a list: pushed, or
    /// beside it on iPad.
    private let showReminders: (UIViewController) -> Void
    private var observer: NSObjectProtocol?
    /// What each sheet swiped away tells the app: it was put away.
    private let blockGone: SheetGone
    private let proposalGone: SheetGone

    init(core: Core, root: UIViewController, showReminders: @escaping (UIViewController) -> Void) {
        self.showReminders = showReminders
        blockGone = SheetGone { core.app.send(.block(.cancel)) }
        proposalGone = SheetGone { core.app.send(.proposal(.close)) }
        self.core = core
        self.root = root
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: core.app, queue: .main) { [weak self] _ in
            self?.update()
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    /// What is in front, so a sheet opens over a question's alert gone or a sheet already up.
    private var front: UIViewController? {
        var shown = root
        while let next = shown?.presentedViewController, !next.isBeingDismissed { shown = next }
        return shown
    }

    private func update() {
        let view = core.app.view
        if view.blockForm != nil, blockForm == nil {
            let model = AppBlockFormModel(core: core)
            let navigation = UINavigationController(rootViewController: AppBlockFormViewController(model: model))
            model.close = { [weak navigation] in
                guard let navigation, navigation.presentingViewController != nil, !navigation.isBeingDismissed else { return }
                navigation.dismiss(animated: true)
            }
            blockForm = navigation
            front?.present(navigation, animated: true)
            navigation.presentationController?.delegate = blockGone
        }
        if view.reminders != nil, reminders == nil {
            let list = RemindersViewController(core: core)
            reminders = list
            showReminders(list)
        }
        if view.proposal != nil, proposal == nil {
            let model = AppProposalModel(core: core)
            let navigation = UINavigationController(rootViewController: AppProposalViewController(model: model))
            model.close = { [weak navigation] in
                guard let navigation, navigation.presentingViewController != nil, !navigation.isBeingDismissed else { return }
                navigation.dismiss(animated: true)
            }
            // Over the whole iPad, not a form sheet: the day's rows half under a form sheet read
            // to the audit as text cut off, and a proposal for the whole day is a screen's worth.
            if root?.traitCollection.userInterfaceIdiom == .pad { navigation.modalPresentationStyle = .fullScreen }
            proposal = navigation
            front?.present(navigation, animated: true)
            navigation.presentationController?.delegate = proposalGone
        }
    }
}

/// A sheet swiped away, which is the same as its Cancel.
private final class SheetGone: NSObject, UIAdaptivePresentationControllerDelegate {
    private let gone: () -> Void

    init(_ gone: @escaping () -> Void) {
        self.gone = gone
    }

    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        gone()
    }
}
