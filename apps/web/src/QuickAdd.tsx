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
import { core } from "./core";
import { say } from "./say";

/** What fits in one place on one line: `text` and `cursor` say which. */
interface Offer {
  text: string;
  cursor: number;
  start: number;
  end: number;
  candidates: { id: string; text: string; label: string }[];
}

export function QuickAdd(props: { prefix: string; isOpen: boolean; onClose: () => void; onAdded: (id?: string) => void }) {
  const [text, setText] = useState(props.prefix);
  const [readback, setReadback] = useState("");
  const [offer, setOffer] = useState<Offer>({ text: "", cursor: -1, start: 0, end: 0, candidates: [] });
  // The combobox's own state, so Down can open the list once a fresh offer is in it.
  const combo = useRef<ContextType<typeof ComboBoxStateContext>>(null);
  const input = useRef<HTMLInputElement>(null);
  // The line a chosen completion makes, which replaces the item text React Aria would put in.
  const spliced = useRef<string | undefined>(undefined);

  useEffect(() => {
    if (props.isOpen) setText(props.prefix);
  }, [props.isOpen, props.prefix]);

  // The readback, as typed.
  useEffect(() => {
    let current = true;
    void core.preview(text).then(
      (preview) => {
        if (!current) return;
        setReadback(preview.readback);
      },
      (error: Error) => current && setReadback(error.message),
    );
    return () => {
      current = false;
    };
  }, [text]);

  /** What fits where the cursor is now. */
  const offerAtCursor = async (): Promise<Offer> => {
    const cursor = input.current?.selectionStart ?? text.length;
    const found = await core.completions(text, cursor, "quick-add");
    const fresh = {
      text,
      cursor,
      start: found.start,
      end: found.end,
      candidates: found.candidates.map((candidate, index) => ({ id: String(index), ...candidate })),
    };
    setOffer(fresh);
    return fresh;
  };

  useEffect(() => {
    void offerAtCursor();
  }, [text]);

  const choose = (key: Key | null) => {
    const candidate = offer.candidates.find((c) => c.id === key);
    if (!candidate) return;
    const line = text.slice(0, offer.start) + candidate.text + text.slice(offer.end);
    spliced.current = line;
    setText(line);
    const caret = offer.start + candidate.text.length;
    requestAnimationFrame(() => input.current?.setSelectionRange(caret, caret));
  };

  const add = async () => {
    if (!text.trim()) return;
    try {
      const added = await core.add(text);
      props.onClose();
      props.onAdded(added.id);
      say(added.said);
    } catch (error) {
      // The dialog stays, with what was typed, so it can be put right.
      say((error as Error).message);
    }
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
                if (offer.text === text && offer.cursor === cursor) return;
                event.preventDefault();
                event.stopPropagation();
                void offerAtCursor().then((fresh) => fresh.candidates.length > 0 && state.open("first", "manual"));
              }}
            >
            <Input
              ref={input}
              autoFocus
              onKeyUp={(event) => {
                // A cursor moved without typing changes what fits.
                if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) void offerAtCursor();
              }}
              onClick={() => void offerAtCursor()}
            />
            </div>
            <Popover>
              <ListBox>{(item: Offer["candidates"][number]) => <ListBoxItem textValue={item.label}>{item.label}</ListBoxItem>}</ListBox>
            </Popover>
          </ComboBox>
          <p id="quick-add-hint" className="quiet">
            Such as: write the chapter tomorrow p1 #Work. Down arrow offers what could come next.
          </p>
          <label htmlFor="quick-add-readback">Will add</label>
          <output id="quick-add-readback" className="readback" tabIndex={0}>
            {readback}
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
