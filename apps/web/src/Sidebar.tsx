// The places (§16.1: project tree, label list, saved filters), as the desktop apps list them —
// the list itself is crates/desktop's `places::sidebar`, run in the worker.

import { useEffect, useMemo, useState } from "react";
import { Button, Collection, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { core } from "./core";
import type { Entry, Place } from "./core";
import { nest, parents } from "./tree";
import type { Node } from "./tree";

/** What identifies an entry across reloads: its kind, as written. */
export function placeKey(place: Place): string {
  return JSON.stringify({ Place: place });
}

function key(entry: Entry): string {
  return JSON.stringify(entry.kind);
}

export function Sidebar(props: { revision: number; place: Place; onPlace: (place: Place) => void }) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [expanded, setExpanded] = useState<Set<Key> | undefined>();

  useEffect(() => {
    let current = true;
    void core.sidebar().then((entries) => current && setEntries(entries));
    return () => {
      current = false;
    };
  }, [props.revision]);

  const nodes = useMemo(() => nest(entries, key, (entry) => entry.depth), [entries]);
  // Everything open at first; after that, as the person leaves it.
  const shown = expanded ?? new Set<Key>(parents(nodes));

  const choose = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    const entry = entries.find((e) => key(e) === chosen);
    if (entry && "Place" in entry.kind) props.onPlace(entry.kind.Place);
  };

  return (
    <Tree
      aria-label="Places"
      items={nodes}
      selectionMode="single"
      disallowEmptySelection
      selectedKeys={[placeKey(props.place)]}
      onSelectionChange={choose}
      expandedKeys={shown}
      onExpandedChange={setExpanded}
    >
      {function item(node: Node<Entry>) {
        return (
          <TreeItem id={node.id} textValue={node.item.text}>
            <TreeItemContent>
              {({ hasChildItems }) => (
                <div className="row">
                  {hasChildItems && (
                    <Button slot="chevron" className="chevron">
                      ›
                    </Button>
                  )}
                  {node.item.text}
                </div>
              )}
            </TreeItemContent>
            <Collection items={node.children}>{item}</Collection>
          </TreeItem>
        );
      }}
    </Tree>
  );
}
