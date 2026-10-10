import Foundation

/// Rows folded away under a collapsed one, on one list, for the iPhone's lists and the
/// watch's alike. Folding is how someone moving a row
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

    /// The action that folds or unfolds a row, with what is said once it has: only a row
    /// with something under it has one.
    static func action<Item>(for row: Shown<Item>) -> (title: String, said: String)? {
        guard row.parent else { return nil }
        return row.collapsed ? ("Expand", "Expanded") : ("Collapse", "Collapsed")
    }

    mutating func toggle(_ key: String) {
        if collapsed.contains(key) { collapsed.remove(key) } else { collapsed.insert(key) }
    }
}
