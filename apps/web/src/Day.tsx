// The planner: a day as it is lived, as a tree.
//
// The first row is the summary — what a glance at a timeline gives a sighted user. Then
// blocks in time order with their sittings beneath them, free time as rows of its own, and
// now as a position rather than a highlight. Opening the day puts it on now, not midnight.
//
// On a sitting, Space starts or stops its timer and Delete takes it out of the block; on a
// block, Enter changes it and Delete deletes it; on free time, Enter adds a block there.
// Everything else is in the row's menu (the Menu key, Shift+F10, or Actions).

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { Button, Collection, Tree, TreeItem, TreeItemContent } from "react-aria-components";
import type { Key, Selection } from "react-aria-components";
import { blockForm, freshBlock } from "./BlockForm";
import { core } from "./core";
import type { DayRow, PlanAssignment, PlanBlock } from "./core";
import { rowKey, useLanding } from "./landing";
import { ask, askMinutes, choose, confirm, pick } from "./Prompts";
import { asksForMenu, RowMenu } from "./RowMenu";
import type { Action } from "./RowMenu";
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

  const change = async (operation: Promise<string>, keep: string | undefined) => {
    const index = Math.max(0, keys.indexOf(selected ?? ""));
    try {
      const said = await operation;
      land(keep, index);
      props.onChanged();
      say(said);
    } catch (error) {
      say((error as Error).message);
    }
  };

  const today = shown?.date ?? "";

  const addBlock = async (start = "09:00", minutes = 60) => {
    const fields = await freshBlock(start, Math.min(minutes, 720));
    const saved = await blockForm({ kind: "add", date: shown?.date ?? "today" }, fields, "New Block");
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
      const which = await choose(heading, "Which occurrences?", [`${shown?.title ?? "This Day"} Only`, "Every Occurrence"]);
      if (which === undefined) return;
      if (which === 0) {
        purpose = { kind: "occurrence", series: block.series, date: today };
        heading = `Change ${block.title}, This Day Only`;
      } else {
        heading = `Change ${block.title}, Every Occurrence`;
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

  const remove = async (block: PlanBlock) => {
    const detail = block.repeats
      ? "Every occurrence goes, not only this day. To skip one day, cancel it instead."
      : "It goes, with what is assigned to it.";
    if (await confirm(`Delete ${block.title}?`, detail, "Delete")) await change(core.deleteBlock(block.series), undefined);
  };

  // Fills a block from the task side's opposite: from the block, pick a task.
  const assign = async (block: PlanBlock) => {
    const tasks = await core.taskChoices();
    const task = await pick(`Assign to ${block.title}`, "Task", tasks);
    if (!task) return;
    const title = tasks.find((t) => t.id === task)?.text ?? "it";
    const minutes = await askMinutes(`How Long Is ${title} Meant to Take?`, "", true);
    if (minutes === undefined) return;
    await change(core.assign(task, block.series, today, minutes ?? undefined), `block:${block.id}`);
  };

  const plannedLength = async (sitting: PlanAssignment) => {
    const current = sitting.planned_mins ? String(sitting.planned_mins) : "";
    const minutes = await askMinutes(`Planned Length of ${sitting.title}`, current, true);
    if (minutes === undefined) return;
    await change(core.planMinutes(sitting.id, minutes ?? undefined), `sitting:${sitting.id}`);
  };

  // Records a sitting's whole time by hand — without a timer, or to replace a capped one.
  const logMinutes = async (sitting: PlanAssignment) => {
    const minutes = await askMinutes(`Minutes on ${sitting.title}`, "", false);
    if (!minutes) return;
    await change(core.logMinutes(sitting.id, minutes), `sitting:${sitting.id}`);
  };

  const goToDay = async () => {
    const phrase = await ask("Go to Day", "Day", "A date, such as friday, or 12 October.");
    if (!phrase?.trim()) return;
    try {
      const day = await core.day(phrase);
      go(day.date);
    } catch (error) {
      say((error as Error).message);
    }
  };

  /** What can be done to a row, as its menu lists it. */
  const actions = (at: DayRow | undefined): Action[] => {
    if (!at) return [];
    const { block, sitting, free, cancelled } = at;
    if (block && at.kind === "block") {
      const list: Action[] = [];
      if (block.accepts_tasks) list.push({ id: "assign", label: "Assign a Task…", run: () => void assign(block) });
      list.push({ id: "edit", label: "Change…", run: () => void edit(block) });
      if (block.repeats) {
        list.push({
          id: "cancel-day",
          label: "Cancel This Day",
          run: () => void change(core.cancelOccurrence(block.series, today), at.key),
        });
      }
      if (block.changed_for_this_day) {
        list.push({
          id: "restore-day",
          label: "Restore This Day",
          run: () => void change(core.restoreOccurrence(block.series, today), at.key),
        });
      }
      list.push({ id: "delete", label: "Delete Block…", run: () => void remove(block) });
      return list;
    }
    if (sitting) {
      const start = { id: "timer", run: () => void change(core.startTimer(sitting.id), at.key) };
      const stop = { id: "stop", label: "Stop Timer", run: () => void change(core.stopTimer(sitting.id), at.key) };
      // Pause and Stop while it runs, Resume and Stop while paused, Start otherwise. Space is
      // the first of them: start, pause, resume.
      const timer: Action[] = sitting.running
        ? [{ id: "timer", label: "Pause Timer", run: () => void change(core.pauseTimer(sitting.id), at.key) }, stop]
        : sitting.status === "paused"
          ? [{ ...start, label: "Resume Timer" }, stop]
          : [{ ...start, label: "Start Timer" }];
      return [
        ...timer,
        { id: "open", label: "Edit Task Details", run: () => props.onOpenTask(sitting.task) },
        { id: "planned", label: "Planned Length…", run: () => void plannedLength(sitting) },
        { id: "log", label: "Log Minutes…", run: () => void logMinutes(sitting) },
        { id: "unassign", label: "Unassign", run: () => void change(core.unassign(sitting.id), undefined) },
      ];
    }
    if (free) return [{ id: "add-here", label: "Add Block Here…", run: () => void addBlock(free.start, free.minutes) }];
    if (cancelled) {
      return [
        {
          id: "restore-day",
          label: "Restore This Day",
          run: () => void change(core.restoreOccurrence(cancelled.series, today), undefined),
        },
      ];
    }
    return [];
  };

  // Enter: what a row is for.
  const activate = (key: Key) => {
    const at = row(String(key));
    if (!at) return;
    const id = { block: "edit", sitting: "open", free: "add-here", cancelled: "restore-day" }[at.kind as string];
    actions(at)
      .find((action) => action.id === id)
      ?.run();
  };

  const keysDown = (event: KeyboardEvent) => {
    const at = row(rowKey(event));
    if (!at) return;
    let action: string | undefined;
    if (asksForMenu(event)) {
      event.preventDefault();
      select(at.key);
      setMenu(true);
      return;
    }
    if (event.key === " " && at.kind === "sitting") action = "timer";
    else if (event.key === "Delete" && at.kind === "sitting") action = "unassign";
    else if (event.key === "Delete" && at.kind === "block") action = "delete";
    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    actions(at)
      .find((a) => a.id === action)
      ?.run();
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
      <div className="buttons" role="toolbar" aria-label="Day">
        <Button isDisabled={!shown} onPress={() => shown && go(step(shown.date, -1))}>
          Previous Day
        </Button>
        <Button onPress={() => go(undefined)}>Today</Button>
        <Button isDisabled={!shown} onPress={() => shown && go(step(shown.date, 1))}>
          Next Day
        </Button>
        <Button onPress={() => void goToDay()}>Go to Day…</Button>
        <Button onPress={() => void addBlock()}>Add Block…</Button>
        <RowMenu actions={actions(row(selected))} isOpen={menu} onOpenChange={setMenu} />
      </div>
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
