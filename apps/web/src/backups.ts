// Automatic backups into a folder the person chooses, through the File System Access API
// (Chromium's), in a tab as much as installed: without it a browser takes no backups by itself.
// When one is due, which backups to remove, the file's name and what it says are the core's;
// this keeps the folder's handle, in IndexedDB, which can hold one, and writes there. The
// browser may ask again, after a restart, before writing: until it is allowed, nothing is.
//
// A real erasure, here or taken in from another device, begins another generation of the
// store's history, and every backup taken before it holds what was erased. Every other device's
// host deletes its own and takes one afresh; it cannot reach this folder, so this does the same
// here, keeping beside the folder which generation its backups were taken of.

import { useEffect } from "react";
import { core } from "./core";

/** Whether this browser can be given a folder at all. */
export const supported = typeof window !== "undefined" && "showDirectoryPicker" in window;

// The parts of the API TypeScript's own types leave out.
interface Folder extends FileSystemDirectoryHandle {
  queryPermission(options: { mode: "readwrite" }): Promise<PermissionState>;
  requestPermission(options: { mode: "readwrite" }): Promise<PermissionState>;
}
type Picker = (options: { id: string; mode: "readwrite" }) => Promise<Folder>;

const DATABASE = "lumenna-backups";
const KEY = "folder";
/** Which generation of the store's history the folder's backups were taken of (none: the first). */
const OF = "backed-up-of";

function database(): Promise<IDBDatabase> {
  return new Promise((done, failed) => {
    const opening = indexedDB.open(DATABASE, 1);
    opening.onupgradeneeded = () => opening.result.createObjectStore("handles");
    opening.onsuccess = () => done(opening.result);
    opening.onerror = () => failed(opening.error);
  });
}

/** The backups an erasure left in the folder because removing them failed (a file another
 * program held), which every look tries again until they are gone. */
const LEFT = "erasure-left";

/** The folder this page last kept or read, so an hourly look reads nothing. */
let held: Folder | undefined;

async function kept(): Promise<Folder | undefined> {
  if (held) return held;
  const db = await database();
  return new Promise((done) => {
    const read = db.transaction("handles").objectStore("handles").get(KEY);
    read.onsuccess = () => done((held = read.result as Folder | undefined));
    read.onerror = () => done(undefined);
  });
}

async function put(key: string, value: unknown): Promise<void> {
  const db = await database();
  await new Promise<void>((done, failed) => {
    const handles = db.transaction("handles", "readwrite").objectStore("handles");
    const write = value === undefined ? handles.delete(key) : handles.put(value, key);
    write.onsuccess = () => done();
    write.onerror = () => failed(write.error);
  });
}

async function keep(folder: Folder | undefined): Promise<void> {
  held = folder;
  await put(KEY, folder);
}

/** The generation the folder's backups were last taken of, as this page last kept or read it. */
let backedUpOf: string | undefined;

async function generationBackedUp(): Promise<string> {
  if (backedUpOf !== undefined) return backedUpOf;
  const db = await database();
  return new Promise((done) => {
    const read = db.transaction("handles").objectStore("handles").get(OF);
    // Backups taken before generations were kept are of the first, as the store's history was.
    read.onsuccess = () => done((backedUpOf = typeof read.result === "string" ? read.result : ""));
    read.onerror = () => done("");
  });
}

/** What an erasure left to remove, as this page last kept or read it. */
let erasureLeft: string[] | undefined;

async function leftByErasure(): Promise<string[]> {
  if (erasureLeft !== undefined) return erasureLeft;
  const db = await database();
  return new Promise((done) => {
    const read = db.transaction("handles").objectStore("handles").get(LEFT);
    read.onsuccess = () => done((erasureLeft = Array.isArray(read.result) ? (read.result as string[]) : []));
    read.onerror = () => done([]);
  });
}

