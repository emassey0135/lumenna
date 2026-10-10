// Every block series: its actions are the core's, in its menu; Enter changes one, Delete
// deletes it. A series is changed whole here; one day of it is changed from the day.

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { blockForm, freshBlock } from "./BlockForm";
import { core } from "./core";
import type { Action, Line } from "./core";
import { byKind, perform, REMOVING } from "./actions";
import { useCommand } from "./commands";
import { rowKey, useLanding } from "./landing";
import { asksForMenu, RowMenu } from "./RowMenu";
import { say } from "./say";

export function Blocks(props: { revision: number; onChanged: () => void }) {
  const [lines, setLines] = useState<Line[]>([]);
  const [readback, setReadback] = useState("");
  const [empty, setEmpty] = useState("");
  const [selected, setSelected] = useState<string | undefined>();
  const [menu, setMenu] = useState(false);
  const tree = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let current = true;
    core.blocks().then(
      (found) => {
        if (!current) return;
        setLines(found.lines);
        setReadback(found.readback);
        setEmpty(found.empty ?? "");
      },
      (error: Error) => current && setReadback(error.message),
    );
    return () => {
      current = false;
    };
  }, [props.revision]);

  const ids = useMemo(() => lines.map((line) => line.row.id), [lines]);
  const land = useLanding(tree, ids, setSelected);
  const line = (id: string | undefined) => lines.find((l) => l.row.id === id);

  const changed = (said: string, keep: string | undefined, index: number) => {
    land(keep, index);
    props.onChanged();
    say(said);
  };

  const add = async () => {
    const saved = await blockForm({ kind: "add", date: "today" }, await freshBlock("09:00", 60), "New block");
    // A new series' row is listed under its first occurrence, so land near where it was.
    if (saved) changed(saved.said, undefined, ids.indexOf(selected ?? ""));
  };
  useCommand("new-block", () => void add());


  const edit = async (at: Line) => {
    try {
      const { fields, rule } = await core.seriesFields(at.row.id);
      // A rule is given exactly when the series repeats.
      const heading = rule ? `Change ${fields.title}, every occurrence` : `Change ${fields.title}`;
      const saved = await blockForm({ kind: "series", id: at.row.id }, fields, heading, rule);
      if (saved) changed(saved.said, at.row.id, ids.indexOf(at.row.id));
    } catch (error) {
      say((error as Error).message);
    }
  };

  // Runs one of a series' actions; Edit Block is the block form.
  const run = async (at: Line, action: Action) => {
    const done = await perform(action, () => edit(at));
    if (done) changed(done.said, at.row.id, ids.indexOf(at.row.id));
  };

  const keys = (event: KeyboardEvent) => {
    const at = line(rowKey(event));
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      setSelected(at.row.id);
      setMenu(true);
    } else if (event.key === "Delete") {
      const action = byKind(at.row.actions, ...REMOVING);
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      void run(at, action);
    }
  };

  const rightClick = (event: MouseEvent) => {
    const id = rowKey(event);
    if (!id) return;
    event.preventDefault();
    setSelected(id);
    setMenu(true);
  };

  const choose = (selection: Selection) => {
    if (selection === "all") return;
    const [key] = [...selection];
    if (key !== undefined) setSelected(String(key));
  };

  return (
    <>
      <h2>Blocks</h2>
      <p className="quiet" id="blocks-readback">
        {readback}
      </p>
      <div className="buttons">
        <Button onPress={() => void add()}>Add block</Button>
        <RowMenu
          actions={line(selected)?.row.actions ?? []}
          onAction={(action) => {
            const at = line(selected);
            if (at) void run(at, action);
          }}
          isOpen={menu}
          onOpenChange={setMenu}
        />
      </div>
      <div onKeyDownCapture={keys} onContextMenu={rightClick}>
        <Tree
          ref={tree}
          aria-label="Blocks"
          aria-describedby="blocks-readback"
          items={lines.map((l) => ({ id: l.row.id, line: l }))}
          selectionMode="single"
          selectionBehavior="replace"
          disallowEmptySelection
          selectedKeys={selected ? [selected] : []}
          onSelectionChange={choose}
          onAction={(key: Key) => {
            const at = line(String(key));
            const action = byKind(at?.row.actions, "edit");
            if (at && action) void run(at, action);
          }}
          renderEmptyState={() => <p className="quiet">{empty}</p>}
        >
          {(item) => (
            <TreeItem id={item.id} textValue={item.line.text}>
              <TreeItemContent>
                <div className="row">{item.line.text}</div>
              </TreeItemContent>
            </TreeItem>
          )}
        </Tree>
      </div>
    </>
  );
}
