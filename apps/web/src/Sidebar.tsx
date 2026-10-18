// The places, as the desktop apps list them: the app's sidebar (records' `places::sidebar`).
//
// Projects, labels and saved filters are managed here, as on the desktop: each row's menu
// (the Menu key, Shift+F10, a right-click, or Actions) has the actions the core gives it, the
// headings' New among them, and Delete runs the row's Delete. Renaming or making a place goes
// to it: the app's decision.

import { useMemo, useRef, useState } from "react";
import type { FocusEvent, KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import type { Action, SidebarEntry, Place } from "./core";
import { byKind } from "./actions";
import { rowKey } from "./landing";
import { asksForMenu, RowMenu } from "./RowMenu";
import { run as runEvent, sayAll } from "./session";
import { nest } from "./tree";
import type { Node } from "./tree";

/** What identifies an entry across reloads: its kind, as written. */
export function placeKey(place: Place): string {
  return JSON.stringify({ Place: place });
}

function key(entry: SidebarEntry): string {
  return JSON.stringify(entry.kind);
}

export function Sidebar(props: {
  entries: SidebarEntry[];
  place: Place;
  onPlace: (place: Place) => void;
  onChanged: () => void;
}) {
  const entries = props.entries;

  // The row in hand: the one focus was last on. Headings are never selected, so it is not
  // the selection.
  const [held, setHeld] = useState<string | undefined>();
  const [menu, setMenu] = useState(false);
  const tree = useRef<HTMLDivElement>(null);

  // The same keys while the place is the same: a new array is a new selection to React Aria,
  // which says "… selected" for each while the tree has focus.
  const here = placeKey(props.place);
  const selected = useMemo(() => [here], [here]);
  const nodes = useMemo(() => nest(entries, key, (entry) => entry.depth), [entries]);
  // What is folded is the app's: a row is open while it offers Collapse.
  const shown = useMemo(
    () => new Set<Key>(entries.filter((e) => e.actions?.some((a) => a.kind === "collapse")).map(key)),
    [entries],
  );
  // Left and Right fold a row: its own Expand or Collapse, run through the app.
  const fold = (open: Set<Key>) => {
    const changed = entries.find((e) => {
      const action = byKind(e.actions, "expand", "collapse");
      return action && (action.kind === "collapse") !== open.has(key(e));
    });
    const action = changed && byKind(changed.actions, "expand", "collapse");
    if (action) void runEvent({ Perform: action }).then((turn) => sayAll(turn.said));
  };
  const entry = (k: string | undefined) => entries.find((e) => key(e) === k);

  const go = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    const found = entry(String(chosen));
    if (found && "Place" in found.kind) props.onPlace(found.kind.Place);
  };

  /** Runs one of a row's actions through the app, which goes where it leads, and says what it did. */
  const run = async (_at: SidebarEntry, action: Action) => {
    const turn = await runEvent({ Perform: action });
    props.onChanged();
    sayAll(turn.said);
  };

  const keys = (event: KeyboardEvent) => {
    const at = entry(rowKey(event));
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      setHeld(key(at));
      setMenu(true);
    } else if (event.key === "Delete") {
      // Delete here does what it does in every other list, asking first as the menu's does.
      const action = byKind(at.actions, "delete");
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      void run(at, action);
    }
  };

  const rightClick = (event: MouseEvent) => {
    const k = rowKey(event);
    if (!k) return;
    event.preventDefault();
    setHeld(k);
    setMenu(true);
  };

  return (
    <>
      <div
        onKeyDownCapture={keys}
        onContextMenu={rightClick}
        onFocusCapture={(event: FocusEvent) => {
          const k = rowKey(event);
          if (k) setHeld(k);
        }}
      >
        <Tree
          ref={tree}
          aria-label="Places"
          items={nodes}
          selectionMode="single"
          disallowEmptySelection
          selectedKeys={selected}
          onSelectionChange={go}
          expandedKeys={shown}
          onExpandedChange={fold}
        >
          {function item(node: Node<SidebarEntry>) {
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
      </div>
      <div className="buttons">
        <RowMenu
          actions={entry(held)?.actions ?? []}
          onAction={(action) => {
            const at = entry(held);
            if (at) void run(at, action);
          }}
          isOpen={menu}
          onOpenChange={setMenu}
          label="Place actions"
        />
      </div>
    </>
  );
}
