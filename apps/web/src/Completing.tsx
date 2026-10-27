// A line written in the filter language (or quick add's), which completes as quick add does:
// Down, or Ctrl+Space, offers what could go at the cursor, a list the screen reader announces
// with its count, and choosing one puts it into exactly the span the core says it replaces.
// Nothing opens by itself, after a `#` or an `@` or anywhere, since typing a name straight
// through is common. What fits is the core's (`App::complete`), asked as the key is pressed.

import { useContext, useEffect, useRef, useState } from "react";
import type { ContextType, KeyboardEvent } from "react";
import { ComboBox, ComboBoxStateContext, FieldError, Input, Label, ListBox, ListBoxItem, Popover, Text } from "react-aria-components";
import type { Key } from "react-aria-components";
import { core } from "./core";
import type { Offered, Syntax } from "./core";
import { say } from "./say";
import { words } from "./words";

const nothing: Offered = { announcement: "", start: 0, end: 0, candidates: [] };

export function CompletingField(props: {
  label: string;
  value: string;
  onChange: (text: string) => void;
  syntax: Syntax;
  description?: string;
  /** What the field takes and how it completes, whole, where the core says both: quick add's. */
  hint?: string;
  placeholder?: string;
  autoFocus?: boolean;
  /** Enter with nothing offered open: what the field does when finished. */
  onEnter?: () => void;
  /** Leaving the field: a line applied when it is left. */
  onBlur?: () => void;
  error?: string | null;
  inputRef?: { current: HTMLInputElement | null };
  /** What fits at `cursor` in `line`: the core's completion, unless the line's offer is the
   * app's own (quick add's, worded as its lines are). */
  offers?: (line: string, cursor: number) => Promise<Offered>;
}) {
  const [offer, setOffer] = useState<Offered>(nothing);
  const combo = useRef<ContextType<typeof ComboBoxStateContext>>(null);
  const own = useRef<HTMLInputElement | null>(null);
  const input = props.inputRef ?? own;
  // The line a chosen completion makes, which replaces the item text React Aria puts in.
  const spliced = useRef<string | undefined>(undefined);
  // Whether the list is open, as React Aria says it changes: the state the combobox hands down
  // can lag a render, and Enter taken for the line while the list was open lost the choice.
  const listOpen = useRef(false);
  // Opened once the candidates are in the list: opened as they are set, it opened on none.
  const opening = useRef(false);
  useEffect(() => {
    if (!opening.current) return;
    opening.current = false;
    combo.current?.open("first", "manual");
    // React Aria puts the first in focus once its collection is built, a render on; Enter can
    // come first, and chose nothing.
    const first = offer.candidates[0];
    if (first) combo.current?.selectionManager.setFocusedKey(first.id);
  }, [offer]);

  // The line the offer was made for. What the input holds, never `props.value`, which can still
  // be on its way back from the app when Down follows typing at once: offered for "@c" when the
  // field said "@ca", the choice came out "@callsa".
  const offeredFor = useRef("");
  /** Asks what fits at the cursor now, and opens the list on it; says so when nothing does. */
  const offerHere = () => {
    const line = input.current?.value ?? props.value;
    const cursor = input.current?.selectionStart ?? line.length;
    const asking = props.offers ? props.offers(line, cursor) : core.complete(line, cursor, props.syntax);
    void asking.then((found) => {
      if (found.candidates.length > 0) opening.current = true;
      else say(found.announcement || words().quickAdd.nothingToComplete);
      offeredFor.current = line;
      setOffer(found);
    });
  };

  const choose = (key: Key | null) => {
    const chosen = offer.candidates.find((c) => c.id === key);
    if (!chosen) return;
    const line = offeredFor.current;
    const next = line.slice(0, offer.start) + chosen.text + line.slice(offer.end);
    const cursor = offer.start + chosen.text.length;
    spliced.current = next;
    props.onChange(next);
    setOffer(nothing);
    requestAnimationFrame(() => input.current?.setSelectionRange(cursor, cursor));
  };

  const keys = (event: KeyboardEvent) => {
    if (!listOpen.current && (event.key === "ArrowDown" || (event.key === " " && event.ctrlKey))) {
      event.preventDefault();
      event.stopPropagation();
      offerHere();
    } else if (event.key === "Enter" && !listOpen.current) {
      props.onEnter?.();
    } else if (event.key === "Enter" && combo.current?.selectionManager.focusedKey == null && offer.candidates[0]) {
      // Open with none in focus yet: the first, which is what the list shows in focus.
      event.preventDefault();
      event.stopPropagation();
      choose(offer.candidates[0].id);
      combo.current?.close();
    }
  };

  return (
    <ComboBox
      className="field"
      allowsCustomValue
      menuTrigger="manual"
      // The core decides what fits at the cursor; React Aria would filter against the whole line.
      defaultFilter={() => true}
      inputValue={props.value}
      onInputChange={(value) => {
        if (spliced.current !== undefined) {
          props.onChange(spliced.current);
          spliced.current = undefined;
        } else {
          props.onChange(value);
        }
      }}
      selectedKey={null}
      onSelectionChange={choose}
      onOpenChange={(open) => void (listOpen.current = open)}
      items={offer.candidates}
      isInvalid={props.error != null}
      validationBehavior="aria"
      onBlur={props.onBlur}
    >
      <Label>{props.label}</Label>
      <ComboState into={combo} />
      <div className="contents" onKeyDownCapture={keys}>
        <Input ref={(element) => void (input.current = element)} placeholder={props.placeholder} autoFocus={props.autoFocus} />
      </div>
      <Text slot="description" className="quiet">
        {props.hint ?? [props.description, words().completionHint].filter(Boolean).join(" ")}
      </Text>
      <FieldError>{props.error}</FieldError>
      <Popover>
        <ListBox>{(item: Offered["candidates"][number]) => <ListBoxItem textValue={item.label}>{item.label}</ListBoxItem>}</ListBox>
      </Popover>
    </ComboBox>
  );
}

/** Hands the enclosing combobox's state to `into`, for what its props cannot ask of it. */
function ComboState(props: { into: { current: ContextType<typeof ComboBoxStateContext> } }) {
  props.into.current = useContext(ComboBoxStateContext);
  return null;
}
