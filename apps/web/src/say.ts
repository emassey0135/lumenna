// Saying what happened (§13: announce state changes explicitly).

import { announce } from "@react-aria/live-announcer";

/**
 * Says a sentence politely: after whatever the screen reader is saying now — the row focus has
 * just moved to — rather than cutting it off. React Aria's announcer, which keeps its live
 * region in the page for every screen reader to hear.
 */
export function say(text: string | undefined): void {
  if (text) announce(text, "polite");
}
