import UIKit

/// A VoiceOver rotor over some of a SwiftUI list's rows: Overdue on a task list, Blocks and
/// Running on the day. UIKit's, on the hosting view: SwiftUI's `accessibilityRotor` with
/// `accessibilityRotorEntry` on each row was listed by VoiceOver and stepped to none of them
/// (2026-10-20), a `List` being a collection view whose rows exist only while on the screen.
/// So the search works in the list's own rows, scrolls the collection view to the one it finds,
/// and hands VoiceOver that row's element.
enum RowRotor {
    /// A rotor called `name` over `rows`' keys in order, stopping on those `wanted` says, from
    /// the row VoiceOver is on (`current`); `landed` is told the row it moves to.
    static func make(
        _ name: String,
        in host: UIViewController,
        rows: @escaping () -> [String],
        wanted: @escaping (Int) -> Bool,
        current: @escaping () -> String?,
        landed: @escaping (String) -> Void
    ) -> UIAccessibilityCustomRotor {
        UIAccessibilityCustomRotor(name: name) { [weak host] predicate in
            guard let host else { return nil }
            let keys = rows()
            let here = current().flatMap { keys.firstIndex(of: $0) }
            let forward = predicate.searchDirection == .next
            let candidates: [Int] = forward
                ? Array(((here.map { $0 + 1 } ?? 0)..<keys.count))
                : Array((0..<(here ?? keys.count)).reversed())
            guard let found = candidates.first(where: wanted),
                  let element = element(of: found, count: keys.count, in: host.view)
            else { return nil }
            landed(keys[found])
            return UIAccessibilityCustomRotorItemResult(targetElement: element, targetRange: nil)
        }
    }

    /// Row `index` of the list's rows, `count` of them in its last section: scrolled to, then
    /// its cell's own element, which is the row VoiceOver reads.
    private static func element(of index: Int, count: Int, in view: UIView) -> NSObjectProtocol? {
        guard let list = collection(in: view), list.numberOfSections > 0 else { return nil }
        let section = list.numberOfSections - 1
        guard list.numberOfItems(inSection: section) == count else { return nil }
        let path = IndexPath(item: index, section: section)
        list.scrollToItem(at: path, at: .centeredVertically, animated: false)
        list.layoutIfNeeded()
        guard let cell = list.cellForItem(at: path) else { return nil }
        return firstElement(in: cell) ?? cell
    }

    /// The collection view a SwiftUI `List` is drawn by, in `view`.
    static func list(in view: UIView) -> UICollectionView? { collection(in: view) }

    private static func collection(in view: UIView) -> UICollectionView? {
        if let list = view as? UICollectionView { return list }
        for child in view.subviews {
            if let list = collection(in: child) { return list }
        }
        return nil
    }

    /// The first accessibility element in `container`, depth first: SwiftUI hosts a row's
    /// elements inside its cell rather than making the cell one.
    private static func firstElement(in container: NSObject) -> NSObject? {
        if container.isAccessibilityElement { return container }
        if let elements = container.accessibilityElements as? [NSObject] {
            for element in elements {
                if let found = firstElement(in: element) { return found }
            }
        }
        if let view = container as? UIView {
            for child in view.subviews {
                if let found = firstElement(in: child) { return found }
            }
        }
        return nil
    }
}
