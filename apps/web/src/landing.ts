// Where focus goes after a change: the same row if it is still listed, else whatever now holds
// its place — so a person working down a list does not lose their place.

import { useEffect, useRef } from "react";
import type { RefObject } from "react";

/**
 * Returns `land(key, index)`, which puts focus on the row `key` the next time `keys` changes,
 * or on the row at `index` if `key` is gone, and tells `onLanded` which row that was.
 *
 * A React Aria tree builds its collection in a pass of its own, so the rows on the page can
 * still be the old ones when `keys` changes: landing waits for rows that are all current.
 */
export function useLanding(
  tree: RefObject<HTMLElement | null>,
  keys: readonly string[],
  onLanded: (key: string | undefined) => void,
): (key: string | undefined, index: number) => void {
  const target = useRef<{ key?: string; index: number } | undefined>(undefined);

  useEffect(() => {
    const wanted = target.current;
    if (!wanted) return;
    target.current = undefined;
    const current = new Set(keys);
    let frame = 0;
    let tries = 0;
    const land = () => {
      const rows = [...(tree.current?.querySelectorAll<HTMLElement>('[role="row"]') ?? [])].filter(
        (row) => row.dataset.key !== undefined,
      );
      const stale = rows.some((row) => !current.has(row.dataset.key!));
      if ((stale || rows.length === 0) && keys.length > 0 && tries++ < 20) {
        frame = requestAnimationFrame(land);
        return;
      }
      const row = rows.find((r) => r.dataset.key === wanted.key) ?? rows[Math.min(wanted.index, rows.length - 1)];
      if (row) {
        row.focus();
        onLanded(row.dataset.key);
      } else {
        // Nothing left to hold focus: the list itself does, so it is not lost to the page.
        tree.current?.focus();
        onLanded(undefined);
      }
    };
    land();
    return () => cancelAnimationFrame(frame);
  }, [keys]);

  return (key, index) => {
    target.current = { key, index };
  };
}

/** The key of the row an event happened in, if any. */
export function rowKey(event: { target: EventTarget }): string | undefined {
  return (event.target as HTMLElement).closest<HTMLElement>('[role="row"]')?.dataset.key;
}
