// Which of a row's actions a key runs. The actions, their questions and running them are the
// app's (session.ts asks each question the web's way).

import type { Action, ActionKind } from "./core";

/** The first of `actions` that is one of `kinds`, by the order of `kinds`: what a key runs. */
export function byKind(actions: readonly Action[] | undefined, ...kinds: ActionKind[]): Action | undefined {
  for (const kind of kinds) {
    const found = actions?.find((action) => action.kind === kind);
    if (found) return found;
  }
  return undefined;
}

/** What Delete runs on a row: whatever removes it there. */
export const REMOVING: ActionKind[] = ["delete", "delete_for_good", "unassign", "unpair"];
