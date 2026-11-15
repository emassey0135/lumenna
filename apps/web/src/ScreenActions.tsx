// What the screen shown offers of its own beside its rows, as buttons: the core's
// (`ViewModel::screen_actions`), in its order, under its names and enabled as it says. Which a
// place has (New task on a list, the day's moves on Today, Undo and Redo everywhere) is never
// decided here; a row's own actions are its menu's.
//
// Each is sent back as the shared key it is (`Key(id)`), which the app runs, but for what the
// core marks the shell's own (`shell`): the filter field, the keyboard help, which this page
// runs as it does their keys (`runCommand`).

import { Button } from "react-aria-components";
import type { ScreenAction } from "./core";
import { keyPressed } from "./actions";
import { runCommand } from "./commands";
import { send } from "./session";

/** Runs one of the screen's own actions. New task goes through the page's own command, which
 * opens quick add at once, before the app's turn, so what is typed straight away goes in; it
 * then sends the same key. */
export function runScreenAction(action: ScreenAction): void {
  if (action.shell || action.id === "new-task") {
    runCommand(action.id);
    return;
  }
  // Focus stays on the button: the app lands it only where it already was on the row in hand.
  void send(keyPressed(action.id));
}

/** The screen's own actions, each a button, for the toolbar or button row the screen has. */
export function ScreenActions(props: { actions: ScreenAction[] | null | undefined }) {
  return (
    <>
      {(props.actions ?? []).map((action) => (
        <Button
          key={action.id}
          isDisabled={!action.enabled}
          // One that asks something first says so the web's way: a screen reader hears that it
          // opens a dialog, and a sighted reader sees the ellipsis, which is left out of its name.
          aria-haspopup={action.asks ? "dialog" : undefined}
          onPress={() => runScreenAction(action)}
        >
          {action.title}
          {action.asks && <span aria-hidden="true">…</span>}
        </Button>
      ))}
    </>
  );
}
