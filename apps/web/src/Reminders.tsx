// Reminders: given while this tab is open, and a task's or block's own list.
//
// The tab that owns the store is this browser's one deliverer (a browser has no lock to take):
// every twenty seconds, and on coming back to the tab, it asks the core what came due since it
// last looked, from when it opened. Each is said through the live region, always, and shown as
// a notification where the browser allows; acting on one is in the page, in Reminders due,
// whose buttons are the core's Dismiss and Snooze, at a block's end with work left short its
// Carry Forward, and when a block's work outruns its time its Replan. When each fires and what it says are the core's; this decides nothing about
// them.
//
// A browser has no device settings the core can keep (no file system), so whether reminders
// are on in this browser is kept in its storage, under the core's name for the setting.

import { useEffect, useRef } from "react";
import { Button, Dialog, Heading, Modal } from "react-aria-components";
import { send } from "./session";
import type { Action, Due, DueEvent, ShownReminders } from "./core";

const ON = "lumenna:reminders";

/** Whether reminders are on in this browser: on unless turned off here. */
export function remindersOn(): boolean {
  try {
    return localStorage.getItem(ON) !== "false";
  } catch {
    return true;
  }
}

/** Turns reminders on or off in this browser; on asks to show notifications. */
export function setRemindersOn(on: boolean): void {
  try {
    localStorage.setItem(ON, on ? "true" : "false");
  } catch {
    // Storage refused: the choice lasts as long as the page.
  }
  if (on) void askToNotify();
}

/**
 * Asks to show notifications, once: when a reminder is first added or reminders are turned on.
 * Refused, reminders are still said and listed in the page.
 */
export async function askToNotify(): Promise<void> {
  try {
    if ("Notification" in window && Notification.permission === "default") await Notification.requestPermission();
  } catch {
    // A browser that cannot ask; the page says them anyway.
  }
}

/** Focuses a reminder's first button in Reminders due. */
function focusDue(id: string): void {
  document.getElementById(`due-${id}`)?.querySelector("button")?.focus();
}

/**
 * Gives this browser's reminders while the tab is open: every twenty seconds, and on coming
 * back to the tab, the app looks for what came due since the last look, from when the tab
 * opened. What came due, what each says and which are still listed are the app's, and so is
 * taking looks asked for while one is under way as one after it: a look skipped here while
 * another was on its way was lost, coming back to the tab included.
 */
export function useDelivery(ready: boolean): void {
  useEffect(() => {
    if (!ready) return;
    // From when the tab opened: nothing is owed from before.
    let since = new Date().toISOString();
    const look = () => {
      const now = new Date().toISOString();
      const from = since;
      since = now;
      if (remindersOn()) void send({ Due: { Look: { since: from, now } } });
    };
    const timer = setInterval(() => void look(), 20_000);
    const back = () => {
      if (document.visibilityState === "visible") void look();
    };
    document.addEventListener("visibilitychange", back);
    window.addEventListener("focus", back);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", back);
      window.removeEventListener("focus", back);
    };
  }, [ready]);
}

/** What came due and was not dealt with, each with the core's buttons. In the banner. */
export function RemindersDue(props: { due: Due[] }) {
  if (props.due.length === 0) return null;
  const deal = async (due: Due, event: DueEvent) => {
    const index = props.due.findIndex((d) => d.id === due.id);
    const turn = await send({ Due: event });
    // Replan opens its screen, which has focus; otherwise focus goes to what took the place of
    // the one dealt with, else out of the list that is gone.
    if (turn.screen.proposal) return;
    const left = turn.screen.due ?? [];
    const next = left[Math.min(index, left.length - 1)];
    setTimeout(() => (next ? focusDue(next.id) : document.querySelector<HTMLElement>(".app > header button")?.focus()), 0);
  };
  return (
    <section className="due" aria-labelledby="due-heading">
      <h2 id="due-heading">Reminders due</h2>
      <ul>
        {props.due.map((due) => (
          <li key={due.id} id={`due-${due.id}`}>
            <span id={`due-text-${due.id}`}>{due.text}</span>
            <Button aria-describedby={`due-text-${due.id}`} onPress={() => void deal(due, { Dismiss: due.id })}>
              {due.dismiss}
            </Button>
            {due.snooze && (
              <Button aria-describedby={`due-text-${due.id}`} onPress={() => void deal(due, { Snooze: due.id })}>
                {due.snooze}
              </Button>
            )}
            {due.replan && (
              <Button aria-describedby={`due-text-${due.id}`} onPress={() => void deal(due, { Replan: due.id })}>
                {due.replan}
              </Button>
            )}
            {due.carry && (
              <Button aria-describedby={`due-text-${due.id}`} onPress={() => void deal(due, { Carry: due.id })}>
                {due.carry}
              </Button>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

// ---------------------------------------------------------------------------------------
// A task's or block's own: what its Reminders action opens.
// ---------------------------------------------------------------------------------------

/** Where a task's or block's list of reminders is shown: rendered once, while the app has one open. */
export function ReminderLists(props: { list: ShownReminders | undefined }) {
  const close = () => void send("CloseReminders");
  return (
    <Modal isDismissable isOpen={props.list !== undefined} onOpenChange={(isOpen) => !isOpen && close()}>
      {props.list && <Reminders list={props.list} close={close} />}
    </Modal>
  );
}

function Reminders(props: { list: ShownReminders; close: () => void }) {
  const { list } = props;
  const adding = useRef<HTMLDivElement>(null);

  // Where focus goes after a reminder is removed is the app's (`reminders_held`): the one now
  // in its place, else Add Reminder.
  const act = async (action: Action) => {
    await send({ Perform: action });
    if (action.kind === "new") void askToNotify();
  };

  return (
    // Named by its heading, the core's count of what it has.
    <Dialog aria-labelledby="reminders-heading" data-list="reminders">
      <Heading slot="title" id="reminders-heading">
        {list.announcement}
      </Heading>
      {list.reminders.length > 0 && (
        <ul className="reminders" aria-labelledby="reminders-heading">
          {list.reminders.map((reminder) => (
            <li key={reminder.id} data-key={reminder.id} onFocus={() => void send({ Hold: ["Reminders", reminder.id] })}>
              <span id={`reminder-${reminder.id}`}>{reminder.text}</span>
              {(reminder.actions ?? []).map((action) => (
                <Button key={action.kind} aria-describedby={`reminder-${reminder.id}`} onPress={() => void act(action)}>
                  {action.sentence ?? action.title}
                </Button>
              ))}
            </li>
          ))}
        </ul>
      )}
      <div className="buttons" ref={adding} data-adding>
        {(list.actions ?? []).map((action) => (
          <Button key={action.kind} onPress={() => void act(action)}>
            {action.sentence ?? action.title}
          </Button>
        ))}
        <Button onPress={props.close}>Close</Button>
      </div>
    </Dialog>
  );
}
