// The planner: a day as it is lived, as a tree.
//
// The first row is the summary — what a glance at a timeline gives a sighted user. Then
// blocks in time order with their sittings beneath them, free time as rows of its own, and
// now as a position rather than a highlight. Opening the day puts it on now, not midnight.
//
// Each row's actions are the core's, in its menu (the Menu key, Shift+F10, a right-click, or
// Actions). On a sitting, Space runs its timer action (start, pause or resume) and Delete
// unassigns it; on a block, Enter changes it and Delete deletes it; on free time, Enter adds a
// block there; on a cancelled day, Enter restores it.

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Toolbar, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { blockForm, freshBlock } from "./BlockForm";
import { core } from "./core";
import type { Action, DayRow, PlanBlock } from "./core";
import { askText, byKind, perform, REMOVING } from "./actions";
import { useCommand } from "./commands";
import { rowKey, useLanding } from "./landing";
import { choose } from "./Prompts";
import { asksForMenu, RowMenu } from "./RowMenu";
import { say } from "./say";

interface Shown {
  date: string;
  title: string;
  rows: DayRow[];
}

/** Every row in the order the tree shows them, sittings after their block. */
function flatten(rows: DayRow[]): DayRow[] {
  return rows.flatMap((row) => [row, ...flatten(row.children)]);
}

/** An ISO date `days` days on, counted in the calendar rather than in hours. */
function step(iso: string, days: number): string {
  const [year, month, day] = iso.split("-").map(Number);
  const date = new Date(Date.UTC(year, month - 1, day + days));
  return date.toISOString().slice(0, 10);
}

