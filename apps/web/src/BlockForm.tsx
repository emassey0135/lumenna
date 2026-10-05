// Adding or changing a block (§16.1: block editor; §13: creating is a form).
//
// Saving sends only what changed, so a concurrent edit to another field elsewhere stands. A
// change the core refuses keeps the form open with the core's sentence in it, so it can be put
// right. Like the prompts, it is a function returning a promise, shown by one host.

import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
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
  TextField,
} from "react-aria-components";
import { core } from "./core";

const KINDS = [
  { id: "work", name: "Work, takes tasks" },
  { id: "break", name: "Break" },
  { id: "event", name: "Event" },
];

/** What the form is for. */
export type Purpose =
  | { kind: "add"; date: string }
  | { kind: "series"; id: string }
  | { kind: "occurrence"; series: string; date: string };

/** A block's fields as the form shows them. */
export interface Fields {
  title: string;
  start: string;
  minutes: string;
  kind: string;
  repeat: string;
  /**
   * The RFC 5545 rule it repeats by, when the repetition words cannot say it: Repeats starts
   * empty, the rule is said beside it, and it is left alone unless something is typed there.
   */
  rule?: string;
}

/** What saving did: the sentence to say, and the series added, if one was. */
export interface Saved {
  said: string;
  series?: string;
}

interface Request {
  purpose: Purpose;
  initial: Fields;
  heading: string;
  answer: (saved?: Saved) => void;
}

let open: ((request: Request) => void) | undefined;

/** Runs the form; undefined if it was cancelled. */
export function blockForm(purpose: Purpose, initial: Fields, heading: string): Promise<Saved | undefined> {
  return new Promise((answer) => {
    if (open) open({ purpose, initial, heading, answer });
    else answer(undefined);
  });
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
  const { purpose, initial, heading } = props.request;
  const [fields, setFields] = useState(initial);
  const [date, setDate] = useState(purpose.kind === "add" ? purpose.date : "");
  const [problem, setProblem] = useState<string | undefined>();
  const set = (change: Partial<Fields>) => setFields({ ...fields, ...change });

  const save = async (event: FormEvent) => {
    event.preventDefault();
    const minutes = Number(fields.minutes.trim());
    if (!/^\s*\d+\s*$/.test(fields.minutes) || minutes <= 0) {
      setProblem("Minutes has to be a whole number.");
      return;
    }
    const changed = (now: string, was: string) => (now.trim() !== was ? now.trim() : undefined);
    try {
      let saved: Saved;
      if (purpose.kind === "add") {
        saved = await core.addBlock({
          title: fields.title.trim(),
          at: fields.start.trim(),
          minutes,
          date: date.trim() || undefined,
          kind: fields.kind,
          repeat: fields.repeat.trim() || undefined,
        });
      } else {
        const edit = {
          title: changed(fields.title, initial.title),
          at: changed(fields.start, initial.start),
          minutes: fields.minutes.trim() !== initial.minutes ? minutes : undefined,
          kind: fields.kind !== initial.kind ? fields.kind : undefined,
          // Emptied means it happens once: the core's word for that is "none". One day of a
          // series cannot repeat differently, so an occurrence never sends it.
          repeat:
            purpose.kind === "series" && fields.repeat.trim() !== initial.repeat ? fields.repeat.trim() || "none" : undefined,
        };
        const said =
          purpose.kind === "series"
            ? await core.editBlock(purpose.id, edit, "Series")
            : await core.editBlock(purpose.series, edit, { Occurrence: { date: purpose.date } });
        saved = { said };
      }
      props.finish(saved);
    } catch (error) {
      setProblem((error as Error).message);
    }
  };

  const field = (label: string, key: keyof Fields, description?: string, autoFocus = false) => (
    <TextField className="field" value={fields[key] ?? ""} onChange={(text) => set({ [key]: text })} autoFocus={autoFocus}>
      <Label>{label}</Label>
      <Input />
      {description && (
        <Text slot="description" className="quiet">
          {description}
        </Text>
      )}
    </TextField>
  );

  return (
    <Dialog>
      <Heading slot="title">{heading}</Heading>
      <form onSubmit={save}>
        {field("Title", "title", undefined, true)}
        {field("Starts at", "start", "Such as 9am, or 14:30.")}
        {field("Minutes", "minutes")}
        <Select className="field" value={fields.kind} onChange={(key) => set({ kind: String(key) })}>
          <Label>Kind</Label>
          <Button>
            <SelectValue />
          </Button>
          <Popover>
            <ListBox items={KINDS}>{(item) => <ListBoxItem id={item.id}>{item.name}</ListBoxItem>}</ListBox>
          </Popover>
        </Select>
        {purpose.kind !== "occurrence" &&
          field(
            "Repeats",
            "repeat",
            fields.rule && !initial.repeat
              ? `Such as every weekday. It repeats by the rule ${fields.rule}, which the repetition words cannot say; leave this empty to keep it.`
              : "Such as every weekday. Empty for once.",
          )}
        {purpose.kind === "add" && (
          <TextField className="field" value={date} onChange={setDate}>
            <Label>Starts on</Label>
            <Input />
            <Text slot="description" className="quiet">
              A date, such as today or next Monday.
            </Text>
          </TextField>
        )}
        {problem && (
          <p role="alert" className="problem">
            {problem}
          </p>
        )}
        <div className="buttons">
          <Button onPress={() => props.finish()}>Cancel</Button>
          <Button type="submit">Save</Button>
        </div>
      </form>
    </Dialog>
  );
}
