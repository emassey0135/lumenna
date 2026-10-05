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
import type { Change, Entry, Place, RowView, Syntax, TaskDetail, TaskFields } from "./core/lumenna_web.js";

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
};

export type Api = typeof api;

Comlink.expose(api);
