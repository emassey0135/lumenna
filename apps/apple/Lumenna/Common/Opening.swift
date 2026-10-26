import LumennaApp
import UIKit

extension UIViewController {
    /// Opens the app's own form for `action` (`Question.form`), asked for by the app's question
    /// open: a task's details, beside the list on iPad, pushed on iPhone. A reminder list is the
    /// app's to open (`AppSheets`), never asked.
    func openForm(_ action: LumennaApp.Action, core: Core) {
        switch action.kind {
        case .edit where action.subject == .task, .editTask:
            showBeside(TaskDetailViewController(core: core, id: action.target))
        default:
            break
        }
    }
}


extension UIAccessibility {
    /// The view VoiceOver is on, or the view its element is inside: VoiceOver can report an
    /// element that is not a view (the iPad's did, for a list cell), found through its
    /// containers. Taken as only a view, ⌘K on the iPad did nothing to VoiceOver's row.
    static var voiceOverView: UIView? {
        var current = focusedElement(using: .notificationVoiceOver)
        for _ in 0..<8 {
            if let view = current as? UIView { return view }
            current = (current as? UIAccessibilityElement)?.accessibilityContainer
        }
        return nil
    }
}
