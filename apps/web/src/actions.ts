// Which of a row's actions a key runs. The actions, their questions and running them are the
// app's (session.ts asks each question the web's way).

import type { ShortAction } from "./core";
import { ActionKind } from "./core";

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

/** The first of `actions` that is one of `kinds`, by the order of `kinds`: what a key runs. */
export function byKind<A extends { kind: ActionKind }>(actions: readonly A[] | undefined, ...kinds: ActionKind[]): A | undefined {
  for (const kind of kinds) {
    const found = actions?.find((action) => action.kind === kind);
    if (found) return found;
  }
  return undefined;
}

/** What Delete runs on a row: whatever removes it there. */
export const REMOVING: ActionKind[] = [ActionKind.Delete, ActionKind.DeleteForGood, ActionKind.Unassign, ActionKind.Unpair];
