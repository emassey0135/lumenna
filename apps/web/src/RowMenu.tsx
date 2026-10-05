// What can be done to the row in hand, as a menu: the desktop apps' context menu.
//
// It opens from its button, or from the row itself with the Menu key or Shift+F10 — the
// keys a screen reader user reaches for — or a right-click. Focus goes back where it was
// when it closes, and only then does the chosen action run, so a dialog the action opens
// returns focus to the row rather than to a menu item that no longer exists.

import { Button, Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";
import type { KeyboardEvent } from "react";

export interface Action {
  id: string;
  label: string;
  run: () => void;
}

export function RowMenu(props: {
  actions: Action[];
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  label?: string;
}) {
  return (
    <MenuTrigger isOpen={props.isOpen} onOpenChange={props.onOpenChange}>
      <Button isDisabled={props.actions.length === 0}>{props.label ?? "Actions"}</Button>
      <Popover>
        <Menu
          items={props.actions}
          onAction={(id) => {
            const action = props.actions.find((a) => a.id === id);
            if (action) setTimeout(action.run, 0);
          }}
        >
          {(action) => <MenuItem id={action.id}>{action.label}</MenuItem>}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

/** Whether a key asks for the menu: the Menu key, or Shift+F10. */
export function asksForMenu(event: KeyboardEvent): boolean {
  return event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
}
