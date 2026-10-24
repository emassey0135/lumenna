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
import type { Screen, ShellEvent, ShownDayItem } from "./core";
import { ActionKind, DayMove } from "./core";
import { actOn, byKind, keyPressed, rowShown, sharedKey } from "./actions";
import { useCommand } from "./commands";
import { rowKey } from "./landing";
import { asksForMenu, RowMenu } from "./RowMenu";
import { send } from "./session";
import { nest } from "./tree";
import type { Node } from "./tree";
import { SwatchMark } from "./Swatch";
import { brailleOnRow } from "./braille";

/** What Space runs on a sitting: its timer. */
/** What Enter runs: what a row is for. */
const OPENING: ActionKind[] = [ActionKind.Edit, ActionKind.EditTask, ActionKind.AddBlock, ActionKind.RestoreDay];

export function Day(props: { screen: Screen }) {
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

  const addBlock = () => void send({ tag: "Block", value0: { tag: "New", start: null, minutes: null } });

  /** Sends `event`; what comes back lands focus and is said as every turn is (session.ts). */
  const act = (event: ShellEvent) => void send(event);

  const move = (step: DayMove) => act({ tag: "Day", value0: step });

  // The Day menu's commands: each goes to the day first, from wherever it is pressed.
  useCommand("previous-day", () => move(DayMove.Previous));
  useCommand("next-day", () => move(DayMove.Next));
  useCommand("go-to-day", () => move(DayMove.Ask));
  useCommand("go-to-now", () => move(DayMove.Now));
  useCommand("new-block", addBlock);

  const onRow = async (key: string, kinds: ActionKind[]) => {
    await send({ tag: "Focus", value0: key });
    act({ tag: "Run", value0: kinds });
  };

  const keysDown = (event: KeyboardEvent) => {
    const key = rowKey(event);
    const at = items.find((item) => item.key === key);
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      void send({ tag: "Focus", value0: at.key }).then(() => setMenu(true));
      return;
    }
    const pressed = sharedKey(event);
    if (!pressed) return;
    event.preventDefault();
    event.stopPropagation();
    void send({ tag: "Focus", value0: at.key }).then(() => act(keyPressed(pressed)));
  };

  const rightClick = (event: MouseEvent) => {
    const key = rowKey(event);
    if (!key) return;
    event.preventDefault();
    void send({ tag: "Focus", value0: key }).then(() => setMenu(true));
  };

  const chosen = (selection: Selection) => {
    if (selection === "all") return;
    const [key] = [...selection];
    if (key !== undefined) void send({ tag: "Focus", value0: String(key) });
  };

  // Left and Right fold a block: its own Expand or Collapse, run through the app.
  const fold = (open: Set<Key>) => {
    const changed = items.find((item) => item.expanded != null && item.expanded !== open.has(item.key));
    const action = changed && byKind(changed.offered, ActionKind.Expand, ActionKind.Collapse);
    if (action) act(actOn(rowShown(changed.key), action));
  };

  return (
    <>
      <h2>{day?.title ?? "Today"}</h2>
      {/* A toolbar is one Tab stop, its buttons a press of the arrows apart (ARIA's toolbar pattern). */}
      <Toolbar className="buttons" aria-label="Day">
        <Button isDisabled={!day} onPress={() => move(DayMove.Previous)}>
          Previous day
        </Button>
        <Button onPress={() => move(DayMove.Today)}>Today</Button>
        <Button isDisabled={!day} onPress={() => move(DayMove.Next)}>
          Next day
        </Button>
        <Button onPress={() => move(DayMove.Ask)}>Go to day</Button>
        <Button onPress={addBlock}>Add block</Button>
        {/* The day's own actions, wherever focus is: its summary row offers them too. */}
        <RowMenu
          label="Day actions"
          actions={day?.offered ?? []}
          onAction={(action) => act(actOn({ tag: "Day" }, action))}
          isOpen={dayMenu}
          onOpenChange={setDayMenu}
        />
        <RowMenu actions={inHand?.offered ?? []} onAction={(action) => inHand && act(actOn(rowShown(inHand.key), action))} isOpen={menu} onOpenChange={setMenu} />
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
                    <div className={`row ${node.item.kind}`} ref={brailleOnRow(node.item.text, node.item.braille)}>
                      {hasChildItems && (
                        <Button slot="chevron" className="chevron">
                          ›
                        </Button>
                      )}
                      <SwatchMark swatch={node.item.block?.swatch} />
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
