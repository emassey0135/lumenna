// The core's fixed words — a button, a label, a hint, a pane's name — read once as the app
// starts, so a component says them as every other app does without waiting on the worker: its
// record of every word a shell draws (`ShellWords`), in sentence case and as a browser calls
// itself. Words that come with the screen (a setting's, a proposal's, pairing's) are the
// screen's, never here.

import { core } from "./core";
import type { FixedWords } from "./core";

export interface Words extends FixedWords {
  /** Each command of the shared table by its id, its name in sentence case: "Previous day". */
  commandTitles: Map<string, string>;
}

let loaded: Words | undefined;

/** Reads the words, as the app starts: they need the core's module, not the store. */
export async function loadWords(): Promise<void> {
  const fixed = await core.words();
  const commandTitles = new Map<string, string>();
  for (const group of fixed.keyGroups) for (const command of group.commands) commandTitles.set(command.id, command.title);
  loaded = { ...fixed, commandTitles };
}

/** The core's words; read only once they have been (`loadWords`). */
export function words(): Words {
  if (!loaded) throw new Error("The core's words are read once the app has started.");
  return loaded;
}

/** A group of the shared commands' heading, in sentence case: "Day". */
export function groupTitle(id: string): string {
  return words().keyGroups.find((group) => group.id === id)?.title ?? "";
}

/** A shared command's name, in sentence case. */
export function commandTitle(id: string): string {
  return words().commandTitles.get(id) ?? id;
}
