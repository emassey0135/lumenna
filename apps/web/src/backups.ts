// Automatic backups into a folder the person chooses, through the File System Access API
// (Chromium's), in a tab as much as installed: without it a browser takes no backups by itself.
// When one is due, which backups to remove, the file's name and what it says are the core's;
// this keeps the folder's handle, in IndexedDB, which can hold one, and writes there. The
// browser may ask again, after a restart, before writing: until it is allowed, nothing is.

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

function database(): Promise<IDBDatabase> {
  return new Promise((done, failed) => {
    const opening = indexedDB.open(DATABASE, 1);
    opening.onupgradeneeded = () => opening.result.createObjectStore("handles");
    opening.onsuccess = () => done(opening.result);
    opening.onerror = () => failed(opening.error);
  });
}

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

async function keep(folder: Folder | undefined): Promise<void> {
  held = folder;
  const db = await database();
  await new Promise<void>((done, failed) => {
    const handles = db.transaction("handles", "readwrite").objectStore("handles");
    const write = folder ? handles.put(folder, KEY) : handles.delete(KEY);
    write.onsuccess = () => done();
    write.onerror = () => failed(write.error);
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

/** Backs up into the folder if one is due there (or `now`), and removes the oldest beyond those
 * kept. The file written, or nothing. */
export async function backUp(now = false): Promise<string | undefined> {
  const handle = supported ? await kept().catch(() => undefined) : undefined;
  if (!handle || (await handle.queryPermission({ mode: "readwrite" })) !== "granted") return undefined;
  const names: string[] = [];
  for await (const name of handle.keys()) names.push(name);
  if (!now && !(await core.backupDue(names, Date.now()))) return undefined;
  const file = await core.backupFile();
  // A writable stream writes beside the file and puts it in place on close: an interrupted
  // backup never looks like a whole one.
  const writing = await (await handle.getFileHandle(file.name, { create: true })).createWritable();
  await writing.write(file.bytes as Uint8Array<ArrayBuffer>);
  await writing.close();
  for (const old of await core.backupsToRemove([...names, file.name])) {
    await handle.removeEntry(old).catch(() => undefined);
  }
  return file.name;
}

/** Backs up when one is due, as the app opens and every hour while it is open. */
export function useFolderBackups(ready: boolean) {
  useEffect(() => {
    if (!ready || !supported) return;
    void backUp().catch(() => undefined);
    const timer = setInterval(() => void backUp().catch(() => undefined), 60 * 60 * 1000);
    return () => clearInterval(timer);
  }, [ready]);
}