export function Day(props: {
  revision: number;
  onChanged: () => void;
  /** The task a selected sitting is for, which the details show. */
  onSelectTask: (id: string | undefined) => void;
  /** Opens a task's details. */
  onOpenTask: (id: string) => void;
}) {
  // The day shown; undefined follows today, so the day moves on at midnight.
  const [date, setDate] = useState<string | undefined>();
  const [shown, setShown] = useState<Shown | undefined>();
  const [problem, setProblem] = useState<string | undefined>();
  const [selected, setSelected] = useState<string | undefined>();
  const [collapsed, setCollapsed] = useState<Set<Key>>(new Set());
  const [menu, setMenu] = useState(false);
  const [minute, setMinute] = useState(0);
  const tree = useRef<HTMLDivElement>(null);
  // Whether the day has opened on now yet: once, so coming back does not move the selection.
  const landed = useRef(false);
  // Whether to say the summary once the day being moved to is showing.
  const announce = useRef(false);
  // Whether to put focus on now once today is showing (Go to Now).
  const toNow = useRef(false);

  // Today moves on with the clock: the now row, and which block is now.
  useEffect(() => {
    if (date !== undefined) return;
    const timer = setInterval(() => setMinute((m) => m + 1), 60_000);
    return () => clearInterval(timer);
  }, [date]);

  useEffect(() => {
    let current = true;
    core.day(date).then(
      (day) => {
        if (!current) return;
        setShown(day);
        setProblem(undefined);
      },
      (error: Error) => current && setProblem(error.message),
    );
    return () => {
      current = false;
    };
  }, [date, props.revision, minute]);

  const flat = useMemo(() => (shown ? flatten(shown.rows) : []), [shown]);
  const keys = useMemo(() => flat.map((row) => row.key), [flat]);
  const row = (key: string | undefined) => flat.find((r) => r.key === key);

  const select = (key: string | undefined) => {
    setSelected(key);
    props.onSelectTask(row(key)?.sitting?.task);
  };
  const land = useLanding(tree, keys, select);

  // The first time, the day opens on now; a new day opens on its summary, and says it.
  useEffect(() => {
    if (!shown) return;
    if (!landed.current) {
      landed.current = true;
      const now = flat.find((r) => r.kind === "now" || (r.kind === "block" && r.block?.when === "now"));
      select((now ?? flat[0])?.key);
    } else if (selected === undefined || !keys.includes(selected)) {
      select(flat[0]?.key);
    }
    if (toNow.current && date === undefined) {
      toNow.current = false;
      const now = flat.find((r) => r.kind === "now" || (r.kind === "block" && r.block?.when === "now"));
      const at = now ?? flat[0];
      if (at) {
        select(at.key);
        // Once the tree has drawn the row, which may take it a frame or two.
        let tries = 0;
        const focus = () => {
          const found = tree.current?.querySelector<HTMLElement>(`[role="row"][data-key="${CSS.escape(at.key)}"]`);
          if (found) found.focus();
          else if (tries++ < 30) requestAnimationFrame(focus);
        };
        requestAnimationFrame(focus);
      }
      return;
    }
    if (announce.current) {
      announce.current = false;
      if (flat[0]) say(flat[0].text);
    }
  }, [shown]);

  // Moves to another day, and says its summary: a new day is a new screen's worth.
  const go = (to: string | undefined) => {
    if (to === date) {
      if (flat[0]) say(flat[0].text);
      return;
    }
    setDate(to);
    setSelected(undefined);
    announce.current = true;
  };

  const today = shown?.date ?? "";

  // From free time, at its start; otherwise when the core says a block on this day starts.
  const addBlock = async (start?: string, minutes = 60) => {
    const day = shown?.date ?? "today";
    const fields = await freshBlock(start, Math.min(minutes, 720), day);
    const saved = await blockForm({ kind: "add", date: day, startFollowsDay: start === undefined }, fields, "New block");
    if (!saved) return;
    land(saved.series ? `block:${saved.series}@${today}` : undefined, keys.indexOf(selected ?? ""));
    props.onChanged();
    say(saved.said);
  };

  // Changes a block — asking "this day, or every day?" of a repeating one, never guessing.
  const edit = async (block: PlanBlock) => {
    let heading = `Change ${block.title}`;
    let purpose: Parameters<typeof blockForm>[0] = { kind: "series", id: block.series };
    if (block.repeats) {
      const which = await choose(heading, "Which occurrences?", [`${shown?.title ?? "This day"} only`, "Every occurrence"]);
      if (which === undefined) return;
      if (which === 0) {
        purpose = { kind: "occurrence", series: block.series, date: today };
        heading = `Change ${block.title}, this day only`;
      } else {
        heading = `Change ${block.title}, every occurrence`;
      }
    }
    // One day alone starts from that day's block; every occurrence, from the series as it is
    // rather than as this day shows it.
    let fields;
    let rule: string | undefined;
    try {
      if (purpose.kind === "series") ({ fields, rule } = await core.seriesFields(block.series));
      else fields = await core.dayFields(block);
    } catch (error) {
      say((error as Error).message);
      return;
    }
    const saved = await blockForm(purpose, fields, heading, rule);
    if (!saved) return;
    land(`block:${block.id}`, keys.indexOf(`block:${block.id}`));
    props.onChanged();
    say(saved.said);
  };

  const goToDay = async () => {
    const phrase = await askText((await core.ownQuestions()).go_to_day);
    if (!phrase?.trim()) return;
    try {
      const day = await core.day(phrase);
      go(day.date);
    } catch (error) {
      say((error as Error).message);
    }
  };

  // The Day menu's commands, while the day is shown.
  useCommand("previous-day", () => {
    if (!shown) return false;
    go(step(shown.date, -1));
  });
  useCommand("next-day", () => {
    if (!shown) return false;
    go(step(shown.date, 1));
  });
  useCommand("go-to-day", () => void goToDay());
  useCommand("go-to-now", () => {
    toNow.current = true;
    if (date === undefined && shown) {
      // Today is showing already: land on now at once.
      setShown({ ...shown });
    } else {
      setDate(undefined);
    }
  });
  useCommand("new-block", () => void addBlock());

  // Runs one of a row's actions. The forms are this client's own: a block's, a new block's in
  // free time, and the details of a sitting's task.
  const run = async (at: DayRow, action: Action) => {
    const index = Math.max(0, keys.indexOf(at.key));
    const done = await perform(action, async () => {
      if (action.kind === "edit" && at.block) await edit(at.block);
      else if (action.kind === "add_block" && at.free) await addBlock(action.other ?? at.free.start, at.free.minutes);
      else if (action.kind === "edit_task") props.onOpenTask(action.target);
    });
    if (!done) return;
    land(at.key, index);
    props.onChanged();
    say(done.said);
  };

  // Enter: what a row is for.
  const activate = (key: Key) => {
    const at = row(String(key));
    const action = byKind(at?.actions, "edit", "edit_task", "add_block", "restore_day");
    if (at && action) void run(at, action);
  };

  const keysDown = (event: KeyboardEvent) => {
    const at = row(rowKey(event));
    if (!at) return;
    if (asksForMenu(event)) {
      event.preventDefault();
      select(at.key);
      setMenu(true);
      return;
    }
    const action =
      event.key === " " && at.kind === "sitting"
        ? byKind(at.actions, "start_timer", "pause_timer", "resume_timer")
        : event.key === "Delete"
          ? byKind(at.actions, ...REMOVING)
          : undefined;
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    void run(at, action);
  };

  const rightClick = (event: MouseEvent) => {
    const key = rowKey(event);
    if (!key) return;
    event.preventDefault();
    select(key);
    setMenu(true);
  };

  const chosen = (selection: Selection) => {
    if (selection === "all") return;
    const [key] = [...selection];
    if (key !== undefined) select(String(key));
  };

  const expanded = useMemo(
    () => new Set<Key>(flat.filter((r) => r.children.length > 0 && !collapsed.has(r.key)).map((r) => r.key)),
    [flat, collapsed],
  );

  return (
    <>
      <h2>{shown?.title ?? "Today"}</h2>
      {/* A toolbar is one Tab stop, its buttons a press of the arrows apart (ARIA's toolbar pattern). */}
      <Toolbar className="buttons" aria-label="Day">
        <Button isDisabled={!shown} onPress={() => shown && go(step(shown.date, -1))}>
          Previous day
        </Button>
        <Button onPress={() => go(undefined)}>Today</Button>
        <Button isDisabled={!shown} onPress={() => shown && go(step(shown.date, 1))}>
          Next day
        </Button>
        <Button onPress={() => void goToDay()}>Go to day</Button>
        <Button onPress={() => void addBlock()}>Add block</Button>
        <RowMenu
          actions={row(selected)?.actions ?? []}
          onAction={(action) => {
            const at = row(selected);
            if (at) void run(at, action);
          }}
          isOpen={menu}
          onOpenChange={setMenu}
        />
      </Toolbar>
      {problem && <p role="alert">{problem}</p>}
      {/* Space and Delete are caught on the way down, before the tree takes Space for selection. */}
      <div onKeyDownCapture={keysDown} onContextMenu={rightClick}>
        <Tree
          ref={tree}
          aria-label="The day"
          items={shown?.rows ?? []}
          selectionMode="single"
          selectionBehavior="replace"
          disallowEmptySelection
          selectedKeys={selected ? [selected] : []}
          onSelectionChange={chosen}
          expandedKeys={expanded}
          onExpandedChange={(open) =>
            setCollapsed(new Set(flat.filter((r) => r.children.length > 0 && !open.has(r.key)).map((r) => r.key)))
          }
          onAction={activate}
        >
          {function item(node: DayRow) {
            return (
              <TreeItem id={node.key} textValue={node.text}>
                <TreeItemContent>
                  {({ hasChildItems }) => (
                    <div className={`row ${node.kind}`}>
                      {hasChildItems && (
                        <Button slot="chevron" className="chevron">
                          ›
                        </Button>
                      )}
                      {node.text}
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
