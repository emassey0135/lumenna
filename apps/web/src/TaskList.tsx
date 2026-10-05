// Tasks, with the filter above them (§16.1: task list + filter entry).
//
// A React Aria tree, so a subtask's level and every row's position are the tree's to say. Space
// checks a task off, Delete trashes it — or, in the trash, Space restores and Delete erases —
// and Enter opens its details. After a change, focus goes back to the same task if it is still
// listed, and otherwise to whatever now holds its place (§13).

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { Button, Collection, Input, Label, TextField, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { core } from "./core";
import type { Line, Place } from "./core";
import { say } from "./say";
import { nest, parents } from "./tree";
import type { Node } from "./tree";

export function TaskList(props: {
  revision: number;
  place: Place;
  title: string;
  query: string;
  selected: string | undefined;
  onSelect: (id: string | undefined) => void;
  onOpen: () => void;
  onChanged: () => void;
}) {
  const trash = props.place === "Trash";
  const [filter, setFilter] = useState(props.query);
  const [lines, setLines] = useState<Line[]>([]);
  const [readback, setReadback] = useState("");
  const [collapsed, setCollapsed] = useState<Set<Key>>(new Set());
  const tree = useRef<HTMLDivElement>(null);
  // Where focus goes once the rows a change produced are shown.
  const landing = useRef<{ id?: string; index: number } | undefined>(undefined);

  useEffect(() => setFilter(props.query), [props.query]);

  useEffect(() => {
    let current = true;
    core.tasks(filter, trash).then(
      (found) => {
        if (!current) return;
        setLines(found.lines);
        setReadback(found.readback);
      },
      // A filter still being typed may not read yet: the rows stay, and the readback says why.
      (error: Error) => current && setReadback(error.message),
    );
    return () => {
      current = false;
    };
  }, [filter, trash, props.revision]);

  const nodes = useMemo(() => nest(lines, (line) => line.row.id, (line) => line.row.depth), [lines]);
  const expanded = useMemo(() => new Set<Key>(parents(nodes).filter((id) => !collapsed.has(id))), [nodes, collapsed]);

  // Focus lands somewhere predictable after a change: the same task, else what holds its place.
  // The tree builds its collection in a pass of its own, so the rows on the page can still be
  // the old ones when the lines change: land only on rows that are the current lines' own.
  useEffect(() => {
    const target = landing.current;
    if (!target) return;
    landing.current = undefined;
    const current = new Set(lines.map((line) => line.row.id));
    let frame = 0;
    let tries = 0;
    const land = () => {
      const rows = [...(tree.current?.querySelectorAll<HTMLElement>('[role="row"]') ?? [])].filter(
        (row) => row.dataset.key !== undefined,
      );
      const stale = rows.some((row) => !current.has(row.dataset.key!));
      if ((stale || rows.length === 0) && lines.length > 0 && tries++ < 20) {
        frame = requestAnimationFrame(land);
        return;
      }
      const row = rows.find((r) => r.dataset.key === target.id) ?? rows[Math.min(target.index, rows.length - 1)];
      if (row) {
        row.focus();
        props.onSelect(row.dataset.key);
      } else {
        // Nothing left to hold focus: the list itself does, so it is not lost to the page.
        tree.current?.focus();
        props.onSelect(undefined);
      }
    };
    land();
    return () => cancelAnimationFrame(frame);
  }, [lines]);

  const change = async (operation: Promise<string>, keep: string | undefined, index: number) => {
    try {
      const said = await operation;
      landing.current = { id: keep, index };
      props.onChanged();
      say(said);
    } catch (error) {
      say((error as Error).message);
    }
  };

  const keys = (event: KeyboardEvent) => {
    if (event.key !== " " && event.key !== "Delete") return;
    const id = (event.target as HTMLElement).closest<HTMLElement>('[role="row"]')?.dataset.key;
    const index = lines.findIndex((l) => l.row.id === id);
    const line = lines[index];
    if (!line) return;
    if (event.key === " ") {
      event.preventDefault();
      event.stopPropagation();
      if (trash) void change(core.restore(line.row.id), undefined, index);
      else void change(core.complete(line.row.id, line.row.checked === true), line.row.id, index);
    } else if (event.key === "Delete") {
      event.preventDefault();
      event.stopPropagation();
      if (!trash) void change(core.trash(line.row.id), undefined, index);
      else if (confirm(`Erase ${line.row.title}? It and its history are deleted for good. This cannot be undone.`)) {
        void change(core.erase(line.row.id), undefined, index);
      }
    }
  };

  const choose = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    props.onSelect(chosen === undefined ? undefined : String(chosen));
  };


  return (
    <>
      <h2>{props.title}</h2>
      {!trash && (
        <TextField
          className="field"
          value={filter}
          onChange={setFilter}
          onKeyDown={(event) => {
            // Finishing the filter says what it found, then goes to it.
            if (event.key === "Enter") {
              say(readback);
              tree.current?.querySelector<HTMLElement>('[role="row"]')?.focus();
            }
          }}
        >
          <Label>Filter</Label>
          <Input placeholder="#Work & overdue, or search: words" />
        </TextField>
      )}
      <p className="quiet" id="readback">
        {readback}
      </p>
      {/* Space and Delete are caught on the way down, before the tree takes Space for selection. */}
      <div onKeyDownCapture={keys}>
      <Tree
        ref={tree}
        aria-label={props.title}
        aria-describedby="readback"
        items={nodes}
        selectionMode="single"
        selectionBehavior="replace"
        disallowEmptySelection
        selectedKeys={props.selected ? [props.selected] : []}
        onSelectionChange={choose}
        expandedKeys={expanded}
        onExpandedChange={(keys) => setCollapsed(new Set(parents(nodes).filter((id) => !keys.has(id))))}
        onAction={() => props.onOpen()}
        renderEmptyState={() => <p className="quiet">Nothing here.</p>}
      >
        {function item(node: Node<Line>) {
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
    </>
  );
}
