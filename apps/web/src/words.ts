// The core's fixed words — a button, a label, a hint — read once as the app starts, so a
// component says them as every other app does without waiting on the worker. Words that come
// with the screen (a setting's, a proposal's, pairing's) are the screen's, never here. Those a
// browser alone says (backups into a folder, reminders while the tab is open) are the core's too,
// in its words for "this browser".

import { core } from "./core";
import type { DetailsWords, FixedWords, QuickAddWords } from "./core";

export interface Words extends FixedWords {
  quickAdd: QuickAddWords;
  details: DetailsWords;
  /** Each command of the shared table by its id, its name in sentence case: "Previous day". */
  commands: Map<string, string>;
  nothingToChoose: string;
}

let loaded: Words | undefined;

/** Reads the words, once the store is open: the core's free functions need it loaded. */
export async function loadWords(): Promise<void> {
  const [quickAdd, details, groups, nothingToChoose, fixed] = await Promise.all([
    core.quickAddWords(),
    core.detailsWords(),
    core.keyboardShortcuts(),
    core.nothingToChoose(),
    core.fixedWords(),
  ]);
  const commands = new Map<string, string>();
  for (const group of groups) for (const command of group.commands) commands.set(command.id, command.title);
  loaded = { ...fixed, quickAdd, details, commands, nothingToChoose };
}

/** The core's words; read only once the app has started (`loadWords`). */
export function words(): Words {
  if (!loaded) throw new Error("The core's words are read once the app has started.");
  return loaded;
}

/** A shared command's name, in sentence case. */
export function commandTitle(id: string): string {
  return words().commands.get(id) ?? id;
}
