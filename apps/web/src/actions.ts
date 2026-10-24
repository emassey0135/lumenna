// Which of a row's actions a key runs. The actions, their questions and running them are the
// app's (session.ts asks each question the web's way).

import type { ActingOn, ShellEvent, ShortAction } from "./core";
import { ActionKind, Side } from "./core";

/** What a menu or a key needs of an action, whole (`Action`) or a list row's in short. */
export interface Named {
  kind: ActionKind;
  title: string;
  sentence: string;
  destructive: boolean;
}

/** A list row's actions, in short: its places among the list's own (`ListView::offered`). */
export function shortActions(list: { offered: readonly ShortAction[] } | null | undefined, item: { actions: ArrayLike<number> } | undefined): ShortAction[] {
  if (!list || !item) return [];
  // BoltFFI hands a list of numbers over as a typed array.
  return Array.from(item.actions).flatMap((at) => (list.offered[at] ? [list.offered[at]] : []));
}

/** An action handed back in short, on the row it names: the app puts that row in hand and runs
 *  its action whole as it has it now (`Event::ActOn`). */
export function actOn(on: ActingOn, action: ShortAction): ShellEvent {
  return { tag: "ActOn", value0: on, value1: action };
}

/** A row of the place shown: a list's, the day's. */
export const rowShown = (key: string): ActingOn => ({ tag: "Row", value0: key });

/** A row of a list beside the main one. */
export const rowHeld = (side: Side, key: string): ActingOn => ({ tag: "Held", value0: side, value1: key });

/** The first of `actions` that is one of `kinds`, by the order of `kinds`: what a key runs. */
export function byKind<A extends { kind: ActionKind }>(actions: readonly A[] | undefined, ...kinds: ActionKind[]): A | undefined {
  for (const kind of kinds) {
    const found = actions?.find((action) => action.kind === kind);
    if (found) return found;
  }
  return undefined;
}

/** A shared key pressed on the row in hand, by its id in the core's table ("delete", "toggle"):
 *  what it runs there is the core's (`keys::runs`), the same on every platform. */
export const keyPressed = (id: "delete" | "toggle"): ShellEvent => ({ tag: "Key", value0: id });

/** The shared key a key press is, on a row: Delete, or Space. */
export function sharedKey(event: { key: string }): "delete" | "toggle" | undefined {
  return event.key === "Delete" ? "delete" : event.key === " " ? "toggle" : undefined;
}
