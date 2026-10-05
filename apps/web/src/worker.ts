/// <reference lib="webworker" />
// The core, in a dedicated worker (§16.12).
//
// SQLite keeps the store in OPFS through sync access handles, which only a dedicated worker may
// use; and Automerge's merges and the sync loop should never stall typing. So the whole core
// runs here, and the page reaches it through Comlink. Each call is one call on the surface, or
// a few with the wording the desktop apps share applied to the result — nothing is decided here
// that the core does not decide.

import * as Comlink from "comlink";
import init, {
  Core,
  announcementText,
  blockDefaults,
  blockEdit,
  blockFields,
  dayBlockFields,
  exportChoices,
  newBlock,
  parseWeight,
  blockText,
  cancelledText,
  dayText,
  deviceText,
  freeText,
  nowText,
  sittingText,
  summaryText,
  candidateText,
  placeQuickAddPrefix,
  placeQuery,
  placeTitle,
  rowText,
  taskEdit,
  taskFields,
  taskStateText,
  trashedText,
} from "./core/lumenna_web.js";
import type {
  BlockChoice,
  BlockDefaults,
  BlockFields,
  BlockScope,
  Direction,
  ExportChoice,
  ExportFormat,
  CancelledBlock,
  Change,
  Entry,
  PairedWith,
  Place,
  PlanAssignment,
  PlanBlock,
  RowView,
  Syntax,
  SyncReport,
  TaskDetail,
  TaskFields,
} from "./core/lumenna_web.js";

let core: Core | undefined;

function store(): Core {
  if (!core) throw new Error("The store is not open yet.");
  return core;
}

/** A change, as said: its announcement and notices as one sentence. */
function said(change: Change): string {
  return announcementText(change.announcement, change.notices ?? []);
}

/** A UTF-16 offset — what an input element counts — as the UTF-8 byte offset the core counts. */
function bytesAt(text: string, units: number): number {
  return new TextEncoder().encode(text.slice(0, units)).length;
}

/** A UTF-8 byte offset from the core as a UTF-16 offset, never splitting a character. */
function unitsAt(text: string, bytes: number): number {
  const encoded = new TextEncoder().encode(text);
  let end = Math.min(bytes, encoded.length);
  // Back to the start of a character: continuation bytes are 10xxxxxx.
  while (end > 0 && end < encoded.length && (encoded[end] & 0xc0) === 0x80) end--;
  return new TextDecoder().decode(encoded.slice(0, end)).length;
}

let opening: Promise<void> | undefined;

async function opened(name: string): Promise<void> {
  const granted = await new Promise<boolean>((resolve) => {
    void navigator.locks.request(`lumenna:${name}`, { ifAvailable: true }, (lock) => {
      resolve(lock !== null);
      // Held while this worker lives.
      return lock ? new Promise<void>(() => {}) : undefined;
    });
  });
  if (!granted) throw new Error("Lumenna is open in another tab. Close it there to use it here.");
  await init();
  core = await Core.open(name);
}

/** A row and the line a screen reader reads for it. */
export interface Line {
  row: RowView;
  text: string;
}

