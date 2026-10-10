// Running one of a record's actions: asking its question the web's way, then handing the action
// back with the answer to the core.
//
// Which actions a row has, what each is called, and what it asks are the core's (`actions` on
// every record it lists). Here is only how each kind of question is asked: a confirmation is an
// alert dialog with Cancel first, a line of text a dialog with a labelled field, a pick a dialog
// that narrows as you type, a choice of answers an alert dialog with a button for each. A
// refused answer to a line of text stays in its dialog, with the core's sentence at the field.

import { core } from "./core";
import type { Action, ActionKind } from "./core";
import { ask, choose, confirm, pick } from "./Prompts";
import { say } from "./say";

/** What an action did: what to say, and whether anything changed. */
export interface Done {
  said: string;
  changed: boolean;
  /** What was typed or picked, for a client choosing where to go next. */
  answer?: string;
}

/**
 * Asks `action`'s question and runs it; undefined when it was cancelled, offered nothing, or was
 * refused, which has been said already. A form's question is `form`'s, the client's own screen.
 */
export async function perform(
  action: Action,
  form?: (action: Action) => Promise<Done | undefined | void> | void,
): Promise<Done | undefined> {
  const q = action.question;
  try {
    switch (q.ask) {
      case "immediate":
        return await core.act(action, { answer: "yes" });
      case "form":
        return (await form?.(action)) || undefined;
      case "confirm":
        if (!(await confirm(q.title, q.message, q.yes))) return undefined;
        return await core.act(action, { answer: "yes" });
      case "text": {
        let done: Done | undefined;
        const text = await ask(q.title, q.label, q.hint, q.initial, async (text) => {
          try {
            done = await core.act(action, { answer: "text", text });
            return undefined;
          } catch (error) {
            return (error as Error).message;
          }
        });
        if (text === undefined || !done) return undefined;
        return { ...done, answer: text.trim() };
      }
      case "pick": {
        const offered = await core.choices(action);
        if (offered.choices.length === 0) {
          say(offered.announcement);
          return undefined;
        }
        const id = await pick(q.title, offered.choices);
        if (id === undefined) return undefined;
        if (q.length === undefined) return { ...(await core.act(action, { answer: "picked", id })), answer: id };
        // The second question, asked once one is picked: how long the sitting is meant to take.
        let done: Done | undefined;
        const length = await ask("Planned length", "Planned length", q.length, "", async (text) => {
          try {
            done = await core.act(action, { answer: "picked", id, length: text });
            return undefined;
          } catch (error) {
            return (error as Error).message;
          }
        });
        if (length === undefined || !done) return undefined;
        return { ...done, answer: id };
      }
      case "choose": {
        const index = await choose(q.title, q.message, q.answers.map((answer) => answer.title));
        const chosen = index === undefined ? undefined : q.answers[index];
        if (!chosen) return undefined;
        return { ...(await core.act(action, { answer: "picked", id: chosen.id })), answer: chosen.id };
      }
    }
  } catch (error) {
    say((error as Error).message);
  }
  return undefined;
}

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
