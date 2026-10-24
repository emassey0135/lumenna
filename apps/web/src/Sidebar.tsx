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
import type { ShortAction, SidebarEntry, Place } from "./core";
import { ActionKind, Side } from "./core";
import { actOn, byKind, keyPressed, rowHeld } from "./actions";
import { rowKey } from "./landing";
import { asksForMenu, RowMenu } from "./RowMenu";
import { send } from "./session";
import { nest } from "./tree";
import type { Node } from "./tree";
import { SwatchMark } from "./Swatch";

/** What identifies a place's entry across reloads. */
export function placeKey(entries: SidebarEntry[], place: Place): string | undefined {
  return entries.find((entry) => entry.kind.tag === "Place" && JSON.stringify(entry.kind.value0) === JSON.stringify(place))?.key;
}

/** What identifies an entry across reloads: the app's key, which its row in hand is named by. */
function key(entry: SidebarEntry): string {
  return entry.key;
}

export function Sidebar(props: {
  entries: SidebarEntry[];
  place: Place;
  onPlace: (place: Place) => void;
}) {
  const entries = props.entries;

  // The row a menu is open for. The row in hand is the app's (`sidebar_held`): each move here
  // tells it, so a change made from a row leaves the row it says in hand.
  const [held, setHeld] = useState<string | undefined>();
  const [menu, setMenu] = useState(false);
  const tree = useRef<HTMLDivElement>(null);

  // The same keys while the place is the same: a new array is a new selection to React Aria,
  // which says "… selected" for each while the tree has focus.
  const here = placeKey(entries, props.place);
  const selected = useMemo(() => (here ? [here] : []), [here]);
  const nodes = useMemo(() => nest(entries, key, (entry) => entry.depth), [entries]);
  // What is folded is the app's: a row is open while it offers Collapse.
  const shown = useMemo(
    () => new Set<Key>(entries.filter((e) => e.offered?.some((a) => a.kind === ActionKind.Collapse)).map(key)),
    [entries],
  );
  // Left and Right fold a row: its own Expand or Collapse, run through the app.
  const fold = (open: Set<Key>) => {
    const changed = entries.find((e) => {
      const action = byKind(e.offered, ActionKind.Expand, ActionKind.Collapse);
      return action && (action.kind === ActionKind.Collapse) !== open.has(key(e));
    });
    const action = changed && byKind(changed.offered, ActionKind.Expand, ActionKind.Collapse);
    if (action) void send(actOn(rowHeld(Side.Sidebar, key(changed)), action));
  };
  const entry = (k: string | undefined) => entries.find((e) => key(e) === k);

  const go = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    const found = entry(String(chosen));
    if (found && found.kind.tag === "Place") props.onPlace(found.kind.value0);
  };

  /** Runs one of a row's actions through the app, which goes where it leads, and says what it did. */
  const run = async (at: SidebarEntry, action: ShortAction) => {
    await send(actOn(rowHeld(Side.Sidebar, key(at)), action));
  };

  const keys = (event: KeyboardEvent) => {
    const at = entry(rowKey(event));
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      setHeld(key(at));
      setMenu(true);
    } else if (event.key === "Delete") {
      // Delete here does what it does in every other list, the core's, on the row held here.
      event.preventDefault();
      event.stopPropagation();
      void send({ tag: "Hold", value0: Side.Sidebar, value1: key(at) }).then(() => send(keyPressed("delete")));
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
          if (k) {
            setHeld(k);
            void send({ tag: "Hold", value0: Side.Sidebar, value1: k });
          }
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
                      <SwatchMark swatch={node.item.swatch} />
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
          actions={entry(held)?.offered ?? []}
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
