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
import { send } from "./session";

/** Where the block form is shown: rendered once, in the app, while the app has one open. */
export function BlockForms(props: { form: BlockFormView | undefined }) {
  return (
    <Modal isDismissable isOpen={props.form !== undefined} onOpenChange={(isOpen) => !isOpen && void send({ Block: "Cancel" })}>
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
    void send({ Block: { Field: { key, value: String(value) } } }).then((turn) => {
      // The last answer is the form as all the typing left it, a kind's flags included.
      if (--pending.current === 0 && turn.screen.block_form) setValues({ ...turn.screen.block_form.values, date: turn.screen.block_form.date });
    });
  };

  // Saved, the form closes, focus goes to the block and it is said, as every turn is; refused,
  // the form stays with why.
  const save = (event: FormEvent) => {
    event.preventDefault();
    void send({ Block: "Save" });
  };

  const control = (field: FormField, first: boolean) => {
    const key = field.key as keyof BlockFields | "date";
    const help = field.hint && (
      <Text slot="description" className="quiet">
        {field.hint}
      </Text>
    );
    switch (field.kind) {
      case "toggle":
        return (
          <Checkbox key={field.key} className="check" isSelected={Boolean(values[key])} onChange={(on) => set(field.key, on)}>
            <span className="box" aria-hidden="true" />
            {field.label}
          </Checkbox>
        );
      case "choice":
        return (
          <Select key={field.key} className="field" value={String(values[key])} onChange={(chosen) => set(field.key, String(chosen))}>
            <Label>{field.label}</Label>
            <Button>
              <SelectValue />
            </Button>
            {help}
            <Popover>
              <ListBox items={field.options ?? []}>{(item) => <ListBoxItem id={item.id}>{item.title}</ListBoxItem>}</ListBox>
            </Popover>
          </Select>
        );
      case "lines":
        return (
          <TextField key={field.key} className="field" value={String(values[key])} onChange={(text) => set(field.key, text)}>
            <Label>{field.label}</Label>
            <TextArea rows={3} placeholder={field.example || undefined} />
            {help}
          </TextField>
        );
      default:
        return (
          <TextField key={field.key} className="field" value={String(values[key])} onChange={(text) => set(field.key, text)} autoFocus={first}>
            <Label>{field.label}</Label>
            <Input placeholder={field.example || undefined} />
            {help}
          </TextField>
        );
    }
  };

  const split = form.second_column ?? -1;
  const columns = split < 0 ? [form.fields] : [form.fields.slice(0, split), form.fields.slice(split)];
  const wide = form.second_column !== undefined && form.second_column !== null;

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
          <Button onPress={() => void send({ Block: "Cancel" })}>Cancel</Button>
          <Button type="submit">{form.yes}</Button>
        </div>
      </form>
    </Dialog>
  );
}
