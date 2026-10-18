// The app, as the page holds it: the screen the worker last drew, and the way in.
//
// What is in hand, what is folded, which question is open and what to say are the app's
// (crates/app), run in the worker beside the store. The page sends what the person did; every
// turn that comes back — the reply to an event, or one the network or a sync pushes later — is
// applied the one way (`apply`): drawn, focus moved where the app put it if the app moved it,
// then said. No screen moves focus or speaks for a turn itself.

import * as Comlink from "comlink";
import { useSyncExternalStore } from "react";
import { core } from "./core";
import type { Event, Notice, Said, Screen } from "./core";
import { landOn } from "./landing";
import { say } from "./say";

let screen: Screen | undefined;
let shownSeq = 0;
let landed = 0;
/** A turn whose focus lands wherever focus is: a command that goes somewhere, Next Day. */
let landAnywhere = false;
const listeners = new Set<() => void>();

/** Draws `next`, unless a newer screen is already drawn: a reply and a push can reach the page
 * in either order. Whether it was drawn. */
function show(next: Screen, seq?: number): boolean {
  if (seq !== undefined) {
    if (seq < shownSeq) return false;
    shownSeq = seq;
  }
  screen = next;
  for (const listener of listeners) listener();
  return true;
}

/** The screen as the app last drew it; undefined until the store is open. */
export function useScreen(): Screen | undefined {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => screen,
  );
}

/** Offers `data` as a file to save, called `name`. */
function download(name: string, data: BlobPart, type: string) {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/** Shows a notification, where the browser allows: the page has the reminder either way. */
function notify(notice: Notice) {
  try {
    if (!("Notification" in window) || Notification.permission !== "granted") return;
    const shown = new Notification(notice.title, { body: notice.text, tag: notice.id });
    shown.onclick = () => {
      window.focus();
      document.getElementById(`due-${notice.id}`)?.querySelector("button")?.focus();
      shown.close();
    };
  } catch {
    // A notification the browser will not show; the page has it.
  }
}

const frame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
const dialogOpen = () => document.querySelector('[role="dialog"], [role="alertdialog"]') !== null;

/** Whether the screen has a dialog of the app's open, which has focus and keeps it. */
function appDialog(s: Screen): boolean {
  return Boolean(s.asking || s.block_form || s.proposal || s.reminders);
}

/**
 * Puts focus on the main list's row `key`, if focus is the list's to move: in that list, or lost
 * (a row removed under it), or coming back to it from a dialog this turn closed. Focus the person
 * has taken elsewhere — the details, the sidebar, a dialog of the page's own — stays.
 */
async function land(key: string | undefined, anywhere: boolean) {
  // A dialog the turn closed leaves, and gives focus back where it was, first.
  let closing = false;
  for (let tries = 0; tries < 30 && dialogOpen(); tries++) {
    closing = true;
    await frame();
  }
  if (dialogOpen()) return;
  // React Aria gives focus back a frame after the dialog has gone.
  if (closing) {
    await frame();
    await frame();
  }
  // The place may have changed: its list is drawn a frame or two later.
  let tree = document.querySelector<HTMLElement>(".app > main [role='treegrid']");
  for (let tries = 0; tries < 10 && !tree; tries++) {
    await frame();
    tree = document.querySelector<HTMLElement>(".app > main [role='treegrid']");
  }
  const active = document.activeElement;
  const lost = !active || active === document.body;
  if (!anywhere && !closing && !lost && !tree?.contains(active)) return;
  await landOn(tree, key);
}

/** Applies a turn: draws it, saves its files, shows its notifications, lands focus if the app
 * moved it, then says what it says, after focus so the move does not cut it off. */
async function apply(turn: Said) {
  if (!show(turn.screen, turn.seq)) return;
  for (const file of turn.files) download(file.name, file.bytes as Uint8Array<ArrayBuffer>, file.kind);
  for (const notice of turn.notices) notify(notice);
  const s = turn.screen;
  if (s.landing > landed) {
    landed = s.landing;
    const anywhere = landAnywhere;
    landAnywhere = false;
    if (!appDialog(s)) await land(s.focus ?? undefined, anywhere);
  }
  for (const sentence of turn.said) say(sentence);
}

/** Starts drawing, once the store is open, and listens for the turns the network brings. */
export async function start(): Promise<void> {
  const first = await core.screen();
  landed = first.landing;
  show(first);
  await core.listen(Comlink.proxy((turn: Said) => void apply(turn)));
}

let inFlight = 0;
let whenSettled: (() => void)[] = [];

async function tracked(sending: Promise<Said>): Promise<Said> {
  inFlight++;
  try {
    // Every turn it left has been applied as it came, through the listener.
    return await sending;
  } finally {
    if (--inFlight === 0) {
      const waiting = whenSettled;
      whenSettled = [];
      for (const resolve of waiting) resolve();
    }
  }
}

/** Sends what the person did. Its turns are applied as every turn is; the screen once the app is
 * still is handed back too, for a form that keeps its own value while typing is on its way. With
 * `land`, the first landing it leads to puts focus in the list wherever focus is. */
export function send(event: Event, options?: { land?: boolean }): Promise<Said> {
  if (options?.land) landAnywhere = true;
  return tracked(core.send(event));
}

/** The quick-add line as typed, the cursor a UTF-16 offset. */
export function typed(text: string, cursor: number): Promise<Said> {
  return tracked(core.typed(text, cursor));
}

/**
 * Once nothing sent is still on its way, and the screen it left has been drawn: so a key pressed
 * straight after Go to Trash acts in the trash, as the person pressed them, not in the place the
 * page still showed.
 */
export function settled(): Promise<void> {
  if (inFlight === 0) return Promise.resolve();
  return new Promise<void>((resolve) => whenSettled.push(resolve)).then(frame);
}

/** Takes in what another tab or process wrote, or a new minute: what it leaves is applied as
 * every turn is. */
export function refresh(): Promise<void> {
  return core.tick();
}
