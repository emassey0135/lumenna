// Reminders: given while this tab is open, and a task's or block's own list.
//
// The tab that owns the store is this browser's one deliverer (a browser has no lock to take):
// every twenty seconds, and on coming back to the tab, it asks the core what came due since it
// last looked, from when it opened. Each is said through the live region, always, and shown as
// a notification where the browser allows; acting on one is in the page, in Reminders due,
// whose buttons are the core's Dismiss and Snooze, and at a block's end with work left short its
// Carry Forward. When each fires and what it says are the core's; this decides nothing about
// them.
//
// A browser has no device settings the core can keep (no file system), so whether reminders
// are on in this browser is kept in its storage, under the core's name for the setting.

import { useEffect, useRef, useState } from "react";
import { Button, Dialog, Heading, Modal } from "react-aria-components";
import { perform } from "./actions";
import { core } from "./core";
import type { Action, DueReminder, ReminderList } from "./core";
import { say } from "./say";

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

/** One thing that came due, as Reminders due lists it. */
export interface Due {
  /** What tells it apart, for the notification's tag and the page's element. */
  id: string;
  title: string;
  text: string;
  /** The buttons, in the core's words and the web's sentence case. */
  dismiss: string;
  snooze?: string;
  /** At a block's end with work left short, the core's Carry Forward; its text asks already. */
  carry?: string;
  /** What carrying forward carries: `<series>@<date>`. */
  block?: string;
  /** What dismissing it dismisses: itself, or for a summary, everything missed. */
  firings: DueReminder[];
}

function shown(due: Due, open: (id: string) => void): void {
  try {
    if (!("Notification" in window) || Notification.permission !== "granted") return;
    const notification = new Notification(due.title, { body: due.text, tag: due.id });
    notification.onclick = () => {
      window.focus();
      open(due.id);
      notification.close();
    };
  } catch {
    // A notification the browser will not show; the page has it.
  }
}

/** Focuses a reminder's first button in Reminders due. */
function focusDue(id: string): void {
  document.getElementById(`due-${id}`)?.querySelector("button")?.focus();
}

