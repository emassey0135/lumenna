/// <reference lib="webworker" />
// The core, in a dedicated worker.
//
// SQLite keeps the store in OPFS through sync access handles, which only a dedicated worker may
// use; and Automerge's merges and the sync loop should never stall typing. So the whole core
// runs here — the store, and the app (crates/app) beside it — and the page reaches it through
// Comlink: it sends what the person did and draws the screen that comes back, worded here with
// what the desktop apps share (each row's line, the day's words, sentence case). Nothing is
// decided here.

import * as Comlink from "comlink";
import init, { open, pickerWords, sentenceCase, keyboardShortcuts, folderBackupWords } from "./core/lumenna_web.js";
import type {
  App,
  Asking,
  Swatch,
  DayItem,
  DayView,
  DeviceList,
  DeviceView,
  DevicesView,
  Notice,
  SaveFile,
  DetailsView,
  ShellEvent,
  Item,
  Lines,
  ListView,
  ReminderList,
  ReminderView,
  Turn,
  ViewModel,
  KeyGroup,
  Syntax,
  PickerWords,
} from "./core/lumenna_web.js";
import { Capitals, TimeStyle } from "./core/lumenna_web.js";

let app: App | undefined;

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
  // The module, and its wasm beside it, which Vite serves as an asset.
  await init(await fetch(new URL("./core/lumenna_web_bg.wasm", import.meta.url)));
  app = await open(name);
  // The clock the core's whole sentences say times in: this browser's.
  const hour12 = new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hour12;
  void app.send({ tag: "Clock", value0: hour12 ? TimeStyle.TwelveHour : TimeStyle.TwentyFourHour });
  // The web's capitals: buttons, headings and questions in sentence case.
  void app.send({ tag: "Capitals", value0: Capitals.Sentence });
  // Pairing's sentences call a browser "this browser"; with no network of its own to search,
  // it pairs by code.
  void app.send({ tag: "PairingAs", called: "this browser", nearby: false });
  // Open once the app has the store.
  await app.settled();
}

function session(): App {
  if (!app) throw new Error("The store is not open yet.");
  return app;
}

/** A row of the list shown, with the line a screen reader reads for it. */
export interface ShownItem extends Item {
  text: string;
  /** The line on a braille display, its states short (`Line::braille`). */
  braille: string;
}

/** The list shown, its rows' lines in the web's words, and what it found as one sentence. */
export interface Shown extends Omit<ListView, "items"> {
  items: ShownItem[];
  readback: string;
}

/** A row of the day, with the line a screen reader reads for it. */
export interface ShownDayItem extends DayItem {
  text: string;
  /** The line on a braille display: lengths as hours and minutes, kinds short. */
  braille: string;
}

/** The day as the page draws it: its heading in the browser's words, each row's line. */
export interface ShownDay extends Omit<DayView, "items"> {
  title: string;
  items: ShownDayItem[];
}

/** The details as the page draws them: the task's state as a line. */
export interface ShownDetails extends DetailsView {
  state: string;
}

/** The quick-add line as the page draws it: offsets in UTF-16 units, as an input counts. */
export interface ShownLine {
  text: string;
  cursor: number;
  readback: string;
  hasErrors: boolean;
  offer: { start: number; end: number; candidates: { id: string; text: string; label: string }[] };
}

/** A paired device with the line it reads as. */
export type ShownDevice = DeviceView & { text: string };

/** Something to pick, as its line reads. */
export interface ShownChoice {
  id: string;
  text: string;
  depth?: number;
  /** Why it is offered, when the core says: its description, never its name. */
  reason?: string;
  /** A colour's, drawn beside its name. */
  swatch?: Swatch;
}

/** What a field written in the filter or quick-add language offers at its cursor. */
export interface Offered {
  announcement: string;
  start: number;
  end: number;
  candidates: { id: string; text: string; label: string }[];
}

/** The question open, a pick's choices as lines. */
export interface ShownAsking extends Asking {
  lines: ShownChoice[] | null;
}

