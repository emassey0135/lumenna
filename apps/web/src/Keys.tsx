// The keyboard shortcuts dialog: every key the web binds, grouped as the desktop apps' menus
// and lists are.
//
// The commands and their names are the desktop apps' shared table (`lumenna_desktop::keys`),
// so a command is called what it is called everywhere. The web lists only those it binds
// (`BOUND`): a browser keeps many of the rest for itself (Ctrl+N, Ctrl+W, Ctrl+T, Ctrl+1 to
// 4, Ctrl+Page Up and Down). What only the web has is `OWN`. A binding added anywhere in the
// app is added here, so the help cannot miss it.

import { useEffect, useState } from "react";
import { Button, Dialog, Heading, Modal } from "react-aria-components";
import { core } from "./core";
import type { ShortcutGroup } from "./core";

/** The shared commands the web binds, by the table's id. */
const BOUND = new Set(["undo", "redo", "open", "toggle", "delete", "actions"]);

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
  {
    id: "help",
    title: "",
    shortcuts: [{ id: "keyboard-help", title: "Keyboard shortcuts", keys: ["?"] }],
  },
];

/** A Mac's browser takes Command where others take Ctrl; the app answers either. */
const mac = /Mac|iPhone|iPad/.test(navigator.platform);

function spoken(key: string): string {
  return mac ? key.replace(/\bCtrl\+/g, "Command+") : key;
}

/**
 * The groups the dialog lists: the shared table's, in its order, holding only what the web
 * binds — the web's own keys added to the group they belong to, and a group of the web's own
 * placed before Help. The web's own replaces a shared one of the same id (Help's F1, which a
 * browser keeps).
 */
export function listed(shared: ShortcutGroup[]): ShortcutGroup[] {
  const groups = shared.map((group) => {
    const own = OWN.find((o) => o.id === group.id)?.shortcuts ?? [];
    const ids = new Set(own.map((s) => s.id));
    return { ...group, shortcuts: [...group.shortcuts.filter((s) => BOUND.has(s.id) && !ids.has(s.id)), ...own] };
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
