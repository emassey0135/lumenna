// Tasks, with the filter above them, as the app shows them (crates/app).
//
// A React Aria tree, so a subtask's level and every row's position are the tree's to say. What
// is listed, what is folded and which row is in hand are the app's: the tree draws the screen
// and sends back what the person did. Each row's actions are the core's, in its menu (the Menu
// key, Shift+F10, a right-click, or Actions). Space runs Mark Done, Mark Not Done or Restore,
// whichever the row has; Delete what removes it; Enter opens its details. After a change focus
// goes where the app puts it — the same task if it is still listed, else whatever now holds its
// place — and only then is the change said, as every turn is (session.ts).

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import type { Screen, ShellEvent, ShortAction, ShownItem } from "./core";
import { ActionKind, Syntax } from "./core";
import { CompletingField } from "./Completing";
import { byKind, REMOVING, shortActions } from "./actions";
import { useCommand } from "./commands";
import { asksForMenu, RowMenu } from "./RowMenu";
import { rowKey } from "./landing";
import { say } from "./say";
import { send } from "./session";
import { nest } from "./tree";
import type { Node } from "./tree";
import { SwatchMark } from "./Swatch";
import { brailleOnRow } from "./braille";

/** What Space runs on a row: whichever of these it has. */
const CHECKING: ActionKind[] = [ActionKind.MarkDone, ActionKind.MarkNotDone, ActionKind.Restore];

