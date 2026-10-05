// Every block series (§16.1: block editor): Enter changes one, Delete deletes it, and the
// rest is in its menu. A series is changed whole here; one day of it is changed from the day.

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { blockForm, freshBlock } from "./BlockForm";
import { core } from "./core";
import type { Line } from "./core";
import { rowKey, useLanding } from "./landing";
import { confirm } from "./Prompts";
import { asksForMenu, RowMenu } from "./RowMenu";
import type { Action } from "./RowMenu";
import { say } from "./say";

export function Blocks(props: { revision: number; onChanged: () => void }) {
  const [lines, setLines] = useState<Line[]>([]);
  const [readback, setReadback] = useState("");
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
    const saved = await blockForm({ kind: "add", date: "today" }, await freshBlock("09:00", 60), "New Block");
    // A new series' row is listed under its first occurrence, so land near where it was.
    if (saved) changed(saved.said, undefined, ids.indexOf(selected ?? ""));
  };

  const edit = async (at: Line) => {
    try {
      const { fields, rule } = await core.seriesFields(at.row.id);
      // A rule is given exactly when the series repeats.
      const heading = rule ? `Change ${fields.title}, Every Occurrence` : `Change ${fields.title}`;
      const saved = await blockForm({ kind: "series", id: at.row.id }, fields, heading, rule);
      if (saved) changed(saved.said, at.row.id, ids.indexOf(at.row.id));
    } catch (error) {
      say((error as Error).message);
    }
  };

  const remove = async (at: Line) => {
    if (!(await confirm(`Delete ${at.row.title}?`, "Every occurrence goes, with what is assigned to it.", "Delete"))) return;
    try {
      changed(await core.deleteBlock(at.row.id), undefined, ids.indexOf(at.row.id));
    } catch (error) {
      say((error as Error).message);
    }
  };

  const actions = (at: Line | undefined): Action[] =>
    at
      ? [
          { id: "edit", label: "Change…", run: () => void edit(at) },
          { id: "delete", label: "Delete…", run: () => void remove(at) },
        ]
      : [];

  const keys = (event: KeyboardEvent) => {
    const at = line(rowKey(event));
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      setSelected(at.row.id);
      setMenu(true);
    } else if (event.key === "Delete") {
      event.preventDefault();
      event.stopPropagation();
      void remove(at);
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
        <Button onPress={() => void add()}>Add Block…</Button>
        <RowMenu actions={actions(line(selected))} isOpen={menu} onOpenChange={setMenu} />
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
            if (at) void edit(at);
          }}
          renderEmptyState={() => <p className="quiet">No blocks yet.</p>}
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