/** A task's or block's reminders, each with the line it reads as. */
export type ShownReminders = Omit<ReminderList, "reminders"> & { reminders: (ReminderView & { text: string })[] };

/** The view model as the page draws it: as the core's, each list's rows with their lines. */
export interface Screen extends Omit<ViewModel, "list" | "day" | "details" | "line" | "devices" | "asking" | "reminders"> {
  reminders: ShownReminders | null;
  asking: ShownAsking | null;
  devices: (Omit<DevicesView, "list"> & { list: (Omit<DeviceList, "devices"> & { devices: ShownDevice[] }) | null }) | null;
  list: Shown | null;
  day: ShownDay | null;
  details: ShownDetails | null;
  line: ShownLine | null;
}

/** What an event left: the screen, what to say once focus has landed, files to save, and
 * notifications to show. */
export interface Said {
  screen: Screen;
  said: string[];
  files: SaveFile[];
  notices: Notice[];
  /** Counted: an older screen is never drawn over a newer one. */
  seq: number;
}

/** The view model as the page draws it, each line as the core words it in this browser's clock. */
function screen(view: ViewModel, lines: Lines): Screen {
  const list = view.list && {
    ...view.list,
    items: view.list.items.map((item, index) => ({
      ...item,
      text: lines.list[index]?.text ?? item.row.title,
      braille: lines.list[index]?.braille ?? "",
    })),
    readback: lines.readback,
  };
  const day = view.day && {
    ...view.day,
    title: lines.dayTitle,
    items: view.day.items.map((item, index) => ({ ...item, text: lines.day[index]?.text ?? "", braille: lines.day[index]?.braille ?? "" })),
  };
  const details = view.details && { ...view.details, state: lines.taskState };
  const asking = view.asking && {
    ...view.asking,
    lines: view.asking.choices && view.asking.choices.choices.map((choice, index) => ({
      id: choice.id,
      text: lines.choices[index]?.text ?? choice.title,
      depth: choice.depth,
      reason: choice.reason ?? undefined,
      swatch: choice.swatch ?? undefined,
    })),
  };
  const devices = view.devices && {
    ...view.devices,
    list: view.devices.list && {
      ...view.devices.list,
      devices: view.devices.list.devices.map((device, index) => ({ ...device, text: lines.devices[index]?.text ?? device.name })),
    },
  };
  const line = view.line && {
    text: view.line.text,
    cursor: unitsAt(view.line.text, view.line.cursor),
    readback: view.line.readback,
    hasErrors: view.line.hasErrors,
    offer: {
      start: unitsAt(view.line.text, view.line.offer.start),
      end: unitsAt(view.line.text, view.line.offer.end),
      candidates: view.line.offer.candidates.map((candidate, index) => ({
        id: String(index),
        text: candidate.text,
        label: lines.candidates[index] ?? candidate.label,
      })),
    },
  };
  const reminders = view.reminders && {
    ...view.reminders,
    reminders: view.reminders.reminders.map((reminder, index) => ({ ...reminder, text: lines.reminders[index]?.text ?? reminder.words })),
  };
  return { ...view, list, day, details, line, devices, asking, reminders };
}

function turned(turn: Turn): Said {
  return {
    screen: screen(turn.view, turn.lines),
    said: turn.said,
    files: turn.files,
    notices: turn.notices,
    seq: turn.seq,
  };
}