export function TaskList(props: { screen: Screen; onOpen: () => void }) {
  const { screen } = props;
  const list = screen.list;
  const trash = screen.place?.tag === "Trash";
  const items = useMemo(() => list?.items ?? [], [list]);
  const [filter, setFilter] = useState(screen.query);
  const tree = useRef<HTMLDivElement>(null);
  const filterField = useRef<HTMLInputElement>(null);
  const [menu, setMenu] = useState(false);

  // A new place starts from its own query. Compared by what the place is: every turn brings a
  // new object, and resetting on each put the field back to the app's answer for earlier typing,
  // losing a key typed since ("@ca" came out "@a").
  const placeIs = JSON.stringify(screen.place ?? null);
  useEffect(() => setFilter(screen.query), [placeIs]);

  const nodes = useMemo(() => nest(items, (item) => item.row.id, (item) => item.row.depth), [items]);
  const expanded = useMemo(() => new Set<Key>(items.filter((item) => item.row.expanded).map((item) => item.row.id)), [items]);
  // The same keys while the row in hand is the same: a new array each time is a new selection
  // to React Aria, which says "… selected" for each while the tree has focus.
  const selected = useMemo(() => (screen.focus ? [screen.focus] : []), [screen.focus]);
  const inHand = items.find((item) => item.row.id === screen.focus);

  /** Sends `event`; what comes back lands focus and is said as every turn is (session.ts). */
  const act = (event: ShellEvent) => void send(event);

  /** Runs one of the row `id`'s kinds, with that row in hand. */
  const onRow = async (id: string, kinds: ActionKind[]) => {
    await send({ tag: "Focus", value0: id });
    act({ tag: "Run", value0: kinds });
  };

  // Filter Tasks: the filter, all of it selected so typing replaces it. Not in the trash.
  useCommand("filter", () => {
    if (trash || !filterField.current) return false;
    filterField.current.focus();
    filterField.current.select();
  });

  // The Task menu's commands, on the task in hand here, before the details' copy of them.
  const onSelected = (...kinds: ActionKind[]) => () => {
    if (!inHand || !byKind(shortActions(list, inHand), ...kinds)) return false;
    act({ tag: "Run", value0: kinds });
  };
  useCommand("mark-done", onSelected(ActionKind.MarkDone, ActionKind.MarkNotDone), 1);
  useCommand("put-in-block", onSelected(ActionKind.PutInBlock), 1);
  useCommand("move-to-project", onSelected(ActionKind.MoveToProject), 1);

  const keys = (event: KeyboardEvent) => {
    const id = rowKey(event);
    if (!id || !items.some((item) => item.row.id === id)) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      void send({ tag: "Focus", value0: id }).then(() => setMenu(true));
      return;
    }
    const kinds = event.key === " " ? CHECKING : event.key === "Delete" ? REMOVING : undefined;
    if (!kinds) return;
    event.preventDefault();
    event.stopPropagation();
    void onRow(id, kinds);
  };

  const rightClick = (event: MouseEvent) => {
    const id = rowKey(event);
    if (!id) return;
    event.preventDefault();
    void send({ tag: "Focus", value0: id }).then(() => setMenu(true));
  };

  const choose = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    if (chosen !== undefined) void send({ tag: "Focus", value0: String(chosen) });
  };

  // Left and Right fold a row: the row's own Expand or Collapse, run through the app.
  const fold = (keys: Set<Key>) => {
    const changed = items.find((item) => item.row.expanded != null && item.row.expanded !== keys.has(item.row.id));
    const action = changed && byKind(shortActions(list, changed), ActionKind.Expand, ActionKind.Collapse);
    if (action) act({ tag: "Act", value0: action });
  };

  return (
    <>
      <h2>{screen.title}</h2>
      {!trash && (
        <CompletingField
          label="Filter"
          value={filter}
          syntax={Syntax.Filter}
          inputRef={filterField}
          placeholder="#Work & overdue, or search: words"
          onChange={(text) => {
            setFilter(text);
            void send({ tag: "Filter", value0: text });
          }}
          onEnter={() => {
            // Finishing the filter says what it found, then goes to it.
            if (!list) return;
            say(list.refusal ?? list.readback);
            tree.current?.querySelector<HTMLElement>('[role="row"]')?.focus();
          }}
        />
      )}
      {/* An empty list says so once, in its own words below; a filter's problem always shows. */}
      <p className="quiet" id="readback">
        {list?.refusal ?? (items.length > 0 ? list?.readback : "")}
      </p>
      <div className="buttons">
        <RowMenu
          actions={shortActions(list, inHand)}
          onAction={(action: ShortAction) => act({ tag: "Act", value0: action })}
          isOpen={menu}
          onOpenChange={setMenu}
        />
      </div>
      {/* Space and Delete are caught on the way down, before the tree takes Space for selection. */}
      <div onKeyDownCapture={keys} onContextMenu={rightClick}>
        <Tree
          ref={tree}
          aria-label={screen.title}
          aria-describedby="readback"
          items={nodes}
          selectionMode="single"
          selectionBehavior="replace"
          disallowEmptySelection
          selectedKeys={selected}
          onSelectionChange={choose}
          expandedKeys={expanded}
          onExpandedChange={fold}
          onAction={() => props.onOpen()}
          renderEmptyState={() => <p className="quiet">{list?.empty}</p>}
        >
          {function item(node: Node<ShownItem>) {
            const done = byKind(shortActions(list, node.item), ActionKind.MarkDone, ActionKind.MarkNotDone);
            return (
              <TreeItem id={node.id} textValue={node.item.text}>
                <TreeItemContent>
                  {({ hasChildItems }) => (
                    <div className="row" ref={brailleOnRow(node.item.text, node.item.braille)}>
                      {hasChildItems && (
                        <Button slot="chevron" className="chevron">
                          ›
                        </Button>
                      )}
                      {/* For a pointer: Mark Done (or not done) at a click, as a sighted user looks
                          for it. Hidden from screen readers and out of the tab order, where Space and
                          the row's menu already do it and the row says its state; its own pointer
                          events stop here, so the row behind it is neither selected nor opened. */}
                      {done && (
                        <span
                          className="done-box"
                          aria-hidden="true"
                          data-checked={done.kind === ActionKind.MarkNotDone || undefined}
                          title={done.sentence ?? done.title}
                          onPointerDown={(event) => event.stopPropagation()}
                          onPointerUp={(event) => event.stopPropagation()}
                          onClick={(event) => {
                            event.stopPropagation();
                            void onRow(node.id, [done.kind]);
                          }}
                        />
                      )}
                      <SwatchMark swatch={node.item.row.swatch} />
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
    </>
  );
}
