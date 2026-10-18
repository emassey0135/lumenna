/// <reference lib="webworker" />
// The core, in a dedicated worker.
//
// SQLite keeps the store in OPFS through sync access handles, which only a dedicated worker may
// use; and Automerge's merges and the sync loop should never stall typing. So the whole core
// runs here, and the page reaches it through Comlink. Each call is one call on the surface, or
// a few with the wording the desktop apps share applied to the result — nothing is decided here
// that the core does not decide.

import * as Comlink from "comlink";
import init, {
  App,
  Core,
  announcementText,
  blockDefaults,
  blockEdit,
  blockFields,
  dayBlockFields,
  exportChoices,
  newBlock,
  blockText,
  cancelledText,
  dayText,
  deviceText,
  freeText,
  nowText,
  sittingText,
  summaryText,
  candidateText,
  choiceText,
  placeQuickAddPrefix,
  taskForm,
  blockForm,
  pairingWords,
  sentenceCase,
  ownQuestions,
  placeQuery,
  placeTitle,
  rowText,
  taskEdit,
  taskFields,
  taskStateText,
  trashedText,
  keyboardShortcuts,
  proposalWords,
} from "./core/lumenna_web.js";
import type {
  Action,
  ActionKind,
  Answer,
  DayItem,
  DayView,
  Event,
  Item,
  ListView,
  Turn,
  ViewModel,
  DayPins,
  DayPlan,
  ProposalWords,
  Replan,
  ReplanPins,
  Choice,
  FormField,
  ShortcutGroup,
  PairingWords,
  Setting,
  BlockDefaults,
  BlockFields,
  BlockScope,
  ExportChoice,
  ExportFormat,
  CancelledBlock,
  Change,
  DueReminders,
  ReminderList,
  SidebarEntry,
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
let app: App | undefined;

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
  app = core.app();
}

function session(): App {
  if (!app) throw new Error("The store is not open yet.");
  return app;
}

/** A row of the list shown, with the line a screen reader reads for it. */
export interface ShownItem extends Item {
  text: string;
}

/** The list shown, its rows' lines in the web's words, and what it found as one sentence. */
export interface Shown extends Omit<ListView, "items"> {
  items: ShownItem[];
  readback: string;
}

/** A row of the day, with the line a screen reader reads for it. */
export interface ShownDayItem extends DayItem {
  text: string;
}

/** The day as the page draws it: its heading in the browser's words, each row's line. */
export interface ShownDay extends Omit<DayView, "items"> {
  title: string;
  items: ShownDayItem[];
}

/** The view model as the page draws it. */
export interface Screen extends Omit<ViewModel, "list" | "day"> {
  list?: Shown;
  day?: ShownDay;
}

/** A day's row as it reads. */
function dayLine(item: DayItem, date: string, summary: string): string {
  switch (item.kind) {
    case "summary":
      return summaryText(date, summary);
    case "block":
      return item.block ? blockText(item.block) : "";
    case "sitting":
      return item.sitting ? sittingText(item.sitting) : "";
    case "free":
      return item.free ? freeText(item.free.title, item.free.details, item.free.start, item.free.end) : "";
    case "now":
      return item.now ? nowText(item.now.title, item.now.time) : "";
    case "cancelled":
      return item.cancelled ? cancelledText(item.cancelled) : "";
  }
}

/** What an event left: the screen, and what to say once focus has landed. */
export interface Said {
  screen: Screen;
  said: string[];
}

/** The view model in the web's words: each row's line, the headings in sentence case. */
function screen(view: ViewModel): Screen {
  const trash = view.place === "Trash";
  const list = view.list && {
    ...view.list,
    items: view.list.items.map((item) => ({ ...item, text: trash ? trashedText(item.row) : rowText(item.row, false) })),
    readback: announcementText(
      [view.list.query && !trash ? view.list.query.description : undefined, view.list.announcement, ...(view.list.notices ?? [])]
        .filter((p): p is string => !!p)
        .join(". "),
      [],
    ),
  };
  const day = view.day && {
    ...view.day,
    title: dayText(view.day.date),
    items: view.day.items.map((item) => ({ ...item, text: dayLine(item, view.day!.date, view.day!.summary) })),
  };
  return {
    ...view,
    sidebar: view.sidebar.map((entry) => ("Group" in entry.kind ? { ...entry, text: sentenceCase(entry.text) } : entry)),
    list,
    day,
  };
}

