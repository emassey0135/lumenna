// The keyboard shortcuts: every key the web binds, in one table, which both the bindings and
// the dialog listing them read, so the help cannot miss one.
//
// The commands and their names are the desktop apps' shared table (`lumenna_desktop::keys`),
// so a command is called what it is called everywhere, and the web has every one of them. A
// browser keeps many of the shared keys for itself, and a page cannot have them: Ctrl+N, Ctrl+W,
// Ctrl+T, Ctrl+1 to 4, Ctrl+Page Up and Down, F5, F6, Ctrl+F, Ctrl+K, Ctrl+B, Ctrl+S. Those
// commands are on Alt+Shift and a letter here, the same letter where it is free, which no
// browser and no screen reader takes (`WEB`). What only the web has is `OWN`.

import { useEffect, useState } from "react";
import { Button, Dialog, Heading, Modal } from "react-aria-components";
import { core } from "./core";
import type { ShortcutGroup } from "./core";

/**
 * The key the web uses for each shared command, by the table's id, where it differs from the
 * shared one. Letters Chrome and Edge take with Alt+Shift are never used: A (inactive dialogs),
 * B (bookmarks bar), I (feedback), T (toolbar).
 */
export const WEB: Record<string, string[]> = {
  "new-task": ["Alt+Shift+N"],
  "new-block": ["Alt+Shift+L"],
  "sync-now": ["Alt+Shift+Y"],
  settings: ["Alt+Shift+E"],
  // The tab is Lumenna's window: closing it is the browser's own key. Nothing else to bind.
  "close-window": ["Ctrl+W"],
  filter: ["Alt+Shift+F"],
  "go-today": ["Alt+Shift+1"],
  "go-tasks": ["Alt+Shift+2"],
  "go-blocks": ["Alt+Shift+3"],
  "go-trash": ["Alt+Shift+4"],
  "next-pane": ["Alt+Shift+Period"],
  "previous-pane": ["Alt+Shift+Comma"],
  "mark-done": ["Alt+Shift+K"],
  "save-task": ["Alt+Shift+S"],
  "put-in-block": ["Alt+Shift+W"],
  "move-to-project": ["Alt+Shift+M"],
  "previous-day": ["Alt+Shift+Page Up"],
  "next-day": ["Alt+Shift+Page Down"],
  "go-to-now": ["Alt+Shift+O"],
  "go-to-day": ["Alt+Shift+G"],
  "keyboard-help": ["?"],
};

/**
 * Shared commands a page cannot have at all: Quit would be the browser's, every tab of it, not
 * Lumenna's.
 */
const NOT_ON_THE_WEB = new Set(["quit"]);

/** The web's own keys: a group of the table's, or one of its own, and what goes in it. */
const OWN: ShortcutGroup[] = [
  {
    id: "lists",
    title: "",
    shortcuts: [
      { id: "expand", title: "Expand the row", keys: ["Right arrow"] },
      { id: "collapse", title: "Collapse the row", keys: ["Left arrow"] },
    ],
  },
  {
    id: "dialogs",
    title: "In a dialog",
    shortcuts: [
      { id: "complete", title: "Offer what could go at the cursor, in quick add", keys: ["Down arrow"] },
      { id: "close-dialog", title: "Close the dialog", keys: ["Escape"] },
    ],
  },
];

/** A Mac's browser takes Command where others take Ctrl, and calls Alt Option. */
const mac = /Mac|iPhone|iPad/.test(navigator.platform);

function spoken(key: string): string {
  return mac ? key.replace(/\bCtrl\+/g, "Command+").replace(/\bAlt\+/g, "Option+") : key;
}

/**
 * Whether `event` is `key`, written as the table writes it ("Alt+Shift+Page Up"). Letters and
 * digits are matched by the physical key, since Option+Shift+K on a Mac types a symbol.
 */
export function matches(key: string, event: KeyboardEvent): boolean {
  const parts = key.split("+");
  const name = parts.pop() ?? "";
  const want = new Set(parts);
  const ctrl = mac ? event.metaKey : event.ctrlKey;
  if (want.has("Ctrl") !== ctrl || want.has("Alt") !== event.altKey || want.has("Shift") !== event.shiftKey) {
    return false;
  }
  if (/^[A-Z]$/.test(name)) return event.code === `Key${name}`;
  if (/^[0-9]$/.test(name)) return event.code === `Digit${name}`;
  if (name === "Comma" || name === "Period") return event.code === name;
  return event.key === name.replace(/ /g, "");
}

/** The shared command `event` is the web's key for, if any. */
export function commandFor(event: KeyboardEvent): string | undefined {
  for (const [id, keys] of Object.entries(WEB)) {
    if (id === "close-window" || id === "keyboard-help") continue;
    if (keys.some((key) => matches(key, event))) return id;
  }
  return undefined;
}

/**
 * The groups the dialog lists: the shared table's, in its order, each command with the key the
 * web uses for it, and the web's own keys added to the group they belong to, a group of the
 * web's own placed before Help.
 */
export function listed(shared: ShortcutGroup[]): ShortcutGroup[] {
  const groups = shared.map((group) => {
    const own = OWN.find((o) => o.id === group.id)?.shortcuts ?? [];
    return {
      ...group,
      shortcuts: [
        ...group.shortcuts
          .filter((s) => !NOT_ON_THE_WEB.has(s.id))
          .map((s) => ({ ...s, keys: WEB[s.id] ?? s.keys })),
        ...own,
      ],
    };
  });
  const help = groups.findIndex((group) => group.id === "help");
  const added = OWN.filter((own) => !shared.some((group) => group.id === own.id));
  groups.splice(help < 0 ? groups.length : help, 0, ...added);
  return groups.filter((group) => group.shortcuts.length > 0);
}

/** Whether a key press is "?" outside a text field or a dialog: the help's own key. */
export function asksForKeys(event: KeyboardEvent): boolean {
  if (event.key !== "?" || event.ctrlKey || event.metaKey || event.altKey) return false;
  const target = event.target as HTMLElement | null;
  return !target?.closest(
    "input, textarea, select, [contenteditable='true'], [role='combobox'], [role='dialog'], [role='alertdialog']",
  );
}

export function KeyboardShortcuts(props: { isOpen: boolean; onClose: () => void }) {
  const [groups, setGroups] = useState<ShortcutGroup[]>([]);

  useEffect(() => {
    if (props.isOpen && groups.length === 0) void core.keyboardShortcuts().then((shared) => setGroups(listed(shared)));
  }, [props.isOpen]);

  return (
    <Modal isDismissable isOpen={props.isOpen} onOpenChange={(open) => !open && props.onClose()}>
      <Dialog className="react-aria-Dialog wide">
        <Heading slot="title">Keyboard shortcuts</Heading>
        {groups.map((group) => (
          <section key={group.id} aria-labelledby={`keys-${group.id}`}>
            <h3 id={`keys-${group.id}`}>{group.title}</h3>
            <table className="keys" aria-labelledby={`keys-${group.id}`}>
              <thead>
                <tr>
                  <th scope="col">Key</th>
                  <th scope="col">Command</th>
                </tr>
              </thead>
              <tbody>
                {group.shortcuts.map((shortcut) => (
                  <tr key={shortcut.id}>
                    <td>{shortcut.keys.map(spoken).join(" or ")}</td>
                    <td>{shortcut.title}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ))}
        <div className="buttons">
          <Button onPress={props.onClose}>Close</Button>
        </div>
      </Dialog>
    </Modal>
  );
}
