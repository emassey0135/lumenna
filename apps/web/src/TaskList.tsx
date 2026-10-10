// Tasks, with the filter above them.
//
// A React Aria tree, so a subtask's level and every row's position are the tree's to say. Each
// row's actions are the core's, in its menu (the Menu key, Shift+F10, a right-click, or
// Actions). Space runs Mark Done, Mark Not Done or Restore, whichever the row has; Delete runs
// what removes it — Move to Trash, or in the trash Delete from Trash — and Enter opens its
// details. After a change, focus goes back to the same task if it is still listed, and
// otherwise to whatever now holds its place.

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Input, Label, TextField, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { core } from "./core";
import type { Action, ActionKind, Line, Place } from "./core";
import { byKind, perform, REMOVING } from "./actions";
import { useCommand } from "./commands";
import { asksForMenu, RowMenu } from "./RowMenu";
import { rowKey, useLanding } from "./landing";
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
  const [empty, setEmpty] = useState("");
  const [collapsed, setCollapsed] = useState<Set<Key>>(new Set());
  const tree = useRef<HTMLDivElement>(null);
  const filterField = useRef<HTMLInputElement>(null);

  useEffect(() => setFilter(props.query), [props.query]);

  useEffect(() => {
    let current = true;
    core.tasks(filter, trash).then(
      (found) => {
        if (!current) return;
        setLines(found.lines);
        setReadback(found.readback);
        setEmpty(found.empty ?? "");
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

  const ids = useMemo(() => lines.map((line) => line.row.id), [lines]);
  const land = useLanding(tree, ids, props.onSelect);

  const [menu, setMenu] = useState(false);

  // Runs one of a row's actions; Edit Details is the details beside the list.
  const run = async (action: Action) => {
    const index = Math.max(0, lines.findIndex((l) => l.row.id === action.target));
    const done = await perform(action, () => {
      props.onSelect(action.target);
      props.onOpen();
    });
    if (!done) return;
    land(action.target, index);
    props.onChanged();
    say(done.said);
  };

  // Filter Tasks: the filter, all of it selected so typing replaces it. Not in the trash.
  useCommand("filter", () => {
    if (trash || !filterField.current) return false;
    filterField.current.focus();
    filterField.current.select();
  });

  // The Task menu's commands, on the task in hand here, so focus lands as a row's own action
  // does; before the details' copy of them.
  const onSelected = (...kinds: ActionKind[]) => () => {
    const line = lines.find((l) => l.row.id === props.selected);
    const action = line && byKind(line.row.actions, ...kinds);
    if (!action) return false;
    void run(action);
  };
  useCommand("mark-done", onSelected("mark_done", "mark_not_done"), 1);
  useCommand("put-in-block", onSelected("put_in_block"), 1);
  useCommand("move-to-project", onSelected("move_to_project"), 1);

  const keys = (event: KeyboardEvent) => {
    const id = rowKey(event);
    const line = lines.find((l) => l.row.id === id);
    if (!line) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      props.onSelect(line.row.id);
      setMenu(true);
      return;
    }
    const action =
      event.key === " "
        ? byKind(line.row.actions, "mark_done", "mark_not_done", "restore")
        : event.key === "Delete"
          ? byKind(line.row.actions, ...REMOVING)
          : undefined;
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    void run(action);
  };

  const rightClick = (event: MouseEvent) => {
    const id = rowKey(event);
    if (!id) return;
    event.preventDefault();
    props.onSelect(id);
    setMenu(true);
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
          <Input ref={filterField} placeholder="#Work & overdue, or search: words" />
        </TextField>
      )}
      <p className="quiet" id="readback">
        {readback}
      </p>
      <div className="buttons">
        <RowMenu
          actions={lines.find((l) => l.row.id === props.selected)?.row.actions ?? []}
          onAction={(action) => void run(action)}
          isOpen={menu}
          onOpenChange={setMenu}
        />
      </div>
      {/* Space and Delete are caught on the way down, before the tree takes Space for selection. */}
      <div onKeyDownCapture={keys} onContextMenu={rightClick}>
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
        renderEmptyState={() => <p className="quiet">{empty}</p>}
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
