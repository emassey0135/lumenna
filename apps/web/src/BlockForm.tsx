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
import type { BlockFields } from "./core";

const KINDS = [
  { id: "work", name: "Work" },
  { id: "break", name: "Break" },
  { id: "event", name: "Event" },
];

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
  answer: (saved?: Saved) => void;
}

let open: ((request: Request) => void) | undefined;

/** Runs the form; undefined if it was cancelled. */
export function blockForm(purpose: Purpose, initial: BlockFields, heading: string, rule?: string): Promise<Saved | undefined> {
  return new Promise((answer) => {
    if (open) open({ purpose, initial, rule, heading, answer });
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

function Form(props: { request: Request; finish: (saved?: Saved) => void }) {
  const { purpose, initial, heading, rule } = props.request;
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

  const field = (label: string, key: keyof BlockFields, description?: string, autoFocus = false) => (
    <TextField className="field" value={String(fields[key])} onChange={(text) => set({ [key]: text })} autoFocus={autoFocus}>
      <Label>{label}</Label>
      <Input />
      {description && (
        <Text slot="description" className="quiet">
          {description}
        </Text>
      )}
    </TextField>
  );

  const flag = (label: string, key: "accepts_tasks" | "counts_capacity" | "anchored") => (
    <Checkbox className="check" isSelected={fields[key]} onChange={(on) => set({ [key]: on })}>
      <span className="box" aria-hidden="true" />
      {label}
    </Checkbox>
  );

  return (
    <Dialog className={once ? undefined : "wide"}>
      <Heading slot="title">{heading}</Heading>
      <form onSubmit={save}>
        <div className={once ? undefined : "columns"}>
          <div>
            {field("Title", "title", undefined, true)}
            {field("Starts at", "start", "Such as 9am, or 14:30.")}
            {field("Minutes", "minutes")}
            <Select className="field" value={fields.kind} onChange={(key) => void kind(String(key))}>
              <Label>Kind</Label>
              <Button>
                <SelectValue />
              </Button>
              <Popover>
                <ListBox items={KINDS}>{(item) => <ListBoxItem id={item.id}>{item.name}</ListBoxItem>}</ListBox>
              </Popover>
            </Select>
            {flag("Takes tasks", "accepts_tasks")}
            {flag("Counts toward the hours for work", "counts_capacity")}
            {flag("Anchored: stays put when the day runs late", "anchored")}
            {purpose.kind === "add" && (
              <TextField className="field" value={date} onChange={setDate}>
                <Label>Starts on</Label>
                <Input />
                <Text slot="description" className="quiet">
                  A date, such as today or next Monday.
                </Text>
              </TextField>
            )}
          </div>
          {!once && (
            <div>
              {field(
                "Repeats",
                "repeat",
                rule && !initial.repeat
                  ? `Such as every weekday. It repeats by the rule ${rule}, which the repetition words cannot say; leave this empty to keep it.`
                  : "Such as every weekday. Empty for once.",
              )}
              {field("Last day it repeats", "until", "A date. Empty for for good.")}
              {field("Shortest length when the day runs late, in minutes", "min_minutes", "Empty for its kind's own.")}
              {field("Offers tasks matching this filter", "task_filter", "Such as #Work. Empty for any.")}
              {field("Colour", "colour", "By name, such as teal. Empty for none.")}
              <TextField className="field" value={fields.notes} onChange={(notes) => set({ notes })}>
                <Label>Notes</Label>
                <TextArea rows={3} />
              </TextField>
            </div>
          )}
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
