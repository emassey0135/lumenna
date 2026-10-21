// Saying what happened: a screen reader does not notice a change it did not cause by moving
// focus, so every change says what it did. And someone who reads the screen reads it there too:
// the last sentence is shown on the page's status line (`useSaid`).

import { announce } from "@react-aria/live-announcer";
import { useEffect, useState } from "react";

let last = "";
const hearers = new Set<(text: string) => void>();

/**
 * Says a sentence politely: after whatever the screen reader is saying now — the row focus has
 * just moved to — rather than cutting it off. React Aria's announcer, which keeps its live
 * region in the page for every screen reader to hear.
 */
export function say(text: string | undefined): void {
  if (!text) return;
  announce(text, "polite");
  last = text;
  for (const hear of hearers) hear(text);
}

/** The last sentence said, for the status line, which shows it and says nothing itself. */
export function useSaid(): string {
  const [said, setSaid] = useState(last);
  useEffect(() => {
    hearers.add(setSaid);
    return () => void hearers.delete(setSaid);
  }, []);
  return said;
}