/** The folder backups go to, and whether this browser lets them be written there now. */
export async function folder(): Promise<{ name: string; allowed: boolean } | undefined> {
  if (!supported) return undefined;
  const handle = await kept().catch(() => undefined);
  if (!handle) return undefined;
  return { name: handle.name, allowed: (await handle.queryPermission({ mode: "readwrite" })) === "granted" };
}

/** Asks for a folder, and backs up there at once. Its name, or nothing when none was chosen. */
export async function choose(): Promise<string | undefined> {
  const picker = window as unknown as { showDirectoryPicker: Picker };
  const handle = await picker.showDirectoryPicker({ id: "lumenna-backups", mode: "readwrite" }).catch(() => undefined);
  if (!handle) return undefined;
  await keep(handle);
  await backUp(true);
  return handle.name;
}

/** Asks the browser again to write to the folder, as it may after a restart; a button's press. */
export async function allow(): Promise<boolean> {
  const handle = await kept();
  return (await handle?.requestPermission({ mode: "readwrite" })) === "granted";
}

/** Stops backing up to the folder; what is there stays. */
export async function stop(): Promise<void> {
  await keep(undefined);
}

/** The folder's writes, one after another. */
let going: Promise<unknown> = Promise.resolve();

/** Backs up into the folder if one is due there (or `now`), and removes the oldest beyond those
 * kept. After a real erasure every backup there goes first, and one is taken of the history
 * begun again. The file written, or nothing. */
export function backUp(now = false): Promise<string | undefined> {
  // One at a time: two at once after an erasure would each take a backup of the new history.
  const turn = going.then(() => backUpNow(now));
  going = turn.catch(() => undefined);
  return turn;
}

async function backUpNow(now: boolean): Promise<string | undefined> {
  const handle = supported ? await kept().catch(() => undefined) : undefined;
  if (!handle || (await handle.queryPermission({ mode: "readwrite" })) !== "granted") return undefined;
  let names: string[] = [];
  for await (const name of handle.keys()) names.push(name);
  const generation = await core.generation();
  const erased = generation !== (await generationBackedUp());
  // Every backup taken before an erasure holds what was erased: all of them go, and any whose
  // removal failed is tried again at every look, while backups go on being taken.
  const stale = (erased ? await core.folderBackups(names) : await leftByErasure()).filter((name) => names.includes(name));
  const failed: string[] = [];
  for (const old of stale) await handle.removeEntry(old).catch(() => failed.push(old));
  names = names.filter((name) => !stale.includes(name) || failed.includes(name));
  if (erased || (erasureLeft ?? []).length > 0) await put(LEFT, (erasureLeft = failed));
  if (!erased && !now && !(await core.backupDue(names, Date.now()))) return undefined;
  const file = await core.backupFile();
  // A writable stream writes beside the file and puts it in place on close: an interrupted
  // backup never looks like a whole one.
  const writing = await (await handle.getFileHandle(file.name, { create: true })).createWritable();
  await writing.write(file.bytes as Uint8Array<ArrayBuffer>);
  await writing.close();
  if (erased) await put(OF, (backedUpOf = generation));
  for (const old of await core.backupsToRemove([...names, file.name])) {
    await handle.removeEntry(old).catch(() => undefined);
  }
  return file.name;
}

/** Replaces the folder's backups once the store has moved on to another generation: a look
 * every few seconds, so what was erased does not wait an hour in them. */
async function followErasure(): Promise<void> {
  if (!held && !(await kept().catch(() => undefined))) return;
  if ((await core.generation()) !== (await generationBackedUp()) || (await leftByErasure()).length > 0) await backUp();
}

/** Backs up when one is due, as the app opens and every hour while it is open, and follows an
 * erasure as soon as it is seen. */
export function useFolderBackups(ready: boolean) {
  useEffect(() => {
    if (!ready || !supported) return;
    void backUp().catch(() => undefined);
    const hourly = setInterval(() => void backUp().catch(() => undefined), 60 * 60 * 1000);
    const following = setInterval(() => void followErasure().catch(() => undefined), 5 * 1000);
    return () => {
      clearInterval(hourly);
      clearInterval(following);
    };
  }, [ready]);
}
