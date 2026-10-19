import LumennaApp
import UIKit

/// A row's actions as the iPhone and iPad offer them, from the app's list, each once — the same
/// rules as `RowActions`, over the app's actions:
///
/// - the `primary` ones, and Expand or Collapse, as swipe actions, which UIKit also offers to
///   VoiceOver, Switch Control and Full Keyboard Access as the row's actions;
/// - the rest as the cell's accessibility custom actions, which UIKit adds to the swipe actions
///   rather than replacing them, so between the two every action is listed once;
/// - all of them, in the app's order, in the menu a long press (or a secondary click) opens.
///
/// Running one is the app's: `run` puts the row in hand and sends the action.
struct AppRowActions {
    let actions: [LumennaApp.Action]
    /// Whether Mark Done or Mark Not Done, when first, swipes from the leading edge, as a
    /// task's does.
    var doneLeads = false
    let run: (LumennaApp.Action) -> Void

    /// Expand and Collapse: the app's, swiped, since folding is a row's commonest move.
    private static let folds: [LumennaApp.ActionKind] = [.expand, .collapse]

    /// What swipes from the leading edge: the row's Mark Done, if it leads.
    var leading: [LumennaApp.Action] {
        guard doneLeads, let first = actions.first, first.primary, [.markDone, .markNotDone].contains(first.kind) else {
            return []
        }
        return [first]
    }

    /// What swipes from the trailing edge: the other primary actions, in the app's order, then
    /// Expand or Collapse.
    var trailing: [LumennaApp.Action] {
        let lead = leading
        let primary = actions.filter { $0.primary && !lead.contains($0) && !Self.folds.contains($0.kind) }
        return primary + actions.filter { Self.folds.contains($0.kind) }
    }

    /// What is offered only as an accessibility action, besides the menu.
    var others: [LumennaApp.Action] {
        let swiped = leading + trailing
        return actions.filter { !swiped.contains($0) }
    }

    func leadingSwipe() -> UISwipeActionsConfiguration? {
        let swipes = leading.map(swipe)
        return swipes.isEmpty ? nil : UISwipeActionsConfiguration(actions: swipes)
    }

    func trailingSwipe() -> UISwipeActionsConfiguration? {
        let swipes = trailing.map(swipe)
        return swipes.isEmpty ? nil : UISwipeActionsConfiguration(actions: swipes)
    }

    /// The cell's custom actions: every action not swiped.
    var customActions: [UIAccessibilityCustomAction] {
        others.map { action in
            UIAccessibilityCustomAction(name: action.title) { _ in
                run(action)
                return true
            }
        }
    }

    /// The long-press menu: every action, in the app's order.
    var menu: UIMenu? {
        let items: [UIMenuElement] = actions.map { action in
            UIAction(title: action.title, attributes: action.destructive ? .destructive : []) { _ in run(action) }
        }
        return items.isEmpty ? nil : UIMenu(children: items)
    }

    /// The menu as a collection view's context menu asks for it.
    var contextMenu: UIContextMenuConfiguration? {
        guard let menu else { return nil }
        return UIContextMenuConfiguration(actionProvider: { _ in menu })
    }

    /// The first of these kinds: what a key or a menu command does to the row in hand.
    func first(_ kinds: LumennaApp.ActionKind...) -> LumennaApp.Action? {
        actions.first { kinds.contains($0.kind) }
    }

    /// A swipe action for one of the app's actions: its spoken name, shown as destructive when it
    /// removes something.
    private func swipe(_ action: LumennaApp.Action) -> UIContextualAction {
        let swipe = UIContextualAction(style: action.destructive ? .destructive : .normal, title: action.title) { _, _, finished in
            run(action)
            finished(true)
        }
        switch action.kind {
        case .markDone, .markNotDone: swipe.backgroundColor = .systemGreen
        case .expand, .collapse: swipe.backgroundColor = .lumennaTint
        default: break
        }
        return swipe
    }
}

extension AppModel {
    /// What running one of a row's actions sends: the row in hand, then the action. A row of a
    /// list beside the main one (`side`: the places, a reminder list, the devices) is held
    /// there instead, so the change lands back in that list.
    func run(_ action: LumennaApp.Action, on key: String, side: LumennaApp.Side? = nil) {
        if let side {
            send(.hold(field0: side, field1: key))
        } else {
            send(.focus(key))
        }
        send(.perform(action))
    }

    /// A row's speech after its title: its line's detail, whether what is under it is folded
    /// away, and the level where it changes from the row shown before — what a tree control
    /// would say by itself, and a list cell does not.
    static func spoken(detail: String?, expanded: Bool?, level: String?) -> String {
        [detail.flatMap { $0.isEmpty ? nil : $0 }, expanded.map { $0 ? "expanded" : "collapsed" }, level]
            .compactMap { $0 }
            .joined(separator: ", ")
    }
}

extension UIViewController {
    /// Opens the app's own form for `action` (`Question.form`), asked for by the app's question
    /// open: a task's details, a reminder list. Beside the list on iPad, pushed on iPhone.
    func openForm(_ action: LumennaApp.Action, core: Core) {
        switch action.kind {
        case .reminders:
            openReminders(of: action.target, core: core)
        case .edit where action.subject == .task, .editTask:
            showBeside(TaskDetailViewController(core: core, id: action.target))
        default:
            break
        }
    }
}
