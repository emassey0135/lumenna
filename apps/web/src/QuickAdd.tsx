// Adding a task the way it would be said: one line, read back as it is typed.
//
// The line is a genuine combobox, the one every line in the filter language is (Completing.tsx):
// Down or Ctrl+Space opens what could go at the cursor, as a list the screen reader announces
// with its count, and choosing one puts it into exactly the span the core says it replaces —
// mid-line too, which the stock comboboxes of the desktop toolkits cannot do. Nothing opens by
// itself after a `#` or `@`, since typing a name straight through is common. The readback after
// the field says what will be added; Tab reaches it.

import { useEffect, useRef, useState } from "react";
import { Button, Dialog, Heading, Modal } from "react-aria-components";
import type { ShownLine } from "./core";
import { Syntax } from "./core";
import { CompletingField } from "./Completing";
import { send, typed } from "./session";

/** Quick add, open while the app has a line, and from the moment it is asked for (`opening`), so
 * what is typed straight away goes into it: opened with `Typing::Open`, closed by the app once a
 * task is added from it, or by Cancel or Escape here. */
export function QuickAdd(props: { line: ShownLine | null; opening: boolean; onClose: () => void }) {
  // Closed at once, and kept closed until it is asked for again: what is typed next belongs
  // wherever focus goes back to, and a turn still on its way must not open it again.
  const [closing, setClosing] = useState(false);
  useEffect(() => {
    if (props.opening) setClosing(false);
  }, [props.opening]);
  const close = () => {
    setClosing(true);
    props.onClose();
    void send({ tag: "Typing", value0: { tag: "Close" } });
  };
  const open = !closing && (props.line != null || props.opening);
  return (
    <Modal isDismissable isOpen={open} onOpenChange={(open) => !open && close()}>
      {open && <Line line={props.line} close={close} />}
    </Modal>
  );
}

function Line(props: { line: ShownLine | null; close: () => void }) {
  // What the field shows, set at once as typed so the caret stays put; the app's line is the same
  // a moment later. While typing is on its way, an answer for earlier typing is not shown.
  const [text, setText] = useState(props.line?.text ?? "");
  const pending = useRef(0);
  const input = useRef<HTMLInputElement | null>(null);
  const line = props.line;

  useEffect(() => {
    if (line && pending.current === 0) setText(line.text);
  }, [line?.text]);

  /** Tells the app the line and where the cursor is, for its readback. */
  const tell = (value: string) => {
    const cursor = input.current?.selectionStart ?? value.length;
    pending.current++;
    void typed(value, cursor).finally(() => {
      pending.current--;
    });
  };

  // Added, the app lets the line go and the dialog closes; refused, it stays with what was
  // typed, so it can be put right.
  const add = () => {
    if (text.trim()) void send({ tag: "QuickAdd", value0: text });
  };

  return (
    <Dialog>
      <Heading slot="title">New task</Heading>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
      >
        {/* Completes as every line in the filter language does, the same combobox. Enter with
            nothing offered open submits the form. */}
        <CompletingField
          label="Task"
          syntax={Syntax.QuickAdd}
          value={text}
          onChange={(value) => {
            setText(value);
            tell(value);
          }}
          description="Such as: write the chapter tomorrow p1 #Work."
          autoFocus
          inputRef={input}
        />
        <label htmlFor="quick-add-readback">Will add</label>
        <output id="quick-add-readback" className="readback" tabIndex={0}>
          {line?.readback ?? ""}
        </output>
        <div className="buttons">
          <Button onPress={props.close}>Cancel</Button>
          <Button type="submit" isDisabled={!text.trim()}>
            Add
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
