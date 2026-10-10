// The keyboard commands: each one a name from the desktop apps' shared table
// (`lumenna_desktop::keys`), run by whichever screen on the page can answer it.
//
// A screen offers a command while it is shown (`useCommand`); the highest priority answers
// first, the newest among equals, and a handler that cannot act this time returns false, so the
// next one is asked. A command the screen in front cannot answer — a day's command while the
// task list is shown — goes somewhere first and is `defer`red: it runs as soon as a screen
// offering it appears.

import { useEffect, useRef } from "react";

/** What a command does; false when it cannot act now, so another is asked. */
export type Handler = () => boolean | void;

interface Entry {
  priority: number;
  handler: { current: Handler };
}

const offered = new Map<string, Entry[]>();
const deferred = new Set<string>();

/** Offers command `id` while the calling component is shown. */
export function useCommand(id: string, handler: Handler, priority = 0) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    const entry: Entry = { priority, handler: ref };
    const list = offered.get(id) ?? [];
    list.push(entry);
    offered.set(id, list);
    if (deferred.has(id)) {
      // A frame later, so the screen has drawn what the command acts on.
      requestAnimationFrame(() => {
        if (deferred.delete(id)) ref.current();
      });
    }
    return () => {
      const at = list.indexOf(entry);
      if (at >= 0) list.splice(at, 1);
    };
  }, [id, priority]);
}

/** Runs command `id`; false if nothing on the page took it. */
export function runCommand(id: string): boolean {
  const list = [...(offered.get(id) ?? [])].reverse();
  list.sort((a, b) => b.priority - a.priority);
  for (const entry of list) {
    if (entry.handler.current() !== false) return true;
  }
  return false;
}

/** Runs command `id` once a screen offering it is shown. */
export function defer(id: string) {
  deferred.add(id);
  // What the command does with focus wins over a move to a place still waiting to draw.
  moves++;
}

/** Counts moves of focus, so one still waiting gives way to a later one. */
let moves = 0;

/**
 * Focus in a landmark: the element it last held, if it is still there, else its list's row in
 * hand (the tree's one Tab stop), else the first thing that takes focus. Tried each frame for a
 * moment, since a place just chosen has not drawn yet.
 */
export function focusIn(pane: string, remembered = true) {
  let tries = 0;
  const move = ++moves;
  const attempt = () => {
    if (move !== moves) return;
    const landmark = document.querySelector<HTMLElement>(pane);
    if (!landmark) return;
    const last = remembered ? lastFocused.get(landmark) : undefined;
    // A row of the list, not the row an empty list draws to say so.
    const row =
      landmark.querySelector<HTMLElement>('[role="row"][data-key][tabindex="0"]') ??
      landmark.querySelector<HTMLElement>('[role="row"][data-key]');
    // A list not drawn yet is waited for a moment; a pane without one takes its first control.
    const waiting = !row && landmark.querySelector('[role="treegrid"], [role="grid"]') && tries++ < 30;
    if (!(last && last.isConnected && landmark.contains(last)) && waiting) {
      requestAnimationFrame(attempt);
      return;
    }
    const target =
      (last && last.isConnected && landmark.contains(last) ? last : undefined) ??
      row ??
      landmark.querySelector<HTMLElement>(
        'input:not([disabled]), textarea:not([disabled]), button:not([disabled]), [tabindex="0"]',
      );
    if (target) {
      target.focus();
    } else {
      // Nothing in it takes focus ("No task selected"): the pane itself does, so moving
      // through the panes still lands, and its name and text are read.
      landmark.tabIndex = -1;
      landmark.focus();
    }
  };
  attempt();
}

/** The panes F6 moves between on the desktop: places, the list, the task's details. */
const PANES = ["nav", "main", "aside"];

/** Where focus last was in each pane, so moving back lands on it. */
const lastFocused = new WeakMap<HTMLElement, HTMLElement>();

document.addEventListener("focusin", (event) => {
  const target = event.target as HTMLElement;
  for (const pane of PANES) {
    const landmark = target.closest<HTMLElement>(`.app > ${pane}`);
    // The pane itself, focused only for want of anything in it, is not a place to come back to.
    if (landmark && landmark !== target) lastFocused.set(landmark, target);
  }
});

/** Moves focus to the next pane (`step` 1) or the previous one (-1), round. */
export function movePane(step: number) {
  const here = PANES.findIndex((pane) => document.activeElement?.closest(`.app > ${pane}`));
  const next = PANES[(here + step + PANES.length + (here < 0 && step < 0 ? 1 : 0)) % PANES.length];
  focusIn(`.app > ${next}`);
}
