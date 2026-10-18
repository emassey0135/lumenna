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
import init, {
  App,
  Core,
  announcementText,
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
  pairingWords,
  sentenceCase,
  rowText,
  taskStateText,
  trashedText,
  keyboardShortcuts,
} from "./core/lumenna_web.js";
import type {
  Asking,
  DayItem,
  DayView,
  DeviceList,
  DeviceView,
  DevicesView,
  Notice,
  SaveFile,
  DetailsView,
  Event,
  Item,
  ListView,
  Turn,
  ViewModel,
  ProposalWords,
  Question,
  ShortcutGroup,
  PairingWords,
} from "./core/lumenna_web.js";

let core: Core | undefined;
let app: App | undefined;

function store(): Core {
  if (!core) throw new Error("The store is not open yet.");
  return core;
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
  // The clock the core's whole sentences say times in: this browser's.
  const hour12 = new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hour12;
  void app.send({ Clock: hour12 ? "twelve_hour" : "twenty_four_hour" });
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
}

/** The question open, its buttons in the web's sentence case, a pick's choices as lines. */
export interface ShownAsking extends Asking {
  lines?: ShownChoice[];
}

/** The view model as the page draws it. */
export interface Screen extends Omit<ViewModel, "list" | "day" | "details" | "line" | "devices" | "asking"> {
  asking?: ShownAsking;
  devices?: Omit<DevicesView, "list"> & { list?: Omit<DeviceList, "devices"> & { devices: ShownDevice[] } };
  list?: Shown;
  day?: ShownDay;
  details?: ShownDetails;
  line?: ShownLine;
}

/** A proposal's words with its buttons and headings in the web's sentence case. */
function webWords(words: ProposalWords): ProposalWords {
  const out = { ...words };
  for (const key of ["title", "item", "apply", "cancel", "unfit"] as const) out[key] = sentenceCase(out[key]);
  if (out.others) out.others = sentenceCase(out.others);
  return out;
}

/** A question with its buttons, and a choice of answers' answers, in the web's sentence case. */
function webQuestion(question: Question): Question {
  switch (question.ask) {
    case "confirm":
      return { ...question, yes: sentenceCase(question.yes) };
    case "text":
      return { ...question, yes: sentenceCase(question.yes) };
    case "pick":
      return { ...question, yes: sentenceCase(question.yes ?? "") };
    case "choose":
      return { ...question, answers: question.answers.map((answer) => ({ ...answer, title: sentenceCase(answer.title) })) };
    default:
      return question;
  }
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
  const details = view.details && { ...view.details, state: taskStateText(view.details.task) };
  const proposal = view.proposal && { ...view.proposal, words: webWords(view.proposal.words) };
  const asking = view.asking && {
    ...view.asking,
    question: webQuestion(view.asking.question),
    lines: view.asking.choices?.choices.map((choice) => ({
      id: choice.id,
      text: choiceText(choice),
      depth: choice.depth,
      reason: choice.reason ?? undefined,
    })),
  };
  const devices = view.devices && {
    ...view.devices,
    list: view.devices.list && {
      ...view.devices.list,
      devices: view.devices.list.devices.map((device) => ({ ...device, text: deviceText(device) })),
    },
  };
  const due = (view.due ?? []).map((item) => ({
    ...item,
    dismiss: sentenceCase(item.dismiss),
    snooze: item.snooze ? sentenceCase(item.snooze) : undefined,
    carry: item.carry ? sentenceCase(item.carry) : undefined,
    replan: item.replan ? sentenceCase(item.replan) : undefined,
  }));
  const settings = view.settings && {
    ...view.settings,
    exports: view.settings.exports.map((choice) => ({ ...choice, label: sentenceCase(choice.label) })),
  };
  const line = view.line && {
    text: view.line.text,
    cursor: unitsAt(view.line.text, view.line.cursor),
    readback: view.line.readback,
    hasErrors: view.line.has_errors,
    offer: {
      start: unitsAt(view.line.text, view.line.offer.start),
      end: unitsAt(view.line.text, view.line.offer.end),
      candidates: view.line.offer.candidates.map((candidate, index) => ({
        id: String(index),
        text: candidate.text,
        label: candidateText(candidate),
      })),
    },
  };
  return {
    ...view,
    sidebar: view.sidebar.map((entry) => ("Group" in entry.kind ? { ...entry, text: sentenceCase(entry.text) } : entry)),
    list,
    day,
    details,
    line,
    proposal,
    settings,
    devices,
    due,
    asking,
  };
}

function turned(turn: Turn): Said {
  const date = turn.view.day?.date;
  return {
    screen: screen(turn.view),
    said: turn.said.map((saying) => (saying.heading && date ? summaryText(date, saying.text) : saying.text)),
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
  send: async (event: Event): Promise<Said> => turned((await session().send(event)) as Turn),

  /** The quick-add line as typed, the cursor a UTF-16 offset as the input counts it. */
  typed: async (text: string, cursor: number): Promise<Said> =>
    turned((await session().send({ Typing: { Text: { text, cursor: bytesAt(text, cursor) } } })) as Turn),

  /**
   * Hands `listener` every turn the app leaves — for what the person did, a pairing's code and
   * words, a round's report, a sync's arrivals — one at a time, in order. A plain function that
   * calls the page's: a Comlink proxy answers any property, `.call` included.
   */
  listen(listener: (said: Said) => void): void {
    session().listen((turn: Turn) => void listener(turned(turn)));
  },

  /** Takes in what changed elsewhere since, or a new minute; what it leaves goes to the listener. */
  async tick(): Promise<void> {
    await session().tick();
  },

  /** What to draw now. */
  screen: (): Screen => screen(session().view()),


  /** A number that moves when another tab or process has written. */
  outsideVersion: (): number => store().outsideVersion(),

  /** Every sentence and button of pairing, buttons and questions in the web's sentence case. */
  pairingWords(): PairingWords {
    const words = pairingWords();
    for (const key of ["title", "wait", "join", "match_title", "match_yes", "match_no", "copy_code"] as const) {
      words[key] = sentenceCase(words[key]);
    }
    return { ...words, intro: words.intro_sentence };
  },

  /** Fixed text — a button, a question with no one's name in it — in sentence case. */
  sentence: (text: string): string => sentenceCase(text),

  /** The keyboard commands the desktop apps share, grouped as their menus are, in sentence case. */
  keyboardShortcuts: (): ShortcutGroup[] => keyboardShortcuts().groups,
};

export type Api = typeof api;

Comlink.expose(api);
