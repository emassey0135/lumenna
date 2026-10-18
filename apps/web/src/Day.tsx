// The planner: a day as it is lived, as a tree, as the app shows it (crates/app).
//
// The first row is the summary — what a glance at a timeline gives a sighted user. Then
// blocks in time order with their sittings beneath them, free time as rows of its own, and
// now as a position rather than a highlight. Opening the day puts it on now, not midnight. Which
// day is shown, which row is in hand and what is folded are the app's.
//
// Each row's actions are the core's, in its menu (the Menu key, Shift+F10, a right-click, or
// Actions). On a sitting, Space runs its timer action (start, pause or resume) and Delete
// unassigns it; on a block, Enter changes it and Delete deletes it; on free time, Enter adds a
// block there; on a cancelled day, Enter restores it.

import { useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Toolbar, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import type { Action, ActionKind, DayMove, Event, Screen, ShownDayItem } from "./core";
import { byKind, REMOVING } from "./actions";
import { useCommand } from "./commands";
import { landOn, rowKey } from "./landing";
import { asksForMenu, RowMenu } from "./RowMenu";
import { run, sayAll, send } from "./session";
import { nest } from "./tree";
import type { Node } from "./tree";

/** What Space runs on a sitting: its timer. */
const TIMING: ActionKind[] = ["start_timer", "pause_timer", "resume_timer"];
/** What Enter runs: what a row is for. */
const OPENING: ActionKind[] = ["edit", "edit_task", "add_block", "restore_day"];

export function Day(props: {
  screen: Screen;
  onChanged: () => void;
  /** Opens a task's details. */
  onOpenTask: (id: string) => void;
}) {
  const day = props.screen.day;
  const items = useMemo(() => day?.items ?? [], [day]);
  const tree = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState(false);
  const [dayMenu, setDayMenu] = useState(false);
  const nodes = useMemo(() => nest(items, (item) => item.key, (item) => item.depth), [items]);
  const expanded = useMemo(() => new Set<Key>(items.filter((i) => i.expanded).map((i) => i.key)), [items]);
  // The same keys while the row in hand is the same: a new array each time is a new selection
  // to React Aria, which says "… selected" for each while the tree has focus.
  const selected = useMemo(() => (props.screen.focus ? [props.screen.focus] : []), [props.screen.focus]);
  const inHand = items.find((item) => item.key === props.screen.focus);

  /** Focus where the app puts it, then what it said. */
  const settle = async (turn: { screen: Screen; said: string[] }, landing = true) => {
    if (landing) await landOn(tree.current, turn.screen.focus ?? undefined);
    sayAll(turn.said);
    props.onChanged();
  };

  /** The forms an action opens that are still this client's own: a sitting's task's details. */
  const form = async (action: Action) => {
    if (action.kind === "edit_task") props.onOpenTask(action.target);
  };

  const addBlock = () => void send({ Block: { New: { start: undefined, minutes: undefined } } });

  /** Runs `event` through the app from this day, its forms the client's own. */
  const act = async (event: Event, landing = true) => settle(await run(event, form), landing);

  const move = (step: DayMove) => void act({ Day: step });

  // The Day menu's commands: each goes to the day first, from wherever it is pressed.
  useCommand("previous-day", () => move("Previous"));
  useCommand("next-day", () => move("Next"));
  useCommand("go-to-day", () => move("Ask"));
  useCommand("go-to-now", () => move("Now"));
  useCommand("new-block", addBlock);

  const onRow = async (key: string, kinds: ActionKind[]) => {
    await send({ Focus: key });
    await act({ Run: kinds });
  };

  const keysDown = (event: KeyboardEvent) => {
    const key = rowKey(event);
    const at = items.find((item) => item.key === key);
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      void send({ Focus: at.key }).then(() => setMenu(true));
      return;
    }
    const kinds = event.key === " " && at.kind === "sitting" ? TIMING : event.key === "Delete" ? REMOVING : undefined;
    if (!kinds) return;
    event.preventDefault();
    event.stopPropagation();
    void onRow(at.key, kinds);
  };

  const rightClick = (event: MouseEvent) => {
    const key = rowKey(event);
    if (!key) return;
    event.preventDefault();
    void send({ Focus: key }).then(() => setMenu(true));
  };

  const chosen = (selection: Selection) => {
    if (selection === "all") return;
    const [key] = [...selection];
    if (key !== undefined) void send({ Focus: String(key) });
  };

  // Left and Right fold a block: its own Expand or Collapse, run through the app.
  const fold = (open: Set<Key>) => {
    const changed = items.find((item) => item.expanded !== undefined && item.expanded !== open.has(item.key));
    const action = changed && byKind(changed.actions, "expand", "collapse");
    if (action) void act({ Perform: action });
  };

  return (
    <>
      <h2>{day?.title ?? "Today"}</h2>
      {/* A toolbar is one Tab stop, its buttons a press of the arrows apart (ARIA's toolbar pattern). */}
      <Toolbar className="buttons" aria-label="Day">
        <Button isDisabled={!day} onPress={() => move("Previous")}>
          Previous day
        </Button>
        <Button onPress={() => move("Today")}>Today</Button>
        <Button isDisabled={!day} onPress={() => move("Next")}>
          Next day
        </Button>
        <Button onPress={() => move("Ask")}>Go to day</Button>
        <Button onPress={addBlock}>Add block</Button>
        {/* The day's own actions, wherever focus is: its summary row offers them too. */}
        <RowMenu
          label="Day actions"
          actions={day?.actions ?? []}
          onAction={(action) => void act({ Perform: action }, false)}
          isOpen={dayMenu}
          onOpenChange={setDayMenu}
        />
        <RowMenu actions={inHand?.actions ?? []} onAction={(action) => void act({ Perform: action })} isOpen={menu} onOpenChange={setMenu} />
      </Toolbar>
      {/* Space and Delete are caught on the way down, before the tree takes Space for selection. */}
      <div onKeyDownCapture={keysDown} onContextMenu={rightClick}>
        <Tree
          ref={tree}
          aria-label="The day"
          items={nodes}
          selectionMode="single"
          selectionBehavior="replace"
          disallowEmptySelection
          selectedKeys={selected}
          onSelectionChange={chosen}
          expandedKeys={expanded}
          onExpandedChange={fold}
          onAction={(key) => void onRow(String(key), OPENING)}
        >
          {function item(node: Node<ShownDayItem>) {
            return (
              <TreeItem id={node.id} textValue={node.item.text}>
                <TreeItemContent>
                  {({ hasChildItems }) => (
                    <div className={`row ${node.item.kind}`}>
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
    </>
  );
}
