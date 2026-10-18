// Finding a row, and putting focus on one once it is drawn. Which row, and when, are the app's
// (`landing` in session.ts).

/** The key of the row an event happened in, if any. */
export function rowKey(event: { target: EventTarget }): string | undefined {
  return (event.target as HTMLElement).closest<HTMLElement>('[role="row"]')?.dataset.key;
}

/**
 * Puts focus on the row `key` once the tree has drawn it, or on the tree itself when nothing is
 * in hand. A React Aria tree builds its collection in a pass of its own, so the row can be a
 * frame or two behind the screen. Focus the person moves somewhere else meanwhile stays there.
 */
export function landOn(tree: HTMLElement | null, key: string | undefined): Promise<void> {
  const from = document.activeElement;
  return new Promise((resolve) => {
    let tries = 0;
    const step = () => {
      const now = document.activeElement;
      if (now !== from && now && now !== document.body && !tree?.contains(now)) {
        resolve();
        return;
      }
      const row = key === undefined ? undefined : tree?.querySelector<HTMLElement>(`[role="row"][data-key="${CSS.escape(key)}"]`);
      if (!row && key !== undefined && tries++ < 20) {
        requestAnimationFrame(step);
        return;
      }
      (row ?? tree)?.focus();
      resolve();
    };
    requestAnimationFrame(step);
  });
}