/** Gives this browser's reminders while the tab is open; `give` receives each batch. */
export function useDelivery(ready: boolean, give: (due: Due[]) => void): void {
  const giving = useRef(give);
  giving.current = give;
  useEffect(() => {
    if (!ready) return;
    // From when the tab opened: nothing is owed from before.
    let since = new Date().toISOString();
    let asking = false;
    const look = async () => {
      if (asking) return;
      asking = true;
      const now = new Date().toISOString();
      try {
        if (!remindersOn()) {
          since = now;
          return;
        }
        const found = await core.remindersDue(since, now);
        since = now;
        const batch: Due[] = [];
        const sentence = (text: string) => core.sentence(text);
        if (found.missed_summary && found.missed?.length) {
          const first = found.missed[0];
          batch.push({
            id: `missed-${first.key}-${first.occurrence}`,
            title: "Lumenna",
            text: found.missed_summary,
            dismiss: await sentence(first.dismiss),
            firings: found.missed,
          });
        }
        for (const reminder of found.due) {
          batch.push({
            id: `${reminder.key}-${reminder.occurrence}`,
            title: reminder.title,
            text: reminder.text,
            dismiss: await sentence(reminder.dismiss),
            snooze: await sentence(reminder.snooze),
            carry: reminder.carry ? await sentence(reminder.carry) : undefined,
            block: `${reminder.target}@${reminder.occurrence}`,
            firings: [reminder],
          });
        }
        if (batch.length === 0) return;
        giving.current(batch);
        for (const due of batch) {
          say(due.text);
          shown(due, focusDue);
        }
      } catch (error) {
        console.warn("Reminders:", (error as Error).message);
      } finally {
        asking = false;
      }
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
export function RemindersDue(props: { due: Due[]; onDealt: (id: string) => void; onChanged: () => void }) {
  if (props.due.length === 0) return null;
  const deal = async (due: Due, how: "dismiss" | "snooze" | "carry") => {
    const index = props.due.findIndex((d) => d.id === due.id);
    try {
      let said = "";
      if (how === "carry" && due.block) {
        // The work carried on, and the firing dealt with: it has been answered.
        said = await core.carryRestForward(due.block);
        for (const firing of due.firings) await core.dismissReminder(firing.key, firing.occurrence);
        props.onChanged();
      } else {
        for (const firing of due.firings) {
          said = await (how === "snooze" ? core.snoozeReminder(firing.key, firing.occurrence) : core.dismissReminder(firing.key, firing.occurrence));
        }
      }
      props.onDealt(due.id);
      // Focus to what took its place, else out of the list that is gone.
      const next = props.due.filter((d) => d.id !== due.id)[Math.min(index, props.due.length - 2)];
      setTimeout(() => (next ? focusDue(next.id) : document.querySelector<HTMLElement>(".app > header button")?.focus()), 0);
      say(said);
    } catch (error) {
      say((error as Error).message);
    }
  };
  return (
    <section className="due" aria-labelledby="due-heading">
      <h2 id="due-heading">Reminders due</h2>
      <ul>
        {props.due.map((due) => (
          <li key={due.id} id={`due-${due.id}`}>
            <span id={`due-text-${due.id}`}>{due.text}</span>
            <Button aria-describedby={`due-text-${due.id}`} onPress={() => void deal(due, "dismiss")}>
              {due.dismiss}
            </Button>
            {due.snooze && (
              <Button aria-describedby={`due-text-${due.id}`} onPress={() => void deal(due, "snooze")}>
                {due.snooze}
              </Button>
            )}
            {due.carry && (
              <Button aria-describedby={`due-text-${due.id}`} onPress={() => void deal(due, "carry")}>
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

let open: ((request: { target: string; done: () => void }) => void) | undefined;

/** Shows the reminders `action` (a Reminders action) is for, until closed. */
export function showReminders(action: Action): Promise<void> {
  return new Promise((done) => {
    if (open) open({ target: action.target, done });
    else done();
  });
}

/** Where a list of reminders is shown: rendered once, in the app. */
export function ReminderLists() {
  const [request, setRequest] = useState<{ target: string; done: () => void } | undefined>();

  useEffect(() => {
    open = setRequest;
    return () => {
      open = undefined;
    };
  }, []);

  const finish = () => {
    request?.done();
    setRequest(undefined);
  };

  return (
    <Modal isDismissable isOpen={request !== undefined} onOpenChange={(isOpen) => !isOpen && finish()}>
      {request && <Reminders target={request.target} close={finish} />}
    </Modal>
  );
}

function Reminders(props: { target: string; close: () => void }) {
  const [list, setList] = useState<ReminderList | undefined>();
  const [reload, setReload] = useState(0);
  const adding = useRef<HTMLDivElement>(null);

  useEffect(() => {
    core.remindersOf(props.target).then(setList, (error: Error) => say(error.message));
  }, [props.target, reload]);

  const run = async (action: Action) => {
    const done = await perform(action);
    if (!done) return;
    say(done.said);
    if (action.kind === "new") void askToNotify();
    setReload((r) => r + 1);
    // A removed reminder's button is gone: focus goes to adding one.
    if (action.kind !== "new") setTimeout(() => adding.current?.querySelector("button")?.focus(), 0);
  };

  return (
    // Named by its heading, the core's count of what it has.
    <Dialog aria-labelledby="reminders-heading">
      <Heading slot="title" id="reminders-heading">
        {list?.announcement ?? "Reminders"}
      </Heading>
      {list && list.reminders.length > 0 && (
        <ul className="reminders" aria-labelledby="reminders-heading">
          {list.reminders.map((reminder) => (
            <li key={reminder.id}>
              <span id={`reminder-${reminder.id}`}>{reminder.own ? reminder.words : `${reminder.words}, a default`}</span>
              {(reminder.actions ?? []).map((action) => (
                <Button key={action.kind} aria-describedby={`reminder-${reminder.id}`} onPress={() => void run(action)}>
                  {action.sentence ?? action.title}
                </Button>
              ))}
            </li>
          ))}
        </ul>
      )}
      <div className="buttons" ref={adding}>
        {(list?.actions ?? []).map((action) => (
          <Button key={action.kind} onPress={() => void run(action)}>
            {action.sentence ?? action.title}
          </Button>
        ))}
        <Button onPress={props.close}>Close</Button>
      </div>
    </Dialog>
  );
}
