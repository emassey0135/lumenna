import LumennaApp
import UIKit

/// The screen's own actions (`ScreenAction`, the core's, `Shared/Screens/ScreenActions.swift`)
/// in a navigation bar, on the iPhone and the iPad: Undo and Redo leading, as the system's
/// arrows; New Task or New Block trailing, as a plus; each named by the core's title for
/// VoiceOver and the Large Content Viewer, and dimmed while it does nothing. Icons, not words:
/// words would take the bar's room from the title, which is then clipped at large text sizes. Never
/// a bottom toolbar, which the floating tab bar covers. What a bar holds no icon for is the
/// screen's to draw: the day's buttons, or a list's Place Actions menu here.
enum ScreenActionBar {
    /// The symbol a bar shows for the action `id`, and whether it leads; nil for one the bar
    /// holds no icon for.
    static func icon(_ id: String) -> (symbol: String, leading: Bool)? {
        switch id {
        case "undo": ("arrow.uturn.backward", true)
        case "redo": ("arrow.uturn.forward", true)
        case "new-task", "new-block": ("plus", false)
        default: nil
        }
    }

    /// Whether the bar holds `action` as an icon.
    static func holds(_ action: LumennaApp.ScreenAction) -> Bool {
        icon(action.id) != nil
    }

    /// Puts `actions` in `item`'s bar, in the core's order on each side, and, where `menu` is
    /// set, the rest in a Place Actions menu after them; each pressed runs `run`.
    static func place(
        _ actions: [LumennaApp.ScreenAction], in item: UINavigationItem, menu: Bool,
        run: @escaping (LumennaApp.ScreenAction) -> Void
    ) {
        var leading: [UIBarButtonItem] = []
        var trailing: [UIBarButtonItem] = []
        for action in actions {
            guard let icon = icon(action.id) else { continue }
            let button = UIBarButtonItem(image: UIImage(systemName: icon.symbol), primaryAction: UIAction { _ in run(action) })
            button.accessibilityLabel = action.title
            // Shown with the name when a button is pressed and held, for those who read the
            // icons with the Large Content Viewer.
            button.title = action.named
            button.isEnabled = action.enabled
            if icon.leading { leading.append(button) } else { trailing.append(button) }
        }
        let rest = actions.filter { !holds($0) }
        if menu, !rest.isEmpty {
            let children = rest.map { action in
                UIAction(title: action.named, attributes: action.enabled ? [] : .disabled) { _ in run(action) }
            }
            let more = UIBarButtonItem(image: UIImage(systemName: "ellipsis.circle"), menu: UIMenu(children: children))
            more.accessibilityLabel = Wording.placeActions
            more.title = Wording.placeActions
            trailing.append(more)
        }
        item.leftItemsSupplementBackButton = true
        item.leftBarButtonItems = leading
        // A bar lays its trailing items from the edge inwards: reversed, they read in the core's
        // order from the leading side.
        item.rightBarButtonItems = trailing.reversed()
    }

    /// Undo or Redo pressed while a field has typing to undo: the typing goes, as ⌘Z has it
    /// (Undo Typing, Apple's own), and the store's change stays. Whether it did.
    static func undoesTyping(_ action: LumennaApp.ScreenAction) -> Bool {
        guard let typing = UIResponder.editingWithUndo else { return false }
        switch action.id {
        case "undo" where typing.canUndo: typing.undo()
        case "redo" where typing.canRedo: typing.redo()
        default: return false
        }
        return true
    }
}
