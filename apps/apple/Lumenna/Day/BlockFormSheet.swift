import SwiftUI
import UIKit

/// The block editor (§16.1): the shared form (`Shared/Forms/BlockForm.swift`), in a navigation
/// sheet with Cancel and Save in its bar.
final class BlockFormViewController: UIHostingController<BlockForm> {
    init(model: BlockFormModel) {
        super.init(rootView: BlockForm(model: model))
        title = model.heading
        // UIKit buttons, Save always enabled: a disabled one failed contrast, and saving a
        // block with no name says why it cannot.
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { _ in model.close() }
        )
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: "Save", primaryAction: UIAction { _ in model.save() }
        )
        navigationItem.rightBarButtonItem?.style = .done
    }

    @available(*, unavailable)
    required dynamic init?(coder: NSCoder) { fatalError("not used") }
}

extension UIViewController {
    /// Shows a block form in a sheet of its own, closing it when it saves or is cancelled.
    func presentBlockForm(_ model: BlockFormModel) {
        let form = BlockFormViewController(model: model)
        let navigation = UINavigationController(rootViewController: form)
        model.close = { [weak navigation] in navigation?.dismiss(animated: true) }
        present(navigation, animated: true)
    }
}

/// Choosing a day to go to: the system's own calendar, in a sheet.
final class DayPickerViewController: UIViewController {
    private let picker = UIDatePicker()
    private let chosen: (Date) -> Void

    init(showing day: Date, chosen: @escaping (Date) -> Void) {
        self.chosen = chosen
        super.init(nibName: nil, bundle: nil)
        title = "Go to Day"
        picker.date = day
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        picker.datePickerMode = .date
        picker.preferredDatePickerStyle = .inline
        picker.tintColor = .lumennaTint
        picker.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(picker)
        NSLayoutConstraint.activate([
            picker.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            picker.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor),
            picker.trailingAnchor.constraint(equalTo: view.layoutMarginsGuide.trailingAnchor),
        ])
        navigationItem.leftBarButtonItem = UIBarButtonItem(
            systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) }
        )
        let go = UIBarButtonItem(title: "Go", primaryAction: UIAction { [weak self] _ in
            guard let self else { return }
            let day = self.picker.date
            let chosen = self.chosen
            self.dismiss(animated: true) { chosen(day) }
        })
        go.style = .done
        navigationItem.rightBarButtonItem = go
    }
}
