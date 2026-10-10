// The places, as the desktop apps list them —
// the list itself is crates/desktop's `places::sidebar`, run in the worker.
//
// Projects, labels and saved filters are managed here, as on the desktop: each row's menu
// (the Menu key, Shift+F10, a right-click, or Actions) has the actions the core gives it, the
// headings' New among them, and Delete runs the row's Delete. Renaming or making a place goes
// to it.

import { useEffect, useMemo, useRef, useState } from "react";
import type { FocusEvent, KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { core } from "./core";
import type { Action, SidebarEntry, Place } from "./core";
import { byKind, perform } from "./actions";
import type { Done } from "./actions";
import { rowKey } from "./landing";
import { ask } from "./Prompts";
import { asksForMenu, RowMenu } from "./RowMenu";
import { say } from "./say";
import { nest, parents } from "./tree";
import type { Node } from "./tree";

/** What identifies an entry across reloads: its kind, as written. */
export function placeKey(place: Place): string {
  return JSON.stringify({ Place: place });
}

function key(entry: SidebarEntry): string {
  return JSON.stringify(entry.kind);
}

export function Sidebar(props: {
  revision: number;
  place: Place;
  onPlace: (place: Place) => void;
  onChanged: () => void;
}) {
  const [entries, setEntries] = useState<SidebarEntry[]>([]);
  const [expanded, setExpanded] = useState<Set<Key> | undefined>();
  // The row in hand: the one focus was last on. Headings are never selected, so it is not
  // the selection.
  const [held, setHeld] = useState<string | undefined>();
  const [menu, setMenu] = useState(false);
  const tree = useRef<HTMLDivElement>(null);

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
  const entry = (k: string | undefined) => entries.find((e) => key(e) === k);

  const go = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    const found = entry(String(chosen));
    if (found && "Place" in found.kind) props.onPlace(found.kind.Place);
  };

  // The saved filter the form just made, which is where to go.
  const made = useRef<Place | undefined>(undefined);

  // A saved filter's form: its name, then its query. The core refuses what it must.
  const newFilter = async (): Promise<Done | undefined> => {
    const name = await ask("New Saved Filter", "Name", "");
    if (name === undefined) return undefined;
    let done: Done | undefined;
    const query = await ask(`Query for ${name.trim()}`, "Query", "Such as #Work & overdue, or p1 | today.", "", async (query) => {
      try {
        done = await core.addFilter(name.trim(), query);
        return undefined;
      } catch (error) {
        return (error as Error).message;
      }
    });
    if (query === undefined || !done) return undefined;
    made.current = { Filter: { name: name.trim(), query } };
    return done;
  };

  /** Where to go once `action` has made, renamed or removed a place: to it, or to Tasks. */
  const where = (at: SidebarEntry, action: Action, answer: string | undefined): Place | undefined => {
    const place = "Place" in at.kind ? at.kind.Place : undefined;
    if (action.kind === "delete") return "Tasks";
    if (answer === undefined) return undefined;
    switch (`${action.subject}:${action.kind}`) {
      case "project:new":
      case "project:new_inside":
      case "project:rename":
        return { Project: answer };
      case "label:new":
      case "label:rename":
      case "label:merge_into":
        return { Label: answer };
      case "filter:rename":
        return typeof place === "object" && "Filter" in place ? { Filter: { name: answer, query: place.Filter.query } } : undefined;
      case "filter:change_query":
        return typeof place === "object" && "Filter" in place ? { Filter: { name: place.Filter.name, query: answer } } : undefined;
    }
    return undefined;
  };

  /** Runs one of a row's actions, says what it did, and goes where it leads. */
  const run = async (at: SidebarEntry, action: Action) => {
    made.current = undefined;
    const done = await perform(action, () => newFilter());
    if (!done) return;
    props.onChanged();
    const then = done.changed ? (made.current ?? where(at, action, done.answer)) : undefined;
    if (then) props.onPlace(then);
    say(done.said);
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
          selectedKeys={[placeKey(props.place)]}
          onSelectionChange={go}
          expandedKeys={shown}
          onExpandedChange={setExpanded}
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
          label="Place Actions"
        />
      </div>
    </>
  );
}
