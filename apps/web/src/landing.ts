// Where focus goes after a change: the same row if it is still listed, else whatever now holds
// its place — so a person working down a list does not lose their place.


/** The key of the row an event happened in, if any. */
export function rowKey(event: { target: EventTarget }): string | undefined {
  return (event.target as HTMLElement).closest<HTMLElement>('[role="row"]')?.dataset.key;
}

/**
 * Puts focus on the row `key` once the tree has drawn it, or on the tree itself when nothing is
 * in hand. A React Aria tree builds its collection in a pass of its own, so the row can be a
 * frame or two behind the screen.
 */
export function landOn(tree: HTMLElement | null, key: string | undefined): Promise<void> {
  return new Promise((resolve) => {
    // A dialog the change opened (a form, a proposal) has focus, and keeps it.
    if (document.querySelector('[role="dialog"], [role="alertdialog"]')) {
      resolve();
      return;
    }
    let tries = 0;
    const step = () => {
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

/** Puts focus on the row `key` of the list in the main landmark, once it is drawn. */
export function landInMain(key: string | undefined): Promise<void> {
  return landOn(document.querySelector<HTMLElement>(".app > main [role='treegrid']"), key);
}
