import UIKit

extension UIViewController {
    /// Says that something could not be done, with the core's own sentence.
    func showFailure(_ message: String, title: String = "Could not do that") {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        (presentedViewController ?? self).present(alert, animated: true)
    }
}

extension Core {
    /// Takes a backup if one is due, saying so over `presenter` if it fails.
    func backUpIfDue(presentingFrom presenter: UIViewController?) {
        backUpIfDue { [weak presenter] message in presenter?.showFailure(message, title: "Backup") }
    }
}
