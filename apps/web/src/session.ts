// The app, as the page holds it: the screen the worker last drew, and the way in.
//
// What is in hand, what is folded, which question is open and what to say are the app's
// (crates/app), run in the worker beside the store. The page sends what the person did and
// draws what comes back; between the two it only asks questions, in its own dialogs.

import { useSyncExternalStore } from "react";
import { core } from "./core";
import type { Action, ActionKind, Answer, Event, Said, Screen } from "./core";
import { ask, choose, confirm, pick } from "./Prompts";
import { say } from "./say";

let screen: Screen | undefined;
const listeners = new Set<() => void>();

function show(next: Screen) {
  screen = next;
  for (const listener of listeners) listener();
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

/** Starts drawing: once the store is open. */
export async function start(): Promise<void> {
  show(await core.screen());
}

let inFlight = 0;
let whenSettled: (() => void)[] = [];

/**
 * Sends what the person did, and draws what comes back. What to say is the caller's to say,
 * once focus is where the screen puts it.
 */
export async function send(event: Event): Promise<Said> {
  inFlight++;
  try {
    const turn = await core.send(event);
    show(turn.screen);
    return turn;
  } finally {
    if (--inFlight === 0) {
      const waiting = whenSettled;
      whenSettled = [];
      for (const resolve of waiting) resolve();
    }
  }
}

/**
 * Once nothing sent is still on its way, and the screen it left has been drawn: so a key pressed
 * straight after Go to Trash acts in the trash, as the person pressed them, not in the place the
 * page still showed.
 */
export function settled(): Promise<void> {
  const drawn = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  if (inFlight === 0) return Promise.resolve();
  return new Promise<void>((resolve) => whenSettled.push(resolve)).then(drawn);
}

/** The quick-add line as typed, the cursor a UTF-16 offset: drawn, with what it will add. */
export async function typed(text: string, cursor: number): Promise<Said> {
  inFlight++;
  try {
    const turn = await core.typed(text, cursor);
    show(turn.screen);
    return turn;
  } finally {
    if (--inFlight === 0) {
      const waiting = whenSettled;
      whenSettled = [];
      for (const resolve of waiting) resolve();
    }
  }
}

/** The screen as it is now, drawn. */
export async function current(): Promise<Screen> {
  const now = await core.screen();
  show(now);
  return now;
}

/**
 * Takes in what changed elsewhere — another tab, `lum`, a sync, a write made outside the app —
 * and draws it if anything did. Whether it did.
 */
export async function refresh(): Promise<boolean> {
  const turn = await core.tick();
  if (!turn) return false;
  show(turn.screen);
  return true;
}

/** Says each sentence of a turn, in order. */
export function sayAll(said: string[]): void {
  for (const sentence of said) say(sentence);
}

/**
 * The client's own form for an action whose question is one: Edit Details is the details beside
 * the list, Reminders the list of them. Answered when the form has been shown.
 */
export type Form = (action: Action) => void | Promise<void>;

/**
 * Runs an action through the app, asking each question it opens the web's way until none is
 * open: a confirmation as an alert dialog with Cancel first, a line of text as a dialog whose
 * field keeps the app's refusal, a pick as a list that narrows as you type, a choice of
 * answers as buttons. Every sentence said along the way, with the last turn.
 */
export async function run(first: Event, form?: Form): Promise<Said> {
  let turn = await send(first);
  const said = [...turn.said];
  while (turn.screen.asking) {
    const { action, question, choices } = turn.screen.asking;
    let answer: Answer | undefined;
    switch (question.ask) {
      case "immediate":
        answer = { answer: "yes" };
        break;
      case "form":
        if (action) await form?.(action);
        break;
      case "confirm":
        if (await confirm(question.title, question.message, await core.sentence(question.yes))) answer = { answer: "yes" };
        break;
      case "text": {
        // Each answer goes to the app; one it refuses stays at the field, with why.
        const text = await ask(
          question.sentence || question.title,
          question.label,
          question.hint,
          question.initial,
          async (text) => {
            turn = await send({ Answer: { answer: "text", text } });
            said.push(...turn.said);
            const refusal = turn.screen.asking?.refusal;
            return turn.screen.asking && turn.screen.asking.question.ask === "text" ? (refusal ?? undefined) : undefined;
          },
          await core.sentence(question.yes),
        );
        if (text === undefined) break;
        continue;
      }
      case "pick": {
        const items = await core.choiceLines(choices?.choices ?? []);
        const id = await pick(question.title, items, await core.sentence(question.yes ?? ""));
        if (id !== undefined) answer = { answer: "picked", id };
        break;
      }
      case "choose": {
        const answers = await Promise.all(question.answers.map((answer) => core.sentence(answer.title)));
        const index = await choose(question.title, question.message, answers);
        const chosen = index === undefined ? undefined : question.answers[index];
        if (chosen) answer = { answer: "picked", id: chosen.id };
        break;
      }
    }
    turn = await send(answer ? { Answer: answer } : "Cancel");
    said.push(...turn.said);
  }
  return { screen: turn.screen, said };
}

/** Runs the first of `kinds` the row in hand offers, or says why it offers none. */
export function runKinds(kinds: ActionKind[], form?: Form): Promise<Said> {
  return run({ Run: kinds }, form);
}
