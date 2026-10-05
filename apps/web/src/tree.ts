// Flat, depth-first rows — what the core sends — as nested nodes for a tree.

export interface Node<T> {
  id: string;
  item: T;
  children: Node<T>[];
}

/**
 * Nests rows under the nearest shallower row above them. A row whose parent was filtered out
 * has no shallower row above it in its branch, and sits at the top rather than under whatever
 * happens to precede it — the rule the desktop apps' outlines use (`crates/desktop` outline).
 */
export function nest<T>(rows: T[], id: (row: T) => string, depth: (row: T) => number): Node<T>[] {
  const top: Node<T>[] = [];
  const chain: { node: Node<T>; depth: number }[] = [];
  for (const row of rows) {
    const node: Node<T> = { id: id(row), item: row, children: [] };
    const level = depth(row);
    while (chain.length > 0 && chain[chain.length - 1].depth >= level) chain.pop();
    (chain.length > 0 ? chain[chain.length - 1].node.children : top).push(node);
    chain.push({ node, depth: level });
  }
  return top;
}

/** Every node with children, by identifier: what a tree shows expanded at first. */
export function parents<T>(nodes: Node<T>[]): string[] {
  return nodes.flatMap((node) => (node.children.length > 0 ? [node.id, ...parents(node.children)] : []));
}
