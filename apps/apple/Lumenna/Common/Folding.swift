import UIKit

/// Rows folded away under a collapsed one, on one list. Folding is how someone moving a row
/// at a time skips a block or project with a lot under it, so every list with depth has it,
/// and everything starts expanded.
struct Folding {
    /// A row as shown: whether anything is under it, and whether that is folded away.
    struct Shown<Item> {
        let item: Item
        let depth: Int
        let parent: Bool
        let collapsed: Bool

        /// What it says of its state: only a row with something under it has one.
        var state: String? { parent ? (collapsed ? "collapsed" : "expanded") : nil }
    }

    private(set) var collapsed: Set<String> = []

    /// `items` as shown, with the rows under a collapsed one left out.
    func shown<Item>(_ items: [Item], depth: (Item) -> Int, key: (Item) -> String) -> [Shown<Item>] {
        var shown: [Shown<Item>] = []
        var hiddenBelow: Int?
        for (index, item) in items.enumerated() {
            let level = depth(item)
            if let hider = hiddenBelow, level > hider { continue }
            hiddenBelow = nil
            let parent = index + 1 < items.count && depth(items[index + 1]) > level
            let isCollapsed = parent && collapsed.contains(key(item))
            if isCollapsed { hiddenBelow = level }
            shown.append(Shown(item: item, depth: level, parent: parent, collapsed: isCollapsed))
        }
        return shown
    }

    /// "level 2" where the level changes from the row shown before, or nil.
    static func levelChange<Item>(_ shown: [Shown<Item>], at index: Int) -> String? {
        let previous = index > 0 ? shown[index - 1].depth : 0
        return shown[index].depth != previous ? "level \(shown[index].depth + 1)" : nil
    }

    /// Expand or Collapse, for a row with something under it. `changed` redraws the list
    /// and says what happened.
    func action<Item>(
        for row: Shown<Item>, key: String, changed: @escaping (_ key: String, _ said: String) -> Void
    ) -> UIContextualAction? {
        guard row.parent else { return nil }
        let title = row.collapsed ? "Expand" : "Collapse"
        let action = UIContextualAction(style: .normal, title: title) { _, _, finished in
            changed(key, row.collapsed ? "Expanded" : "Collapsed")
            finished(true)
        }
        action.backgroundColor = .lumennaTint
        return action
    }

    mutating func toggle(_ key: String) {
        if collapsed.contains(key) { collapsed.remove(key) } else { collapsed.insert(key) }
    }
}
