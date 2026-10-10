import UIKit

/// A row's actions as the iPhone and iPad offer them, from the core's list, each once:
///
/// - the `primary` ones as swipe actions, which UIKit also offers to VoiceOver, Switch Control
///   and Full Keyboard Access as the row's actions;
/// - the rest as the cell's accessibility custom actions, which UIKit adds to the swipe
///   actions rather than replacing them, so between the two every action is listed once;
/// - all of them, in the core's order, in the menu a long press (or a secondary click) opens,
///   for touch, where nothing else reaches what is not swiped.
///
/// Expand or Collapse is the app's own, not the core's: a swipe action, and last in the menu.
struct RowActions {
    let actions: [Action]
    /// Whether Mark Done or Mark Not Done, when first, swipes from the leading edge, as a
    /// task's does.
    var doneLeads = false
    /// Expand or Collapse, for a row with something under it.
    var fold: UIContextualAction?
    let run: (Action) -> Void

    /// What swipes from the leading edge: the row's Mark Done, if it leads.
    var leading: [Action] {
        guard doneLeads, let first = actions.first, first.primary, [.markDone, .markNotDone].contains(first.kind) else {
            return []
        }
        return [first]
    }

    /// What swipes from the trailing edge: the other primary actions, in the core's order.
    var trailing: [Action] {
        let lead = leading
        return actions.filter { $0.primary && !lead.contains($0) }
    }

    /// What is offered only as an accessibility action, besides the menu.
    var others: [Action] { actions.filter { !$0.primary } }

    func leadingSwipe() -> UISwipeActionsConfiguration? {
        let swipes = leading.map(swipe)
        return swipes.isEmpty ? nil : UISwipeActionsConfiguration(actions: swipes)
    }

    func trailingSwipe() -> UISwipeActionsConfiguration? {
        let swipes = trailing.map(swipe) + [fold].compactMap { $0 }
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

    /// The long-press menu: every action, in the core's order, then Expand or Collapse.
    var menu: UIMenu? {
        var items: [UIMenuElement] = actions.map { action in
            UIAction(title: action.title, attributes: action.destructive ? .destructive : []) { _ in run(action) }
        }
        if let fold, let title = fold.title {
            items.append(UIAction(title: title) { _ in fold.handler(fold, UIView()) { _ in } })
        }
        return items.isEmpty ? nil : UIMenu(children: items)
    }

    /// The menu as a collection view's context menu asks for it.
    var contextMenu: UIContextMenuConfiguration? {
        guard let menu else { return nil }
        return UIContextMenuConfiguration(actionProvider: { _ in menu })
    }

    /// A swipe action for one of the core's actions: its spoken name, shown as destructive
    /// when it removes something.
    private func swipe(_ action: Action) -> UIContextualAction {
        let swipe = UIContextualAction(style: action.destructive ? .destructive : .normal, title: action.title) { _, _, finished in
            run(action)
            finished(true)
        }
        if action.kind == .markDone || action.kind == .markNotDone { swipe.backgroundColor = .systemGreen }
        return swipe
    }
}
