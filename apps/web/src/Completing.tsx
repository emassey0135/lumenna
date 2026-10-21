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

const nothing: Offered = { announcement: "", start: 0, end: 0, candidates: [] };

export function CompletingField(props: {
  label: string;
  value: string;
  onChange: (text: string) => void;
  syntax: Syntax;
  description?: string;
  placeholder?: string;
  autoFocus?: boolean;
  /** Enter with nothing offered open: what the field does when finished. */
  onEnter?: () => void;
  /** Leaving the field: a line applied when it is left. */
  onBlur?: () => void;
  error?: string | null;
  inputRef?: { current: HTMLInputElement | null };
}) {
  const [offer, setOffer] = useState<Offered>(nothing);
  const combo = useRef<ContextType<typeof ComboBoxStateContext>>(null);
  const own = useRef<HTMLInputElement | null>(null);
  const input = props.inputRef ?? own;
  // The line a chosen completion makes, which replaces the item text React Aria puts in.
  const spliced = useRef<string | undefined>(undefined);
  // Opened once the candidates are in the list: opened as they are set, it opened on none.
  const opening = useRef(false);
  useEffect(() => {
    if (!opening.current) return;
    opening.current = false;
    combo.current?.open("first", "manual");
  }, [offer]);

  /** Asks what fits at the cursor now, and opens the list on it; says so when nothing does. */
  const offerHere = () => {
    const cursor = input.current?.selectionStart ?? props.value.length;
    void core.complete(props.value, cursor, props.syntax).then((found) => {
      if (found.candidates.length > 0) opening.current = true;
      else say(found.announcement || "Nothing to complete here.");
      setOffer(found);
    });
  };

  const choose = (key: Key | null) => {
    const chosen = offer.candidates.find((c) => c.id === key);
    if (!chosen) return;
    const next = props.value.slice(0, offer.start) + chosen.text + props.value.slice(offer.end);
    const cursor = offer.start + chosen.text.length;
    spliced.current = next;
    props.onChange(next);
    setOffer(nothing);
    requestAnimationFrame(() => input.current?.setSelectionRange(cursor, cursor));
  };

  const keys = (event: KeyboardEvent) => {
    const open = combo.current?.isOpen ?? false;
    if (!open && (event.key === "ArrowDown" || (event.key === " " && event.ctrlKey))) {
      event.preventDefault();
      event.stopPropagation();
      offerHere();
    } else if (!open && event.key === "Enter") {
      props.onEnter?.();
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
        {[props.description, "Down arrow offers what could come next."].filter(Boolean).join(" ")}
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