const api = {
  /**
   * Opens the store — one tab at a time. OPFS gives the store one connection, and a device has
   * one sync endpoint, so the first tab holds a lock for as long as it is open and any other is
   * told where Lumenna is already open.
   */
  open(name: string): Promise<void> {
    // Asking twice is one opening: React's development mode runs effects twice, and a second
    // request for the lock would find this worker's own first request holding it.
    opening ??= opened(name);
    return opening;
  },

  /**
   * Runs what the person did through the app. Every turn it leaves — what to draw, what to say —
   * goes to the listener; the answer is the screen once the app is still, with nothing to say.
   */
  send: async (event: ShellEvent): Promise<Said> => turned(await session().send(event)),

  /** The quick-add line as typed, the cursor a UTF-16 offset as the input counts it. */
  typed: async (text: string, cursor: number): Promise<Said> =>
    turned(await session().send({ tag: "Typing", value0: { tag: "Text", text, cursor: bytesAt(text, cursor) } })),

  /**
   * Hands `listener` every turn the app leaves — for what the person did, a pairing's code and
   * words, a round's report, a sync's arrivals — one at a time, in order. A plain function that
   * calls the page's: a Comlink proxy answers any property, `.call` included.
   */
  listen(listener: (said: Said) => void): void {
    session().listen({ turn: (turn: Turn) => void listener(turned(turn)) });
  },

  /** Takes in what changed elsewhere since, or a new minute; what it leaves goes to the listener. */
  async tick(): Promise<void> {
    await session().tick();
  },

  /** What to draw now. */
  screen: (): Screen => turned(session().now()).screen,


  /** A number that moves when another tab or process has written. */
  outsideVersion: (): number => session().outsideVersion(),

  /** What could go at `cursor` (UTF-16 units, as an input counts) in `text`, written in
   * `syntax`: its span in UTF-16 too, and each candidate's line. */
  complete: (text: string, cursor: number, syntax: Syntax): Offered => {
    const found = session().complete(text, bytesAt(text, cursor), syntax);
    return {
      announcement: found.announcement,
      start: unitsAt(text, found.start),
      end: unitsAt(text, found.end),
      candidates: found.candidates.map((candidate, index) => ({ id: String(index), text: candidate.text, label: candidate.label })),
    };
  },

  /** Fixed text — a button, a question with no one's name in it — in sentence case. */
  sentence: (text: string): string => sentenceCase(text),

  /** What the web's own day and time pickers say, in sentence case. */
  pickerWords: (): PickerWords => pickerWords(),

  /** The keyboard commands the desktop apps share, grouped as their menus are, in sentence case. */
  keyboardShortcuts: (): KeyGroup[] => keyboardShortcuts(),

  /** A backup taken now, for the folder the person chose: its name there and its bytes. */
  backupFile: (): { name: string; bytes: Uint8Array } => {
    const file = session().backupFile();
    // A copy of its own: the core's bytes may be a view of the WebAssembly memory.
    const bytes = new Uint8Array(file.bytes);
    return Comlink.transfer({ name: file.name, bytes }, [bytes.buffer]);
  },

  /** Whether a backup is due in a folder holding the files `names`: the core's rule. */
  backupDue: (names: string[], now: number): boolean => session().backupDue(names, now),

  /** Which of the files `names` in the backup folder to remove: the oldest beyond those kept. */
  backupsToRemove: (names: string[]): string[] => session().backupsToRemove(names),

  /** Every backup among the files `names` in the backup folder: what goes after an erasure. */
  folderBackups: (names: string[]): string[] => session().folderBackups(names),

  /** Which generation of the store's history this browser holds: a real erasure begins another. */
  generation: (): string => session().generation(),

  /** What automatic backups to a folder do, in a sentence. */
  folderBackupWords: (): string => folderBackupWords(),
};

export type Api = typeof api;

// The core's errors carry their sentence in `value.reason`, and Comlink sends only an error's
// name, message and stack to the page: unhandled, every failure the page heard of said only
// "LumennaError". The sentence becomes the message on the way.
const throwing = Comlink.transferHandlers.get("throw") as Comlink.TransferHandler<unknown, unknown>;
Comlink.transferHandlers.set("throw", {
  canHandle: (value): value is unknown => throwing.canHandle(value),
  serialize: (thrown) => {
    const { value } = thrown as { value: unknown };
    const reason = (value as { value?: { reason?: unknown } } | null)?.value?.reason;
    return throwing.serialize(typeof reason === "string" ? { value: Object.assign(new Error(reason), { name: "LumennaError" }) } : thrown);
  },
  deserialize: (serialized) => throwing.deserialize(serialized),
});

Comlink.expose(api);
