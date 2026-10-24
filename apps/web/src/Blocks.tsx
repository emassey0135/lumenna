// Every block series, as the app lists them: its actions are the core's, in its menu; Enter
// changes one, Delete deletes it, asking first; Add block makes one.

import { useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import type { Screen, ShellEvent } from "./core";
import { ActionKind } from "./core";
import { actOn, keyPressed, rowShown, shortActions } from "./actions";
import { useCommand } from "./commands";
import { rowKey } from "./landing";
import { asksForMenu, RowMenu } from "./RowMenu";
import { send } from "./session";
import { SwatchMark } from "./Swatch";
import { brailleOnRow } from "./braille";

export function Blocks(props: { screen: Screen }) {
  const list = props.screen.list;
  const items = useMemo(() => list?.items ?? [], [list]);
  const tree = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState(false);
  // The same keys while the row in hand is the same: a new array each time is a new selection
  // to React Aria, which says "… selected" for each while the tree has focus.
  const selected = useMemo(() => (props.screen.focus ? [props.screen.focus] : []), [props.screen.focus]);
  const inHand = items.find((item) => item.row.id === props.screen.focus);

  const add = () => void send({ tag: "Block", value0: { tag: "New", start: null, minutes: null } });
  useCommand("new-block", add);

  /** Sends `event`; what comes back lands focus and is said as every turn is (session.ts). */
  const act = (event: ShellEvent) => void send(event);

  const onRow = async (id: string, kinds: ActionKind[]) => {
    await send({ tag: "Focus", value0: id });
    act({ tag: "Run", value0: kinds });
  };

  const keys = (event: KeyboardEvent) => {
    const id = rowKey(event);
    if (!id || !items.some((item) => item.row.id === id)) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      void send({ tag: "Focus", value0: id }).then(() => setMenu(true));
    } else if (event.key === "Delete") {
      event.preventDefault();
      event.stopPropagation();
      void send({ tag: "Focus", value0: id }).then(() => act(keyPressed("delete")));
    }
  };

  const rightClick = (event: MouseEvent) => {
    const id = rowKey(event);
    if (!id) return;
    event.preventDefault();
    void send({ tag: "Focus", value0: id }).then(() => setMenu(true));
  };

  const choose = (selection: Selection) => {
    if (selection === "all") return;
    const [key] = [...selection];
    if (key !== undefined) void send({ tag: "Focus", value0: String(key) });
  };

  return (
    <>
      <h2>Blocks</h2>
      <p className="quiet" id="blocks-readback">
        {items.length > 0 ? list?.readback : ""}
      </p>
      <div className="buttons">
        <Button onPress={add}>Add block</Button>
        <RowMenu actions={shortActions(list, inHand)} onAction={(action) => inHand && act(actOn(rowShown(inHand.row.id), action))} isOpen={menu} onOpenChange={setMenu} />
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
          onAction={(key: Key) => void onRow(String(key), [ActionKind.Edit])}
          renderEmptyState={() => <p className="quiet">{list?.empty}</p>}
        >
          {(node) => (
            <TreeItem id={node.id} textValue={node.item.text}>
              <TreeItemContent>
                <div className="row" ref={brailleOnRow(node.item.text, node.item.braille)}>
                  <SwatchMark swatch={node.item.row.swatch} />
                  {node.item.text}
                </div>
              </TreeItemContent>
            </TreeItem>
          )}
        </Tree>
      </div>
    </>
  );
}
