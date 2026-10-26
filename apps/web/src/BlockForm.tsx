// Adding or changing a block: the app's form (crates/app), drawn.
//
// Every setting a block has: what it is and when — title, start, length, kind, and the three
// flags, which start from the kind and follow it while it changes — then how it repeats and
// behaves. Which fields show, what saving sends (only what changed) and what is refused are the
// app's; a refusal keeps the form open with the reason in it.

import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  Checkbox,
  Dialog,
  Heading,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Modal,
  Popover,
  Select,
  SelectValue,
  Text,
  TextArea,
  TextField,
} from "react-aria-components";
import type { BlockFields, BlockFormView, FormField } from "./core";
import { FieldKind } from "./core";
import { CompletingField } from "./Completing";
import { DayPicker, TimePicker } from "./Pickers";
import { send } from "./session";
import { SwatchMark } from "./Swatch";

/**
 * A field's place in the form's values. A field is named as the core names it ("accepts_tasks"),
 * which is what its edit sends back; the values record is typed as BoltFFI writes its fields.
 */
function fieldOf(key: string): string {
  return key.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
}

/** Where the block form is shown: rendered once, in the app, while the app has one open. */
export function BlockForms(props: { form: BlockFormView | null }) {
  return (
    <Modal isDismissable isOpen={props.form !== null} onOpenChange={(isOpen) => !isOpen && void send({ tag: "Block", value0: { tag: "Cancel" } })}>
      {props.form && <Form form={props.form} />}
    </Modal>
  );
}

function Form(props: { form: BlockFormView }) {
  const { form } = props;
  // What each field shows, set at once as typed; the app's form is the same a moment later, and
  // replaces it when it says otherwise (a kind's flags, the start a new day brings). While typing
  // is on its way, an answer for earlier typing is not shown.
  const [values, setValues] = useState({ ...form.values, date: form.date });
  const pending = useRef(0);
  useEffect(() => {
    if (pending.current === 0) setValues({ ...form.values, date: form.date });
  }, [JSON.stringify(form.values), form.date]);

  const set = (key: string, value: string | boolean) => {
    setValues((now) => ({ ...now, [key]: value }));
    pending.current++;
    void send({ tag: "Block", value0: { tag: "Field", key, value: String(value) } }).then((turn) => {
      // The last answer is the form as all the typing left it, a kind's flags included.
      if (--pending.current === 0 && turn.screen.blockForm) setValues({ ...turn.screen.blockForm.values, date: turn.screen.blockForm.date });
    });
  };

  // Saved, the form closes, focus goes to the block and it is said, as every turn is; refused,
  // the form stays with why.
  const save = (event: FormEvent) => {
    event.preventDefault();
    void send({ tag: "Block", value0: { tag: "Save" } });
  };

  const control = (field: FormField, first: boolean) => {
    const key = fieldOf(field.key) as keyof BlockFields | "date";
    const help = field.hint && (
      <Text slot="description" className="quiet">
        {field.hint}
      </Text>
    );
    switch (field.kind) {
      case FieldKind.Toggle:
        return (
          <Checkbox key={field.key} className="check" isSelected={Boolean(values[key])} onChange={(on) => set(field.key, on)}>
            <span className="box" aria-hidden="true" />
            {field.label}
          </Checkbox>
        );
      case FieldKind.Choice:
        return (
          <Select key={field.key} className="field" value={String(values[key])} onChange={(chosen) => set(field.key, String(chosen))}>
            <Label>{field.label}</Label>
            <Button>
              <SelectValue />
            </Button>
            {help}
            <Popover>
              <ListBox items={field.options ?? []}>{(item) => (
                  <ListBoxItem id={item.id} textValue={item.title}>
                    <SwatchMark swatch={item.swatch} />
                    {item.title}
                  </ListBoxItem>
                )}</ListBox>
            </Popover>
          </Select>
        );
      case FieldKind.Lines:
        return (
          <TextField key={field.key} className="field" value={String(values[key])} onChange={(text) => set(field.key, text)}>
            <Label>{field.label}</Label>
            <TextArea rows={3} placeholder={field.example || undefined} />
            {help}
          </TextField>
        );
      default: {
        const line = String(values[key]);
        // A block's Tasks from is the filter language, which completes.
        if (field.syntax != null) {
          return (
            <CompletingField key={field.key} label={field.label} value={line} onChange={(text) => set(field.key, text)} syntax={field.syntax} description={field.hint} placeholder={field.example || undefined} />
          );
        }
        const typed = (
          <TextField key={field.key} className="field" value={line} onChange={(text) => set(field.key, text)} autoFocus={first}>
            <Label>{field.label}</Label>
            <Input placeholder={field.example || undefined} />
            {help}
          </TextField>
        );
        // A day or a time has the browser's picker beside its line, filling it.
        if (field.kind === FieldKind.Date) {
          return (
            <div key={field.key}>
              {typed}
              <DayPicker line={line} onPick={(day) => set(field.key, day)} />
            </div>
          );
        }
        if (field.kind === FieldKind.Time) {
          return (
            <div key={field.key}>
              {typed}
              <TimePicker line={line} onPick={(time) => set(field.key, time)} />
            </div>
          );
        }
        return typed;
      }
    }
  };

  const split = form.secondColumn ?? -1;
  const columns = split < 0 ? [form.fields] : [form.fields.slice(0, split), form.fields.slice(split)];
  const wide = form.secondColumn !== undefined && form.secondColumn !== null;

  return (
    <Dialog className={wide ? "react-aria-Dialog wide" : "react-aria-Dialog"}>
      <Heading slot="title">{form.heading}</Heading>
      <form onSubmit={save}>
        <div className={wide ? "columns" : undefined}>
          {columns.map((column, index) => (
            <div key={index}>{column.map((field) => control(field, field === form.fields[0]))}</div>
          ))}
        </div>
        {form.refusal && (
          <p role="alert" className="problem">
            {form.refusal}
          </p>
        )}
        <div className="buttons">
          <Button onPress={() => void send({ tag: "Block", value0: { tag: "Cancel" } })}>Cancel</Button>
          <Button type="submit">{form.yes}</Button>
        </div>
      </form>
    </Dialog>
  );
}
