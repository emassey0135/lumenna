// The places, as the desktop apps list them —
// the list itself is crates/desktop's `places::sidebar`, run in the worker.
//
// Projects, labels and saved filters are managed here, as on the desktop: each row's menu
// (the Menu key, Shift+F10, a right-click, or Actions) has what can be done to it, the
// headings have New, and Delete deletes, asking first. Renaming or making a place goes to it.

import { useEffect, useMemo, useRef, useState } from "react";
import type { FocusEvent, KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { core } from "./core";
import type { SidebarEntry, Place } from "./core";
import { rowKey } from "./landing";
import { ask, choose, confirm, pick } from "./Prompts";
import { asksForMenu, RowMenu } from "./RowMenu";
import type { Action } from "./RowMenu";
import { say } from "./say";
import { nest, parents } from "./tree";
import type { Node } from "./tree";

/** What identifies an entry across reloads: its kind, as written. */
export function placeKey(place: Place): string {
  return JSON.stringify({ Place: place });
}

function key(entry: SidebarEntry): string {
  return JSON.stringify(entry.kind);
}

/** Whether a row has a Delete: projects, labels and saved filters do. */
function deletable(entry: SidebarEntry): boolean {
  if (!("Place" in entry.kind)) return false;
  const place = entry.kind.Place;
  return typeof place === "object" && ("Project" in place || "Label" in place || "Filter" in place);
}

export function Sidebar(props: {
  revision: number;
  place: Place;
  onPlace: (place: Place) => void;
  onChanged: () => void;
}) {
  const [entries, setEntries] = useState<SidebarEntry[]>([]);
  const [expanded, setExpanded] = useState<Set<Key> | undefined>();
  // The row in hand: the one focus was last on. Headings are never selected, so it is not
  // the selection.
  const [held, setHeld] = useState<string | undefined>();
  const [menu, setMenu] = useState(false);
  const tree = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let current = true;
    void core.sidebar().then((entries) => current && setEntries(entries));
    return () => {
      current = false;
    };
  }, [props.revision]);

  const nodes = useMemo(() => nest(entries, key, (entry) => entry.depth), [entries]);
  // Everything open at first; after that, as the person leaves it.
  const shown = expanded ?? new Set<Key>(parents(nodes));
  const entry = (k: string | undefined) => entries.find((e) => key(e) === k);

  const go = (selection: Selection) => {
    if (selection === "all") return;
    const [chosen] = [...selection];
    const found = entry(String(chosen));
    if (found && "Place" in found.kind) props.onPlace(found.kind.Place);
  };

  /** Runs a change, says what it did, and — when it made, renamed or removed a place — goes to `then`. */
  const change = async (operation: Promise<{ said: string; changed: boolean }>, then?: Place) => {
    try {
      const done = await operation;
      props.onChanged();
      if (then && done.changed) props.onPlace(then);
      say(done.said);
    } catch (error) {
      say((error as Error).message);
    }
  };

  const named = (title: string, initial = "", description = "") =>
    ask(title, "Name", description, initial, (text) => (text.trim() ? undefined : "It needs a name."));

  const newProject = async (parent?: string) => {
    const name = await named(parent ? `New Project in ${parent}` : "New Project");
    if (name) await change(core.addProject(name.trim(), parent), { Project: name.trim() });
  };

  const newLabel = async () => {
    const name = await named("New Label");
    if (name) await change(core.addLabel(name.trim()), { Label: name.trim() });
  };

  const newFilter = async () => {
    const name = await named("New Saved Filter");
    if (!name) return;
    const query = await ask(`Query for ${name.trim()}`, "Query", "Such as #Work & overdue, or p1 | today.");
    if (query === undefined) return;
    await change(core.addFilter(name.trim(), query), { Filter: { name: name.trim(), query } });
  };

  const project = (name: string, archived: boolean): Action[] => [
    {
      id: "rename",
      label: "Rename…",
      run: async () => {
        const to = await named(`Rename ${name}`, name);
        if (to && to.trim() !== name) await change(core.renameProject(name, to.trim()), { Project: to.trim() });
      },
    },
    { id: "inside", label: "New Project Inside…", run: () => void newProject(name) },
    {
      id: "under",
      label: "Move Under…",
      run: async () => {
        // By position, so a project could even be called "The top level".
        const choices = [undefined, ...(await core.projects()).filter((p) => p !== name)];
        const items = choices.map((p, index) => ({ id: String(index), text: p ?? "The top level" }));
        const chosen = await pick(`Move ${name}`, "Under", items);
        if (chosen !== undefined) await change(core.moveProject(name, choices[Number(chosen)]));
      },
    },
    { id: "up", label: "Move Up", run: () => void change(core.reorderProject(name, "Up")) },
    { id: "down", label: "Move Down", run: () => void change(core.reorderProject(name, "Down")) },
    {
      id: "weight",
      label: "Weight…",
      run: async () => {
        const description = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.";
        // The surface reads it, refusing a typo at the field rather than taking it as inherit.
        const text = await ask(`Weight of ${name}`, "Weight", description, "1.0", (text) => core.weightProblem(text));
        if (text !== undefined) await change(core.weighProject(name, text));
      },
    },
    { id: "archive", label: archived ? "Unarchive" : "Archive", run: () => void change(core.archiveProject(name)) },
    {
      id: "delete",
      label: "Delete…",
      run: async () => {
        const how = await choose(`Delete ${name}?`, "Its tasks can go to the trash with it, or move to the Inbox.", [
          "Delete and Trash Its Tasks",
          "Delete and Keep Its Tasks",
        ]);
        if (how !== undefined) await change(core.deleteProject(name, how === 1), "Tasks");
      },
    },
  ];

  const label = (name: string): Action[] => [
    {
      id: "rename",
      label: "Rename…",
      run: async () => {
        const to = await named(`Rename ${name}`, name);
        if (to && to.trim() !== name) await change(core.renameLabel(name, to.trim()), { Label: to.trim() });
      },
    },
    {
      id: "merge",
      label: "Merge Into…",
      run: async () => {
        // For when a typo made a near-duplicate: this one's tasks move to the other.
        const others = (await core.labels()).filter((l) => l !== name).map((l) => ({ id: l, text: l }));
        const into = await pick(`Merge ${name}`, "Into", others);
        if (into) await change(core.mergeLabels(name, into), { Label: into });
      },
    },
    {
      id: "colour",
      label: "Colour…",
      run: async () => {
        const colour = await ask(`Colour for ${name}`, "Colour", "A colour name, such as red or teal, or none. The name always shows too.");
        if (colour === undefined) return;
        const none = !colour.trim() || colour.trim().toLowerCase() === "none";
        await change(core.recolourLabel(name, none ? undefined : colour.trim()));
      },
    },
    { id: "up", label: "Move Up", run: () => void change(core.reorderLabel(name, "Up")) },
    { id: "down", label: "Move Down", run: () => void change(core.reorderLabel(name, "Down")) },
    {
      id: "delete",
      label: "Delete…",
      run: async () => {
        if (await confirm(`Delete ${name}?`, "Tasks wearing it stay; they just stop showing it.", "Delete")) {
          await change(core.deleteLabel(name), "Tasks");
        }
      },
    },
  ];

  const filter = (name: string, query: string): Action[] => [
    {
      id: "rename",
      label: "Rename…",
      run: async () => {
        const to = await named(`Rename ${name}`, name);
        if (to && to.trim() !== name) await change(core.editFilter(name, to.trim(), undefined), { Filter: { name: to.trim(), query } });
      },
    },
    {
      id: "query",
      label: "Change Query…",
      run: async () => {
        const to = await ask(`Query for ${name}`, "Query", "Such as #Work & overdue, or p1 | today.", query);
        if (to !== undefined && to !== query) await change(core.editFilter(name, undefined, to), { Filter: { name, query: to } });
      },
    },
    { id: "up", label: "Move Up", run: () => void change(core.reorderFilter(name, "Up")) },
    { id: "down", label: "Move Down", run: () => void change(core.reorderFilter(name, "Down")) },
    {
      id: "delete",
      label: "Delete…",
      run: async () => {
        if (await confirm(`Delete ${name}?`, "The tasks it shows are not touched.", "Delete")) {
          await change(core.deleteFilter(name), "Tasks");
        }
      },
    },
  ];

  /** What can be done to a row, as its menu lists it. */
  const actions = (at: SidebarEntry | undefined): Action[] => {
    if (!at) return [];
    if ("Group" in at.kind) {
      if (at.kind.Group === "Projects") return [{ id: "new", label: "New Project…", run: () => void newProject() }];
      if (at.kind.Group === "Labels") return [{ id: "new", label: "New Label…", run: () => void newLabel() }];
      return [{ id: "new", label: "New Saved Filter…", run: () => void newFilter() }];
    }
    const place = at.kind.Place;
    if (typeof place !== "object") return [];
    if ("Project" in place) return project(place.Project, at.archived);
    if ("Label" in place) return label(place.Label);
    if ("Filter" in place) return filter(place.Filter.name, place.Filter.query);
    return [];
  };

  const keys = (event: KeyboardEvent) => {
    const at = entry(rowKey(event));
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      setHeld(key(at));
      setMenu(true);
    } else if (event.key === "Delete" && deletable(at)) {
      // Delete here does what it does in every other list, asking first as the menu's does.
      event.preventDefault();
      event.stopPropagation();
      actions(at)
        .find((action) => action.id === "delete")
        ?.run();
    }
  };

  const rightClick = (event: MouseEvent) => {
    const k = rowKey(event);
    if (!k) return;
    event.preventDefault();
    setHeld(k);
    setMenu(true);
  };

  return (
    <>
      <div
        onKeyDownCapture={keys}
        onContextMenu={rightClick}
        onFocusCapture={(event: FocusEvent) => {
          const k = rowKey(event);
          if (k) setHeld(k);
        }}
      >
        <Tree
          ref={tree}
          aria-label="Places"
          items={nodes}
          selectionMode="single"
          disallowEmptySelection
          selectedKeys={[placeKey(props.place)]}
          onSelectionChange={go}
          expandedKeys={shown}
          onExpandedChange={setExpanded}
        >
          {function item(node: Node<SidebarEntry>) {
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
      <div className="buttons">
        <RowMenu actions={actions(entry(held))} isOpen={menu} onOpenChange={setMenu} label="Place Actions" />
      </div>
    </>
  );
}
