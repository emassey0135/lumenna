// Tasks, with the filter above them, as the app shows them (crates/app).
//
// A React Aria tree, so a subtask's level and every row's position are the tree's to say. What
// is listed, what is folded and which row is in hand are the app's: the tree draws the screen
// and sends back what the person did. Each row's actions are the core's, in its menu (the Menu
// key, Shift+F10, a right-click, or Actions). Space runs Mark Done, Mark Not Done or Restore,
// whichever the row has; Delete what removes it; Enter opens its details. After a change focus
// goes where the app puts it — the same task if it is still listed, else whatever now holds its
// place — and only then is the change said.

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Input, Label, TextField, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import type { Action, ActionKind, Event, Screen, ShownItem } from "./core";
import { byKind, REMOVING } from "./actions";
import { useCommand } from "./commands";
import { asksForMenu, RowMenu } from "./RowMenu";
import { landOn, rowKey } from "./landing";
import { run, sayAll, send } from "./session";
import type { Form } from "./session";
import { nest } from "./tree";
import type { Node } from "./tree";

/** What Space runs on a row: whichever of these it has. */
const CHECKING: ActionKind[] = ["mark_done", "mark_not_done", "restore"];

export function TaskList(props: { screen: Screen; onOpen: () => void; onForm: Form; onChanged: () => void }) {
  const { screen } = props;
  const list = screen.list;
  const trash = screen.place === "Trash";
  const items = useMemo(() => list?.items ?? [], [list]);
  const [filter, setFilter] = useState(screen.query);
  const tree = useRef<HTMLDivElement>(null);
  const filterField = useRef<HTMLInputElement>(null);
  const [menu, setMenu] = useState(false);

  // A new place starts from its own query.
  useEffect(() => setFilter(screen.query), [screen.place]);

  const nodes = useMemo(() => nest(items, (item) => item.row.id, (item) => item.row.depth), [items]);
  const expanded = useMemo(() => new Set<Key>(items.filter((item) => item.row.expanded).map((item) => item.row.id)), [items]);
  // The same keys while the row in hand is the same: a new array each time is a new selection
  // to React Aria, which says "… selected" for each while the tree has focus.
  const selected = useMemo(() => (screen.focus ? [screen.focus] : []), [screen.focus]);
  const inHand = items.find((item) => item.row.id === screen.focus);

  /** Runs `event` through the app from this list: focus where it lands, then what it said. */
  const act = async (event: Event) => {
    const turn = await run(event, props.onForm);
    await landOn(tree.current, turn.screen.focus ?? undefined);
    sayAll(turn.said);
    props.onChanged();
  };

  /** Runs one of the row `id`'s kinds, with that row in hand. */
  const onRow = async (id: string, kinds: ActionKind[]) => {
    await send({ Focus: id });
    await act({ Run: kinds });
  };

  // Filter Tasks: the filter, all of it selected so typing replaces it. Not in the trash.
  useCommand("filter", () => {
    if (trash || !filterField.current) return false;
    filterField.current.focus();
    filterField.current.select();
  });

  // The Task menu's commands, on the task in hand here, before the details' copy of them.
  const onSelected = (...kinds: ActionKind[]) => () => {
    if (!inHand || !byKind(inHand.row.actions, ...kinds)) return false;
    void act({ Run: kinds });
  };
  useCommand("mark-done", onSelected("mark_done", "mark_not_done"), 1);
  useCommand("put-in-block", onSelected("put_in_block"), 1);
  useCommand("move-to-project", onSelected("move_to_project"), 1);

  const keys = (event: KeyboardEvent) => {
    const id = rowKey(event);
    if (!id || !items.some((item) => item.row.id === id)) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      void send({ Focus: id }).then(() => setMenu(true));
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
    void send({ Focus: id }).then(() => setMenu(true));
  };

  const choose = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    if (chosen !== undefined) void send({ Focus: String(chosen) });
  };

  // Left and Right fold a row: the row's own Expand or Collapse, run through the app.
  const fold = (keys: Set<Key>) => {
    const changed = items.find((item) => item.row.expanded !== undefined && item.row.expanded !== keys.has(item.row.id));
    const action = changed && byKind(changed.row.actions, "expand", "collapse");
    if (action) void act({ Perform: action });
  };

  return (
    <>
      <h2>{screen.title}</h2>
      {!trash && (
        <TextField
          className="field"
          value={filter}
          onChange={(text) => {
            setFilter(text);
            void send({ Filter: text });
          }}
          onKeyDown={(event) => {
            // Finishing the filter says what it found, then goes to it.
            if (event.key === "Enter" && list) {
              sayAll([list.refusal ?? list.readback]);
              tree.current?.querySelector<HTMLElement>('[role="row"]')?.focus();
            }
          }}
        >
          <Label>Filter</Label>
          <Input ref={filterField} placeholder="#Work & overdue, or search: words" />
        </TextField>
      )}
      {/* An empty list says so once, in its own words below; a filter's problem always shows. */}
      <p className="quiet" id="readback">
        {list?.refusal ?? (items.length > 0 ? list?.readback : "")}
      </p>
      <div className="buttons">
        <RowMenu
          actions={inHand?.row.actions ?? []}
          onAction={(action: Action) => void act({ Perform: action })}
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
            const done = byKind(node.item.row.actions, "mark_done", "mark_not_done");
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
                      {/* For a pointer: Mark Done (or not done) at a click, as a sighted user looks
                          for it. Hidden from screen readers and out of the tab order, where Space and
                          the row's menu already do it and the row says its state; its own pointer
                          events stop here, so the row behind it is neither selected nor opened. */}
                      {done && (
                        <span
                          className="done-box"
                          aria-hidden="true"
                          data-checked={done.kind === "mark_not_done" || undefined}
                          title={done.sentence ?? done.title}
                          onPointerDown={(event) => event.stopPropagation()}
                          onPointerUp={(event) => event.stopPropagation()}
                          onClick={(event) => {
                            event.stopPropagation();
                            void onRow(node.id, [done.kind]);
                          }}
                        />
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
    </>
  );
}