function turned(turn: Turn): Said {
  const date = turn.view.day?.date;
  return {
    screen: screen(turn.view),
    said: turn.said.map((saying) => (saying.heading && date ? summaryText(date, saying.text) : saying.text)),
  };
}

/** A row and the line a screen reader reads for it. */
export interface Line {
  row: RowView;
  text: string;
}

const api = {
  /**
   * Opens the store — one tab at a time. OPFS gives the store one connection, and a device has
   * one sync endpoint, so the first tab holds a lock for as long as it is open and any
   * other is told where Lumenna is already open.
   */
  open(name: string): Promise<void> {
    // Asking twice is one opening: React's development mode runs effects twice, and a second
    // request for the lock would find this worker's own first request holding it.
    opening ??= opened(name);
    return opening;
  },

  /** Runs what the person did through the app: what to draw, and what to say. */
  send: (event: Event): Said => turned(session().send(event)),

  /** What changed elsewhere since, or a new minute: undefined when nothing did. */
  tick(): Said | undefined {
    const turn = session().tick();
    return turn ? turned(turn) : undefined;
  },

  /** What to draw now. */
  screen: (): Screen => screen(session().view()),

  /** What a pick offers, each as its line reads, for the chooser. */
  choiceLines: (choices: Choice[]) =>
    choices.map((choice) => ({ id: choice.id, text: choiceText(choice), depth: choice.depth, reason: choice.reason ?? undefined })),

  /** A number that moves when another tab or process has written. */
  outsideVersion: (): number => store().outsideVersion(),

  /** The places, as the sidebar lists them. */
  sidebar: (): SidebarEntry[] =>
    // A heading is fixed text, so in the web's sentence case; a place's line names it.
    store()
      .sidebar()
      .entries.map((entry) => ("Group" in entry.kind ? { ...entry, text: sentenceCase(entry.text) } : entry)),

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
    return { lines, readback: announcementText(parts.filter((p): p is string => !!p).join(". "), []), empty: listing.empty };
  },

  /** One task, as its details form starts from it. */
  task(id: string) {
    const task = store().showTask(id);
    return { task, fields: taskFields(task), state: taskStateText(task) };
  },

  /** The task form's fields, in order, in the core's words. */
  taskForm: (): FormField[] => taskForm().fields,

  /** The block form's fields, in order, in the core's words. */
  blockForm: (): FormField[] => blockForm().fields,

  /** Every sentence and button of pairing, buttons and questions in the web's sentence case. */
  pairingWords(): PairingWords {
    const words = pairingWords();
    for (const key of ["title", "wait", "join", "match_title", "match_yes", "match_no", "copy_code"] as const) {
      words[key] = sentenceCase(words[key]);
    }
    return { ...words, intro: words.intro_sentence };
  },

  /** The questions the web asks of its own accord: Go to Day, a new filter's steps, a length. */
  ownQuestions: () => ownQuestions(),

  /** Fixed text — a button, a question with no one's name in it — in sentence case. */
  sentence: (text: string): string => sentenceCase(text),

  /** The projects the details form's Project field offers, in tree order, each with its depth. */
  projectOptions: (): Choice[] => store().projectOptions().options,

  /** The keyboard commands the desktop apps share, grouped as their menus are, in sentence case. */
  keyboardShortcuts: (): ShortcutGroup[] => keyboardShortcuts().groups,

  /**
   * Saves a form's fields over the task it started from — only what changed, so a concurrent
   * edit on another device is not reverted. Undefined when nothing changed.
   */
  save(original: TaskDetail, fields: TaskFields): string | undefined {
    const edit = taskEdit(original, fields);
    return edit ? said(store().editTask(original.id, edit)) : undefined;
  },

  /**
   * Runs one of a record's actions with the answer to its question: what every menu, key and
   * button does. The core decides what it does, and refuses what it must.
   */
  act: (action: Action, answer: Answer) => done(store().act(action, answer)),

  /** What a pick offers for `action`, each as its line reads; when nothing, why. */
  choices(action: Action) {
    const found = store().choices(action);
    return {
      announcement: found.announcement,
      // Why each is offered, when the core says: its description, never its name.
      choices: found.choices.map((choice) => ({
        id: choice.id,
        text: choiceText(choice),
        depth: choice.depth,
        reason: choice.reason ?? undefined,
      })),
    };
  },

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
  // The day and its blocks
  // -------------------------------------------------------------------------------------

  /**
   * A day as it is lived, as rows: the summary first, then blocks in time order with their
   * sittings beneath them, free time and now as rows of their own, and the blocks cancelled
   * for the day last. Today when `date` is absent.
   */
  day(date?: string) {
    const plan = store().plan(date);
    const rows: DayRow[] = [{ key: "summary", text: summaryText(plan.date, plan.summary ?? ""), kind: "summary", actions: plan.actions ?? [], children: [] }];
    for (const item of plan.timeline ?? []) {
      if (item.item === "block") {
        const block = plan.blocks.find((b) => b.row === item.row);
        if (!block) continue;
        rows.push({
          key: `block:${block.id}`,
          text: blockText(block),
          kind: "block",
          block,
          actions: block.actions ?? [],
          children: block.assignments.map((sitting) => ({
            key: `sitting:${sitting.id}`,
            text: sittingText(sitting),
            kind: "sitting",
            sitting,
            block,
            actions: sitting.actions ?? [],
            children: [],
          })),
        });
      } else if (item.item === "free") {
        const free = { start: item.start, end: item.end, minutes: item.minutes };
        rows.push({ key: `free:${item.start}`, text: freeText(item.title ?? "", item.details ?? [], item.start, item.end), kind: "free", free, actions: item.actions ?? [], children: [] });
      } else {
        rows.push({ key: "now", text: nowText(item.title ?? "", item.time), kind: "now", actions: [], children: [] });
      }
    }
    for (const cancelled of plan.cancelled ?? []) {
      rows.push({ key: `cancelled:${cancelled.series}`, text: cancelledText(cancelled), kind: "cancelled", cancelled, actions: cancelled.actions ?? [], children: [] });
    }
    // The day's own actions (Plan the Day, Replan from Now, …) are its summary row's, and the
    // Day actions menu's.
    return { date: plan.date, title: dayText(plan.date), rows, actions: plan.actions ?? [] };
  },

  /** An ISO date as a person says it, and `HH:MM` as this browser says it. */
  dayText: (iso: string): string => dayText(iso),

  /** Every block series, each with its line; and how many, as a sentence. */
  blocks() {
    const listing = store().listBlocks();
    const lines: Line[] = listing.rows.map((row) => ({ row, text: rowText(row, false) }));
    return { lines, readback: announcementText(listing.announcement, listing.notices ?? []), empty: listing.empty };
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
  /** When a new block on `date` (a phrase; today when absent) starts, `HH:MM`, unless said otherwise. */
  newBlockStart: (date?: string): string => store().newBlockStart(date),

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

  // -------------------------------------------------------------------------------------
  // The planner: Replan from Now and Plan the Day, each a proposal said in full and made as
  // the person checks it, in this browser's clock.
  // -------------------------------------------------------------------------------------

  /** Every word of a proposal's screen, the buttons and headings in the web's sentence case. */
  proposalWords(kind: ActionKind): ProposalWords {
    const words = proposalWords(kind);
    for (const key of ["title", "item", "apply", "cancel", "unfit"] as const) words[key] = sentenceCase(words[key]);
    if (words.others) words.others = sentenceCase(words.others);
    return words;
  },

  /** What replanning the rest of today proposes, keeping to `pins` (none at first). */
  replan: (pins?: ReplanPins): Proposed => replanned(store().replan(pins)),

  /** Makes the changes checked (by index), and what is proposed next. */
  applyReplan(proposal: Proposed, accepted: number[]): Made {
    const record = proposal.record as Replan;
    const made = store().applyReplan(record, accepted.map((index) => record.changes[index]).filter((c) => c !== undefined));
    return { said: announcementText(made.announcement, made.notices ?? []), next: replanned(made.next) };
  },

  /** What planning `date` proposes, keeping to `pins` (none at first). */
  proposeDayPlan: (date: string, pins?: DayPins): Proposed => planned(store().proposeDayPlan(date, pins)),

  /** Makes the sittings checked (by index), and what is proposed next. */
  applyDayPlan(proposal: Proposed, accepted: number[]): Made {
    const record = proposal.record as DayPlan;
    const made = store().applyDayPlan(record, accepted.map((index) => record.sittings[index]).filter((s) => s !== undefined));
    return { said: announcementText(made.announcement, made.notices ?? []), next: planned(made.next) };
  },

  /** A reminder's Carry Forward: the rest of a block's short work (`<series>@<date>`) carried on. */
  carryRestForward: (block: string): string => said(store().carryRestForward(block)),

  // -------------------------------------------------------------------------------------
  // Devices and sync. A browser reaches other devices through a relay, and pairs by code.
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
  devices(): { list: { id: string; name: string; thisDevice: boolean; text: string; actions: Action[] }[]; empty: string } {
    const found = store().devices();
    const list = found.devices.map((device) => ({
        id: device.node_id,
        name: device.name,
        thisDevice: device.this_device,
        text: deviceText(device),
        actions: device.actions ?? [],
      }));
    return { list, empty: found.empty ?? "" };
  },

  /** How syncing is going, as one sentence. */
  syncStatus: (): string => {
    const status = store().syncStatus();
    return announcementText(status.announcement, status.notices ?? []);
  },

  // -------------------------------------------------------------------------------------
  // Reminders. A browser has no lock to take: the one tab that owns the store gives them,
  // keeping its own "since".
  // -------------------------------------------------------------------------------------

  /** What came due after `since` up to `now` (RFC 3339): one by one, and what was missed. */
  remindersDue: (since: string, now: string): DueReminders => store().remindersDue(since, now),

  /** A task's or block's reminders: its own, or the defaults it takes. */
  remindersOf: (target: string): ReminderList => store().remindersOf(target),

  /** A notification's Dismiss, on every device; what to say. */
  dismissReminder: (key: string, occurrence: string): string => said(store().dismissReminder(key, occurrence)),

  /** A notification's Snooze, for the core's ten minutes; what to say. */
  snoozeReminder: (key: string, occurrence: string): string => said(store().snoozeReminder(key, occurrence)),

  /** A saved filter, from the form its heading's New opens. */
  addFilter: (name: string, query: string) => done(store().addFilter(name, query)),

  // -------------------------------------------------------------------------------------
  // Settings, backups and exports.
  // -------------------------------------------------------------------------------------

  /** Every setting, each with its name, its control and what it can be. */
  settings: (): Setting[] => store().settings(undefined).settings,

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
  /** What can be done to it, as the core offers it. */
  actions: Action[];
  children: DayRow[];
}

