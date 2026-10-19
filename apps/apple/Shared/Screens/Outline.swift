import Foundation

/// A row and the rows beneath it, as an outline shows them.
struct OutlineNode<Row>: Identifiable {
    let row: Row
    let id: String
    var children: [OutlineNode<Row>] = []
}

/// The app's rows, flat and depth first as it sends them, made a tree for a list that nests
/// them (the Mac's task list, day and sidebar).
enum Outline {
    /// A row's parent is the nearest shallower row above it; one whose parent was left out (a
    /// filter's match whose parent is not) sits at the top.
    static func tree<Row>(_ rows: [Row], id: (Row) -> String, depth: (Row) -> UInt32) -> [OutlineNode<Row>] {
        var top: [OutlineNode<Row>] = []
        // The chain of open parents, as paths into `top`.
        var chain: [(depth: UInt32, path: [Int])] = []
        for row in rows {
            let level = depth(row)
            while let last = chain.last, last.depth >= level { chain.removeLast() }
            let node = OutlineNode(row: row, id: id(row))
            if let parent = chain.last {
                let index = append(node, at: parent.path, in: &top)
                chain.append((level, parent.path + [index]))
            } else {
                top.append(node)
                chain.append((level, [top.count - 1]))
            }
        }
        return top
    }

    /// The rows as the outline shows them, in its order: depth first, inside expanded rows
    /// only, which is how AppKit numbers them.
    static func shown<Row>(_ nodes: [OutlineNode<Row>], expanded: (Row) -> Bool) -> [String] {
        nodes.flatMap { node in [node.id] + (expanded(node.row) ? shown(node.children, expanded: expanded) : []) }
    }

    private static func append<Row>(_ node: OutlineNode<Row>, at path: [Int], in nodes: inout [OutlineNode<Row>]) -> Int {
        guard let first = path.first else { return 0 }
        if path.count == 1 {
            nodes[first].children.append(node)
            return nodes[first].children.count - 1
        }
        return append(node, at: Array(path.dropFirst()), in: &nodes[first].children)
    }
}
