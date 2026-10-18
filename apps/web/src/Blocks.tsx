// Every block series, as the app lists them: its actions are the core's, in its menu; Enter
// changes one, Delete deletes it, asking first; Add block makes one.

import { useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { blockForm, freshBlock } from "./BlockForm";
import { core } from "./core";
import type { Action, ActionKind, Event, Screen } from "./core";
import { REMOVING } from "./actions";
import { useCommand } from "./commands";
import { landOn, rowKey } from "./landing";
import { asksForMenu, RowMenu } from "./RowMenu";
import { current, refresh, run, sayAll, send } from "./session";

export function Blocks(props: { screen: Screen; onChanged: () => void }) {
  const list = props.screen.list;
  const items = useMemo(() => list?.items ?? [], [list]);
  const tree = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState(false);
  // The same keys while the row in hand is the same: a new array each time is a new selection
  // to React Aria, which says "… selected" for each while the tree has focus.
  const selected = useMemo(() => (props.screen.focus ? [props.screen.focus] : []), [props.screen.focus]);
  const inHand = items.find((item) => item.row.id === props.screen.focus);

  /** Focus where the app puts it, then what it said. */
  const settle = async (focus: string | undefined, said: string[]) => {
    await landOn(tree.current, focus);
    sayAll(said);
    props.onChanged();
  };

  /** A write made by the block form: the app takes it in, the row named in hand. */
  const wrote = async (said: string, key?: string) => {
    await refresh();
    const screen = key ? (await send({ Focus: key })).screen : await current();
    await settle(screen.focus ?? undefined, [said]);
  };

  const add = async () => {
    const saved = await blockForm({ kind: "add", date: "today", startFollowsDay: true }, await freshBlock(undefined, 60), "New block");
    // A new series' row is listed under its first occurrence: focus goes to it once listed.
    if (!saved) return;
    await refresh();
    const row = (await current()).list?.items.find((item) => item.row.id.split("@")[0] === saved.series);
    await wrote(saved.said, row?.row.id);
  };
  useCommand("new-block", () => void add());

  // Edit Block is the block form, over every occurrence.
  const form = async (action: Action) => {
    if (action.kind !== "edit") return;
    try {
      const { fields, rule } = await core.seriesFields(action.target);
      // A rule is given exactly when the series repeats.
      const heading = rule ? `Change ${fields.title}, every occurrence` : `Change ${fields.title}`;
      const saved = await blockForm({ kind: "series", id: action.target }, fields, heading, rule);
      if (saved) await wrote(saved.said, props.screen.focus ?? undefined);
    } catch (error) {
      sayAll([(error as Error).message]);
    }
  };

  const act = async (event: Event) => {
    const turn = await run(event, form);
    await settle(turn.screen.focus ?? undefined, turn.said);
  };

  const onRow = async (id: string, kinds: ActionKind[]) => {
    await send({ Focus: id });
    await act({ Run: kinds });
  };

  const keys = (event: KeyboardEvent) => {
    const id = rowKey(event);
    if (!id || !items.some((item) => item.row.id === id)) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      void send({ Focus: id }).then(() => setMenu(true));
    } else if (event.key === "Delete") {
      event.preventDefault();
      event.stopPropagation();
      void onRow(id, REMOVING);
    }
  };

  const rightClick = (event: MouseEvent) => {
    const id = rowKey(event);
    if (!id) return;
    event.preventDefault();
    void send({ Focus: id }).then(() => setMenu(true));
  };

  const choose = (selection: Selection) => {
    if (selection === "all") return;
    const [key] = [...selection];
    if (key !== undefined) void send({ Focus: String(key) });
  };

  return (
    <>
      <h2>Blocks</h2>
      <p className="quiet" id="blocks-readback">
        {items.length > 0 ? list?.readback : ""}
      </p>
      <div className="buttons">
        <Button onPress={() => void add()}>Add block</Button>
        <RowMenu actions={inHand?.row.actions ?? []} onAction={(action) => void act({ Perform: action })} isOpen={menu} onOpenChange={setMenu} />
      </div>
      <div onKeyDownCapture={keys} onContextMenu={rightClick}>
        <Tree
          ref={tree}
          aria-label="Blocks"
          aria-describedby="blocks-readback"
          items={items.map((item) => ({ id: item.row.id, item }))}
          selectionMode="single"
          selectionBehavior="replace"
          disallowEmptySelection
          selectedKeys={selected}
          onSelectionChange={choose}
          onAction={(key: Key) => void onRow(String(key), ["edit"])}
          renderEmptyState={() => <p className="quiet">{list?.empty}</p>}
        >
          {(node) => (
            <TreeItem id={node.id} textValue={node.item.text}>
              <TreeItemContent>
                <div className="row">{node.item.text}</div>
              </TreeItemContent>
            </TreeItem>
          )}
        </Tree>
      </div>
    </>
  );
}