/**
 * A proposal as its screen shows it, Replan from Now's or Plan the Day's alike: what to say on
 * showing it, a line (and why) for each change, what does not fit; and the core's own record,
 * handed back to apply it.
 */
export interface Proposed {
  announcement: string;
  items: { sentence: string; reason?: string }[];
  unfit: string[];
  /** Other ways to replan (Replan from Now's alone): each its sentence, and the pins it asks with. */
  alternatives: { sentence: string; pins: ReplanPins }[];
  pins: ReplanPins | DayPins;
  record: Replan | DayPlan;
}

/** What applying a proposal made, as said, and the proposal for the day as it is now. */
export interface Made {
  said: string;
  next: Proposed;
}

function replanned(record: Replan): Proposed {
  return {
    announcement: announcementText(record.announcement, record.notices ?? []),
    items: record.changes.map((change) => ({ sentence: change.sentence })),
    unfit: record.unplaceable ?? [],
    alternatives: record.alternatives ?? [],
    pins: record.pins,
    record,
  };
}

function planned(record: DayPlan): Proposed {
  return {
    announcement: announcementText(record.announcement, record.notices ?? []),
    items: record.sittings.map((sitting) => ({ sentence: sitting.sentence, reason: sitting.reason ?? undefined })),
    unfit: record.unfit ?? [],
    alternatives: [],
    pins: record.pins,
    record,
  };
}

export type Api = typeof api;

Comlink.expose(api);
