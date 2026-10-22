import LumennaApp

/// A row's actions as the iPhone, the iPad and the watch offer them, from the app's list, each
/// once — the same rules as `RowActions`, over the app's actions:
///
/// - the `primary` ones, and Expand or Collapse, as swipe actions, which SwiftUI also offers to
///   VoiceOver, Switch Control and Full Keyboard Access as the row's actions;
/// - the rest as the row's accessibility actions, which are added to the swipe actions rather
///   than replacing them, so between the two every action is listed once;
/// - all of them, in the app's order, in the menu a long press (or a secondary click) opens.
///
/// Running one is the app's: `run` puts the row in hand and sends the action. The actions are
/// whole (`Action`) or, for a list's rows, in short (`Offered`).
struct AppRowActions<A: RowAction> {
    let actions: [A]
    /// Whether Mark Done or Mark Not Done, when first, swipes from the leading edge, as a
    /// task's does.
    var doneLeads = false
    let run: (A) -> Void

    /// Expand and Collapse: the app's, swiped, since folding is a row's commonest move.
    private static var folds: [LumennaApp.ActionKind] { [.expand, .collapse] }

    /// What swipes from the leading edge: the row's Mark Done, if it leads.
    var leading: [A] {
        guard doneLeads, let first = actions.first, first.primary, [.markDone, .markNotDone].contains(first.kind) else {
            return []
        }
        return [first]
    }

    /// What swipes from the trailing edge: the other primary actions, in the app's order, then
    /// Expand or Collapse.
    var trailing: [A] {
        let lead = leading
        let primary = actions.filter { $0.primary && !lead.contains($0) && !Self.folds.contains($0.kind) }
        return primary + actions.filter { Self.folds.contains($0.kind) }
    }

    /// What is offered only as an accessibility action, besides the menu.
    var others: [A] {
        let swiped = leading + trailing
        return actions.filter { !swiped.contains($0) }
    }

    /// The first of these kinds: what a key or a menu command does to the row in hand.
    func first(_ kinds: LumennaApp.ActionKind...) -> A? {
        actions.first { kinds.contains($0.kind) }
    }
}
