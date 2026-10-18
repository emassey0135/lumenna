// Adding a task the way it would be said: one line, read back as it is typed.
//
// The line is a genuine combobox: Down opens what could go at the cursor, as a list the
// screen reader announces with its count, and choosing one puts it into exactly the span the
// core says it replaces — mid-line too, which the stock comboboxes of the desktop toolkits
// cannot do. Nothing opens by itself after a `#` or `@`, since typing a name straight through
// is common. The readback after the field says what will be added; Tab reaches it.

import { useContext, useEffect, useRef, useState } from "react";
import type { ContextType } from "react";
import { Button, ComboBox, ComboBoxStateContext, Dialog, Heading, Input, Label, ListBox, ListBoxItem, Modal, Popover } from "react-aria-components";
import type { Key } from "react-aria-components";
import type { ShownLine } from "./core";
import { sayAll, send, typed } from "./session";


export function QuickAdd(props: { line: ShownLine | undefined; isOpen: boolean; onClose: () => void; onAdded: () => void }) {
  // What the field shows, set at once as typed so the caret stays put; the app's line is the same
  // a moment later. While typing is on its way, an answer for earlier typing is not shown.
  const [text, setText] = useState(props.line?.text ?? "");
  const pending = useRef(0);
  const opened = useRef(false);
  // The combobox's own state, so Down can open the list once a fresh offer is in it.
  const combo = useRef<ContextType<typeof ComboBoxStateContext>>(null);
  const input = useRef<HTMLInputElement>(null);
  // The line a chosen completion makes, which replaces the item text React Aria would put in.
  const spliced = useRef<string | undefined>(undefined);
  const line = props.line;
  const offer = line?.offer ?? { start: 0, end: 0, candidates: [] };

  // Opening starts the line with what the place adds; closing lets it go.
  useEffect(() => {
    if (props.isOpen && !opened.current) {
      opened.current = true;
      void send({ Typing: "Open" }).then((turn) => setText(turn.screen.line?.text ?? ""));
    } else if (!props.isOpen && opened.current) {
      opened.current = false;
      void send({ Typing: "Close" });
    }
  }, [props.isOpen]);

  useEffect(() => {
    if (line && pending.current === 0) setText(line.text);
  }, [line?.text]);

  /** Tells the app the line and where the cursor is now, which decides what fits. */
  const tell = (value: string) => {
    const cursor = input.current?.selectionStart ?? value.length;
    pending.current++;
    return typed(value, cursor).finally(() => {
      pending.current--;
    });
  };

  const choose = (key: Key | null) => {
    const at = offer.candidates.findIndex((c) => c.id === key);
    if (at < 0) return;
    void send({ Typing: { Choose: at } }).then((turn) => {
      const next = turn.screen.line;
      if (!next) return;
      spliced.current = next.text;
      setText(next.text);
      requestAnimationFrame(() => input.current?.setSelectionRange(next.cursor, next.cursor));
    });
  };

  const add = async () => {
    if (!text.trim()) return;
    const turn = await send({ QuickAdd: text });
    // Refused, the dialog stays, with what was typed, so it can be put right.
    if (!turn.screen.add_refused) {
      props.onClose();
      props.onAdded();
    }
    sayAll(turn.said);
  };

  return (
    <Modal isDismissable isOpen={props.isOpen} onOpenChange={(open) => !open && props.onClose()}>
      <Dialog>
        <Heading slot="title">New task</Heading>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void add();
          }}
        >
          <ComboBox
            className="field"
            aria-describedby="quick-add-hint"
            allowsCustomValue
            menuTrigger="manual"
            // The core decides what fits at the cursor; React Aria would filter against the whole line.
            defaultFilter={() => true}
            inputValue={text}
            onInputChange={(value) => {
              // React Aria puts a chosen item's own text in; the splice is what belongs there.
              if (spliced.current !== undefined) {
                setText(spliced.current);
                spliced.current = undefined;
              } else {
                setText(value);
                void tell(value);
              }
            }}
            selectedKey={null}
            onSelectionChange={choose}
            items={offer.candidates}
          >
            <Label>Task</Label>
            <ComboState into={combo} />
            {/* Down opens what fits where the cursor is now. An offer still on its way would
                open the list on the last one, which then closes as the right one replaces it:
                so Down waits for it, and opens the list itself. */}
            <div
              className="contents"
              onKeyDownCapture={(event) => {
                const state = combo.current;
                if (event.key !== "ArrowDown" || !state || state.isOpen) return;
                const cursor = input.current?.selectionStart ?? text.length;
                if (line && line.text === text && line.cursor === cursor && pending.current === 0) return;
                event.preventDefault();
                event.stopPropagation();
                void tell(text).then((turn) => (turn.screen.line?.offer.candidates.length ?? 0) > 0 && state.open("first", "manual"));
              }}
            >
            <Input
              ref={input}
              autoFocus
              onKeyUp={(event) => {
                // A cursor moved without typing changes what fits.
                if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) void tell(text);
              }}
              onClick={() => void tell(text)}
            />
            </div>
            <Popover>
              <ListBox>{(item: ShownLine["offer"]["candidates"][number]) => <ListBoxItem textValue={item.label}>{item.label}</ListBoxItem>}</ListBox>
            </Popover>
          </ComboBox>
          <p id="quick-add-hint" className="quiet">
            Such as: write the chapter tomorrow p1 #Work. Down arrow offers what could come next.
          </p>
          <label htmlFor="quick-add-readback">Will add</label>
          <output id="quick-add-readback" className="readback" tabIndex={0}>
            {line?.readback ?? ""}
          </output>
          <div className="buttons">
            <Button onPress={props.onClose}>Cancel</Button>
            <Button type="submit" isDisabled={!text.trim()}>
              Add
            </Button>
          </div>
        </form>
      </Dialog>
    </Modal>
  );
}

/** Hands the enclosing combobox's state to `into`, for what its props cannot ask of it. */
function ComboState(props: { into: { current: ContextType<typeof ComboBoxStateContext> } }) {
  props.into.current = useContext(ComboBoxStateContext);
  return null;
}