const api = {
  /**
   * Opens the store — one tab at a time. OPFS gives the store one connection, and a device has
   * one sync endpoint (§8), so the first tab holds a lock for as long as it is open and any
   * other is told where Lumenna is already open.
   */
  open(name: string): Promise<void> {
    // Asking twice is one opening: React's development mode runs effects twice, and a second
    // request for the lock would find this worker's own first request holding it.
    opening ??= opened(name);
    return opening;
  },

  /** A number that moves when another tab or process has written. */
  outsideVersion: (): number => store().outsideVersion(),

  /** The places, as the sidebar lists them. */
  sidebar: (): Entry[] => store().sidebar().entries,

  place: (place: Place) => ({
    title: placeTitle(place),
    query: placeQuery(place),
    quickAddPrefix: placeQuickAddPrefix(place),
  }),

  /** Tasks matching a query, each with its line; and what was found, as a sentence. */
  tasks(query: string, trash: boolean) {
    const listing = store().listTasks(query);
    const lines: Line[] = listing.rows.map((row) => ({ row, text: trash ? trashedText(row) : rowText(row, false) }));
    const parts = [listing.query && !trash ? listing.query.description : undefined, listing.announcement, ...(listing.notices ?? [])];
    return { lines, readback: announcementText(parts.filter((p): p is string => !!p).join(". "), []) };
  },

  /** One task, as its details form starts from it. */
  task(id: string) {
    const task = store().showTask(id);
    return { task, fields: taskFields(task), state: taskStateText(task) };
  },

  /** The projects, by name, for the details form's choice. */
  projects: (): string[] => store().listProjects().rows.map((row) => row.title),

  /**
   * Saves a form's fields over the task it started from — only what changed, so a concurrent
   * edit on another device is not reverted. Undefined when nothing changed.
   */
  save(original: TaskDetail, fields: TaskFields): string | undefined {
    const edit = taskEdit(original, fields);
    return edit ? said(store().editTask(original.id, edit)) : undefined;
  },

  complete: (id: string, done: boolean): string => said(done ? store().uncompleteTask(id) : store().completeTask(id)),
  trash: (id: string): string => said(store().trashTask(id)),
  restore: (id: string): string => said(store().restoreTask(id)),
  erase: (id: string): string => said(store().eraseTask(id)),
  undo: (): string => said(store().undo()),
  redo: (): string => said(store().redo()),

  /** Adds a task from a quick-add line, returning what was said and the task's identifier. */
  add(text: string): { said: string; id?: string } {
    const change = store().addTask(text);
    return { said: said(change), id: change.task?.id };
  },

  /** What a quick-add line would add: the readback, with every diagnostic. */
  preview(text: string): { readback: string; hasErrors: boolean } {
    if (!text.trim()) return { readback: "", hasErrors: true };
    const preview = store().previewTask(text);
    const parts = [preview.announcement, ...preview.diagnostics.map((d) => d.message)];
    return { readback: parts.join(". "), hasErrors: preview.has_errors };
  },

  /**
   * What fits at the cursor (a UTF-16 offset, as an input counts), with the span it replaces in
   * UTF-16 offsets and each candidate's text and line.
   */
  completions(text: string, cursor: number, syntax: Syntax) {
    const found = store().completeText(text, bytesAt(text, cursor), syntax);
    return {
      announcement: found.announcement,
      start: unitsAt(text, found.start),
      end: unitsAt(text, found.end),
      candidates: found.candidates.map((candidate) => ({ text: candidate.text, label: candidateText(candidate) })),
    };
  },

  // -------------------------------------------------------------------------------------
  // The day and its blocks (§3.7, §13)
  // -------------------------------------------------------------------------------------

  /**
   * A day as it is lived, as rows: the summary first, then blocks in time order with their
   * sittings beneath them, free time and now as rows of their own, and the blocks cancelled
   * for the day last. Today when `date` is absent.
   */
  day(date?: string) {
    const plan = store().plan(date);
    const rows: DayRow[] = [{ key: "summary", text: summaryText(plan.date, plan.summary ?? ""), kind: "summary", children: [] }];
    for (const item of plan.timeline ?? []) {
      if (item.item === "block") {
        const block = plan.blocks.find((b) => b.row === item.row);
        if (!block) continue;
        rows.push({
          key: `block:${block.id}`,
          text: blockText(block),
          kind: "block",
          block,
          children: block.assignments.map((sitting) => ({
            key: `sitting:${sitting.id}`,
            text: sittingText(sitting),
            kind: "sitting",
            sitting,
            block,
            children: [],
          })),
        });
      } else if (item.item === "free") {
        const free = { start: item.start, end: item.end, minutes: item.minutes };
        rows.push({ key: `free:${item.start}`, text: freeText(item.start, item.end, item.minutes), kind: "free", free, children: [] });
      } else {
        rows.push({ key: "now", text: nowText(item.time), kind: "now", children: [] });
      }
    }
    for (const cancelled of plan.cancelled ?? []) {
      rows.push({ key: `cancelled:${cancelled.series}`, text: cancelledText(cancelled), kind: "cancelled", cancelled, children: [] });
    }
    return { date: plan.date, title: dayText(plan.date), rows };
  },

  /** An ISO date as a person says it, and `HH:MM` as this browser says it. */
  dayText: (iso: string): string => dayText(iso),

  /** Every block series, each with its line; and how many, as a sentence. */
  blocks() {
    const listing = store().listBlocks();
    const lines: Line[] = listing.rows.map((row) => ({ row, text: rowText(row, false) }));
    return { lines, readback: announcementText(listing.announcement, listing.notices ?? []) };
  },

  /**
   * A series' fields as the form for every occurrence starts from them, with the rule it
   * repeats by when the repetition words cannot say it.
   */
  seriesFields(id: string): { fields: BlockFields; rule?: string } {
    const shown = store().showBlock(id);
    return { fields: blockFields(shown), rule: shown.repeats ? (shown.rrule ?? undefined) : undefined };
  },

  /** One day's block as the form for that day alone starts from it. */
  dayFields: (block: PlanBlock): BlockFields => dayBlockFields(block),

  /** What a kind of block has unless set apart. */
  kindDefaults: (kind: string): BlockDefaults | undefined => blockDefaults(kind) ?? undefined,

  /** Adds a block from its form; what was said, and the series it made. */
  addBlock(fields: BlockFields, date: string): { said: string; series?: string } {
    const change = store().addBlock(newBlock(fields, date));
    return { said: said(change), series: change.affected?.blocks?.[0] };
  },

  /**
   * Saves a block's form over what it started from — only what changed — to every occurrence,
   * or to one day's when `date` is given. Undefined when nothing changed.
   */
  saveBlock(series: string, date: string | undefined, before: BlockFields, after: BlockFields): string | undefined {
    const edit = blockEdit(before, after);
    if (!edit) return undefined;
    const scope: BlockScope = date ? { Occurrence: { date } } : "Series";
    return said(store().editBlock(series, edit, scope));
  },

  cancelOccurrence: (series: string, date: string): string => said(store().cancelOccurrence(series, date)),
  restoreOccurrence: (series: string, date: string): string => said(store().restoreOccurrence(series, date)),
  deleteBlock: (id: string): string => said(store().deleteBlock(id)),

  assign: (task: string, block: string, date: string | undefined, minutes: number | undefined): string =>
    said(store().assign(task, block, date, minutes)),
  unassign: (sitting: string): string => said(store().unassign(sitting)),
  planMinutes: (sitting: string, minutes: number | undefined): string => said(store().planMinutes(sitting, minutes)),

  /** Starts a sitting's timer, or resumes it when paused. */
  startTimer: (sitting: string): string => said(store().startTimer(sitting)),

  /** Pauses a sitting's timer, keeping the time so far. */
  pauseTimer(sitting: string): string {
    const timer = store().pauseTimer(sitting);
    return announcementText(timer.announcement, timer.notices ?? []);
  },

  /** Stops a sitting's timer, which ends the sitting. */
  stopTimer(sitting: string): string {
    const timer = store().stopTimer(sitting, undefined);
    return announcementText(timer.announcement, timer.notices ?? []);
  },

  /** Records a sitting's whole time by hand, replacing what is logged. */
  logMinutes(sitting: string, minutes: number): string {
    const timer = store().stopTimer(sitting, minutes);
    return announcementText(timer.announcement, timer.notices ?? []);
  },

  /** The week's work blocks a task could go in, each as it reads in a chooser. */
  workBlocks: (): BlockChoice[] => store().workBlocks().blocks,

  /** Every open task, as a chooser lists them. */
  taskChoices: (): { id: string; text: string }[] =>
    store().listTasks("").rows.map((row) => ({ id: row.id, text: rowText(row, false) })),

  // -------------------------------------------------------------------------------------
  // Devices and sync (§7). A browser reaches other devices through a relay, and pairs by code.
  // -------------------------------------------------------------------------------------

  /**
   * Pairs with another device: waits with a code of its own, given to `showCode`, or dials
   * `code`. `confirm` is asked whether the words match. Resolves to what to say.
   *
   * The page's callbacks arrive as Comlink proxies, which answer any property — `.call`
   * included — so the core is handed plain functions that call them.
   */
  async pair(
    code: string | undefined,
    name: string,
    showCode: (code: string) => void,
    confirm: (words: string[]) => Promise<boolean>,
  ): Promise<string> {
    const paired: PairedWith = await store().pair(
      code,
      name,
      (shown: string) => void showCode(shown),
      (words: string[]) => confirm(words),
    );
    return announcementText(paired.announcement, paired.notices ?? []);
  },

  cancelPairing: () => store().cancelPairing(),

  /** Keeps this browser in sync while it is open; `changed` hears what arrives. */
  async startSync(changed: () => void): Promise<void> {
    await store().startSync(() => void changed());
  },

  /** Syncs now; what to say about it, each device that could not be reached included. */
  async syncNow(): Promise<string> {
    const report: SyncReport = await store().syncNow();
    const failures = report.peers.filter((peer) => peer.error).map((peer) => `${peer.name}: ${peer.error}`);
    return announcementText(report.announcement, failures);
  },

  stopSync: () => store().stopSync(),
  syncRunning: (): boolean => store().syncRunning(),

  /** The paired devices, this one first, each with its line. */
  devices(): { id: string; name: string; thisDevice: boolean; text: string }[] {
    return store()
      .devices()
      .devices.map((device) => ({
        id: device.node_id,
        name: device.name,
        thisDevice: device.this_device,
        text: deviceText(device),
      }));
  },

  renameDevice: (id: string, name: string): string => said(store().renameDevice(id, name)),
  unpairDevice: (id: string): string => said(store().unpairDevice(id)),

  /** How syncing is going, as one sentence. */
  syncStatus: (): string => {
    const status = store().syncStatus();
    return announcementText(status.announcement, status.notices ?? []);
  },

  // -------------------------------------------------------------------------------------
  // Projects, labels and saved filters. Each says what it did, and whether anything changed.
  // -------------------------------------------------------------------------------------

  addProject: (name: string, parent?: string) => done(store().addProject(name, parent)),
  renameProject: (name: string, to: string) => done(store().renameProject(name, to)),
  moveProject: (name: string, parent?: string) => done(store().moveProject(name, parent)),
  reorderProject: (name: string, direction: Direction) => done(store().reorderProject(name, direction)),
  weighProject: (name: string, weight: string) => done(store().weighProject(name, parseWeight(weight))),

  /** What is wrong with a weight as typed, in the surface's words, or nothing. */
  weightProblem(text: string): string | undefined {
    try {
      parseWeight(text);
      return undefined;
    } catch (error) {
      return (error as Error).message;
    }
  },
  archiveProject: (name: string) => done(store().archiveProject(name)),
  deleteProject: (name: string, keepTasks: boolean) => done(store().deleteProject(name, keepTasks)),
  addLabel: (name: string) => done(store().addLabel(name)),
  renameLabel: (name: string, to: string) => done(store().renameLabel(name, to)),
  mergeLabels: (from: string, into: string) => done(store().mergeLabels(from, into)),
  recolourLabel: (name: string, colour?: string) => done(store().recolourLabel(name, colour)),
  reorderLabel: (name: string, direction: Direction) => done(store().reorderLabel(name, direction)),
  deleteLabel: (name: string) => done(store().deleteLabel(name)),
  addFilter: (name: string, query: string) => done(store().addFilter(name, query)),
  editFilter: (name: string, rename?: string, query?: string) => done(store().editFilter(name, rename, query)),
  reorderFilter: (name: string, direction: Direction) => done(store().reorderFilter(name, direction)),
  deleteFilter: (name: string) => done(store().deleteFilter(name)),

  /** The labels, by name, for choosing one to merge into. */
  labels: (): string[] => store().listLabels().rows.map((row) => row.title),

  // Waiting for other tasks.
  waitFor: (id: string, on: string): string => said(store().addDependency(id, on)),
  stopWaiting: (id: string, on: string): string => said(store().removeDependency(id, on)),

  // -------------------------------------------------------------------------------------
  // Settings, backups and exports.
  // -------------------------------------------------------------------------------------

  /** Every setting, by key. */
  settings(): Record<string, string> {
    return Object.fromEntries(store().settings(undefined).settings.map((setting) => [setting.key, setting.value]));
  },

  setSetting: (key: string, value: string): string => said(store().setSetting(key, value)),

  /** A backup taken now, for the page to download, and what to say about it. */
  backup(): { name: string; bytes: Uint8Array; said: string } {
    const file = store().backupFile() as { name: string; bytes: Uint8Array; said: string };
    return Comlink.transfer(file, [file.bytes.buffer as ArrayBuffer]);
  },

  /** Reads a JSON export or a backup the person chose; what it did. */
  importFile(name: string, bytes: Uint8Array): string {
    const imported = store().importBytes(name, bytes);
    const result = "Backup" in imported ? imported.Backup.done : imported.Export.done;
    return announcementText(result.announcement, result.notices ?? []);
  },

  /** The exports Settings offers. */
  exportChoices: (): ExportChoice[] => exportChoices().exports,

  /** An export's contents, to download. */
  exportContent: (format: ExportFormat): string => store().export(format).content ?? "",
};

/** What a change said, and whether it changed anything — which decides whether to go to it. */
function done(change: Change): { said: string; changed: boolean } {
  return { said: said(change), changed: change.changed };
}

/** One row of the day, with what it is and the rows beneath it. */
export interface DayRow {
  key: string;
  text: string;
  kind: "summary" | "block" | "sitting" | "free" | "now" | "cancelled";
  block?: PlanBlock;
  sitting?: PlanAssignment;
  free?: { start: string; end: string; minutes: number };
  cancelled?: CancelledBlock;
  children: DayRow[];
}

export type Api = typeof api;

Comlink.expose(api);
