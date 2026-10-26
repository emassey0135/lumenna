import UIKit

extension UIViewController {
    /// Says that something could not be done, with the core's own sentence: under `title`, the
    /// question's own, or as the whole alert, which the sentence says by itself.
    func showFailure(_ message: String, title: String? = nil) {
        let alert = UIAlertController(title: title ?? message, message: title == nil ? nil : message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        (presentedViewController ?? self).present(alert, animated: true)
    }
}

extension Core {
    /// Takes a backup if one is due, saying so over `presenter` if it fails.
    func backUpIfDue(presentingFrom presenter: UIViewController?) {
        backUpIfDue { [weak presenter] message in presenter?.showFailure(message) }
    }
}
