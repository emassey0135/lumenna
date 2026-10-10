// Adding or changing a block.
//
// Every setting a block has: what it is and when — title, start, length, kind, and the
// three flags, which start from the kind and follow it while it changes — then how it repeats
// and behaves. The form for one day of a repeating block has only the first half: that is all
// one day can change. What saving sends is the surface's (`new_block`, `block_edit`): only
// what changed, so a concurrent edit to another field elsewhere stands. A change the core
// refuses keeps the form open with the core's sentence in it.

import { useEffect, useState } from "react";
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
import { core } from "./core";
import type { BlockFields, FormField } from "./core";

/** What the form is for. */
export type Purpose =
  | { kind: "add"; date: string }
  | { kind: "series"; id: string }
  | { kind: "occurrence"; series: string; date: string };

/** What saving did: the sentence to say, and the series added, if one was. */
export interface Saved {
  said: string;
  series?: string;
}

interface Request {
  purpose: Purpose;
  initial: BlockFields;
  /** The rule a series repeats by, when the repetition words cannot say it. */
  rule?: string;
  heading: string;
  /** The form's fields, in the core's words and order. */
  form: FormField[];
  answer: (saved?: Saved) => void;
}

let open: ((request: Request) => void) | undefined;

/** Runs the form; undefined if it was cancelled. */
export async function blockForm(purpose: Purpose, initial: BlockFields, heading: string, rule?: string): Promise<Saved | undefined> {
  const form = await core.blockForm();
  return new Promise((answer) => {
    if (open) open({ purpose, initial, rule, heading, form, answer });
    else answer(undefined);
  });
}

/** A new block's fields: a work block at `start` for `minutes`, with a work block's flags. */
export async function freshBlock(start: string, minutes: number): Promise<BlockFields> {
  const defaults = await core.kindDefaults("work");
  return {
    title: "",
    start,
    minutes: String(minutes),
    kind: "work",
    accepts_tasks: defaults?.accepts_tasks ?? true,
    counts_capacity: defaults?.counts_capacity ?? true,
    anchored: defaults?.anchored ?? false,
    repeat: "",
    until: "",
    min_minutes: "",
    task_filter: "",
    colour: "",
    notes: "",
  };
}

/** Where the block form is shown: rendered once, in the app. */
export function BlockForms() {
  const [request, setRequest] = useState<Request | undefined>();

  useEffect(() => {
    open = setRequest;
    return () => {
      open = undefined;
    };
  }, []);

  const finish = (saved?: Saved) => {
    request?.answer(saved);
    setRequest(undefined);
  };

  return (
    <Modal isDismissable isOpen={request !== undefined} onOpenChange={(isOpen) => !isOpen && finish()}>
      {request && <Form request={request} finish={finish} />}
    </Modal>
  );
}

/** What one day of a repeating block can change: the rest belongs to the series. */
const ONE_DAY = ["title", "start", "minutes", "kind", "accepts_tasks", "counts_capacity", "anchored"];

/** Where the second column starts: how it repeats and behaves, after what it is and when. */
const SECOND_COLUMN = "repeat";

function Form(props: { request: Request; finish: (saved?: Saved) => void }) {
  const { purpose, initial, heading, rule, form } = props.request;
  const [fields, setFields] = useState(initial);
  const [date, setDate] = useState(purpose.kind === "add" ? purpose.date : "");
  const [problem, setProblem] = useState<string | undefined>();
  const set = (change: Partial<BlockFields>) => setFields((now: BlockFields) => ({ ...now, ...change }));
  const once = purpose.kind === "occurrence";

  // A new kind brings its own flags, which can then be set apart from it.
  const kind = async (to: string) => {
    set({ kind: to });
    const defaults = await core.kindDefaults(to);
    if (defaults) set({ accepts_tasks: defaults.accepts_tasks, counts_capacity: defaults.counts_capacity, anchored: defaults.anchored });
  };

  const save = async (event: FormEvent) => {
    event.preventDefault();
    try {
      if (purpose.kind === "add") {
        props.finish(await core.addBlock(fields, date));
      } else {
        const series = purpose.kind === "series" ? purpose.id : purpose.series;
        const said = await core.saveBlock(series, purpose.kind === "occurrence" ? purpose.date : undefined, initial, fields);
        props.finish({ said: said ?? "Nothing changed" });
      }
    } catch (error) {
      setProblem((error as Error).message);
    }
  };

  const description = (field: FormField) => {
    // A rule the repetition words cannot say is kept while the field stays empty.
    if (field.key === "repeat" && rule && !initial.repeat) {
      return `${field.hint} It repeats by the rule ${rule}, which the repetition words cannot say; leave this empty to keep it.`;
    }
    return field.hint;
  };

  const control = (field: FormField, first: boolean) => {
    const key = field.key as keyof BlockFields;
    const hint = description(field);
    const help = hint && (
      <Text slot="description" className="quiet">
        {hint}
      </Text>
    );
    switch (field.kind) {
      case "toggle":
        return (
          <Checkbox key={field.key} className="check" isSelected={Boolean(fields[key])} onChange={(on) => set({ [key]: on })}>
            <span className="box" aria-hidden="true" />
            {field.label}
          </Checkbox>
        );
      case "choice":
        return (
          <Select
            key={field.key}
            className="field"
            value={String(fields[key])}
            onChange={(chosen) => (field.key === "kind" ? void kind(String(chosen)) : set({ [key]: String(chosen) }))}
          >
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
          <TextField key={field.key} className="field" value={String(fields[key])} onChange={(text) => set({ [key]: text })}>
            <Label>{field.label}</Label>
            <TextArea rows={3} placeholder={field.example || undefined} />
            {help}
          </TextField>
        );
      default: {
        // The day is the form's own, not the block's: only a new block has one.
        const value = field.key === "date" ? date : String(fields[key]);
        const change = (text: string) => (field.key === "date" ? setDate(text) : set({ [key]: text }));
        return (
          <TextField key={field.key} className="field" value={value} onChange={change} autoFocus={first}>
            <Label>{field.label}</Label>
            <Input placeholder={field.example || undefined} />
            {help}
          </TextField>
        );
      }
    }
  };

  const shown = form.filter(
    (field) =>
      (field.key !== "date" || purpose.kind === "add") &&
      (!once || ONE_DAY.includes(field.key)) &&
      (!field.repeating_only || fields.repeat.trim() !== "" || (rule !== undefined && !initial.repeat)),
  );
  const split = shown.findIndex((field) => field.key === SECOND_COLUMN);
  const columns = split < 0 ? [shown] : [shown.slice(0, split), shown.slice(split)];

  return (
    <Dialog className={once ? undefined : "wide"}>
      <Heading slot="title">{heading}</Heading>
      <form onSubmit={save}>
        <div className={once ? undefined : "columns"}>
          {columns.map((column, index) => (
            <div key={index}>{column.map((field) => control(field, field === shown[0]))}</div>
          ))}
        </div>
        {problem && (
          <p role="alert" className="problem">
            {problem}
          </p>
        )}
        <div className="buttons">
          <Button onPress={() => props.finish()}>Cancel</Button>
          <Button type="submit">{purpose.kind === "add" ? "Add" : "Save"}</Button>
        </div>
      </form>
    </Dialog>
  );
}
