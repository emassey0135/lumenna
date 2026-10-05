// Asking things: whether to go ahead, which of a few, a line of text, one of many.
//
// Each is a function returning a promise, so an action reads in the order it happens —
// `if (await confirm(…))` — and one host, rendered once in the app, shows whichever question
// is open. They are React Aria dialogs: focus moves in, Tab stays inside, Escape cancels, and
// focus goes back where it was when they close.
//
// A question before something destructive is an alert dialog with Cancel focused first, so Enter or
// Escape straight away does nothing harmful — not the browser's confirm(), which is modal to
// the whole browser and which some screen readers read as a bare message.

import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  ComboBox,
  Dialog,
  FieldError,
  Heading,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Modal,
  Popover,
  Text,
  TextField,
} from "react-aria-components";
import type { Key } from "react-aria-components";

type Question =
  | { kind: "choose"; heading: string; detail: string; options: string[]; answer: (index?: number) => void }
  | {
      kind: "ask";
      heading: string;
      label: string;
      description: string;
      initial: string;
      check?: (text: string) => string | undefined | Promise<string | undefined>;
      answer: (text?: string) => void;
    }
  | { kind: "pick"; heading: string; label: string; items: { id: string; text: string }[]; answer: (id?: string) => void };

let open: ((question: Question) => void) | undefined;

function put<T>(question: (answer: (value?: T) => void) => Question): Promise<T | undefined> {
  return new Promise((resolve) => {
    if (open) open(question(resolve));
    else resolve(undefined);
  });
}

/** Which of `options` to go ahead with, or undefined for Cancel, which is focused first. */
export function choose(heading: string, detail: string, options: string[]): Promise<number | undefined> {
  return put((answer) => ({ kind: "choose", heading, detail, options, answer }));
}

/** Whether to go ahead with something destructive. */
export async function confirm(heading: string, detail: string, action: string): Promise<boolean> {
  return (await choose(heading, detail, [action])) === 0;
}

/**
 * A line of text, or undefined if cancelled. `check` says what is wrong with an answer, which
 * is shown at the field rather than closing the dialog.
 */
export function ask(
  heading: string,
  label: string,
  description: string,
  initial = "",
  check?: (text: string) => string | undefined | Promise<string | undefined>,
): Promise<string | undefined> {
  return put((answer) => ({ kind: "ask", heading, label, description, initial, check, answer }));
}

/** One of `items`, found by typing part of it, or undefined if cancelled. */
export function pick(heading: string, label: string, items: { id: string; text: string }[]): Promise<string | undefined> {
  return put((answer) => ({ kind: "pick", heading, label, items, answer }));
}

/** Minutes, or undefined if cancelled; `optional` lets an empty answer mean none (null). */
export async function askMinutes(heading: string, initial: string, optional: boolean): Promise<number | null | undefined> {
  const description = optional
    ? "Minutes, or empty for no planned length."
    : "The whole of this sitting, replacing what is logged.";
  const text = await ask(heading, "Minutes", description, initial, (text) => {
    if (optional && !text.trim()) return undefined;
    return /^\s*[1-9]\d*\s*$/.test(text) ? undefined : "That is not a number of minutes.";
  });
  if (text === undefined) return undefined;
  return text.trim() ? Number(text) : null;
}

/** Where the questions are shown: rendered once, in the app. */
export function Prompts() {
  const [question, setQuestion] = useState<Question | undefined>();

  useEffect(() => {
    open = setQuestion;
    return () => {
      open = undefined;
    };
  }, []);

  const close = () => setQuestion(undefined);
  const cancel = () => {
    question?.answer(undefined);
    close();
  };

  return (
    <Modal isDismissable isOpen={question !== undefined} onOpenChange={(isOpen) => !isOpen && cancel()}>
      {question?.kind === "choose" && (
        <Dialog role="alertdialog" aria-describedby="prompt-detail">
          <Heading slot="title">{question.heading}</Heading>
          <p id="prompt-detail">{question.detail}</p>
          <div className="buttons">
            {question.options.map((option, index) => (
              <Button
                key={option}
                onPress={() => {
                  question.answer(index);
                  close();
                }}
              >
                {option}
              </Button>
            ))}
            <Button autoFocus onPress={cancel}>
              Cancel
            </Button>
          </div>
        </Dialog>
      )}
      {question?.kind === "ask" && <Asking question={question} close={close} cancel={cancel} />}
      {question?.kind === "pick" && <Picking question={question} close={close} cancel={cancel} />}
    </Modal>
  );
}

function Asking(props: { question: Extract<Question, { kind: "ask" }>; close: () => void; cancel: () => void }) {
  const { question } = props;
  const [text, setText] = useState(question.initial);
  const [problem, setProblem] = useState<string | undefined>();

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const wrong = await question.check?.(text);
    setProblem(wrong);
    if (wrong) return;
    question.answer(text);
    props.close();
  };

  return (
    <Dialog>
      <Heading slot="title">{question.heading}</Heading>
      <form onSubmit={submit}>
        <TextField
          className="field"
          value={text}
          onChange={setText}
          isInvalid={problem !== undefined}
          validationBehavior="aria"
          autoFocus
        >
          <Label>{question.label}</Label>
          <Input />
          <Text slot="description" className="quiet">
            {question.description}
          </Text>
          <FieldError>{problem}</FieldError>
        </TextField>
        <div className="buttons">
          <Button onPress={props.cancel}>Cancel</Button>
          <Button type="submit">OK</Button>
        </div>
      </form>
    </Dialog>
  );
}

function Picking(props: { question: Extract<Question, { kind: "pick" }>; close: () => void; cancel: () => void }) {
  const { question } = props;
  const [chosen, setChosen] = useState<Key | null>(null);
  const [problem, setProblem] = useState<string | undefined>();

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (chosen === null) {
      setProblem(question.items.length ? "Choose one first." : "There is nothing to choose.");
      return;
    }
    question.answer(String(chosen));
    props.close();
  };

  return (
    <Dialog>
      <Heading slot="title">{question.heading}</Heading>
      <form onSubmit={submit}>
        <ComboBox
          className="field"
          items={question.items}
          selectedKey={chosen}
          onSelectionChange={(key) => {
            setChosen(key);
            setProblem(undefined);
          }}
          isInvalid={problem !== undefined}
          validationBehavior="aria"
          menuTrigger="focus"
          autoFocus
        >
          <Label>{question.label}</Label>
          <Input />
          <Text slot="description" className="quiet">
            {question.items.length === 1 ? "1 to choose from." : `${question.items.length} to choose from.`} Type
            part of one to narrow them.
          </Text>
          <FieldError>{problem}</FieldError>
          <Popover>
            <ListBox>{(item: { id: string; text: string }) => <ListBoxItem>{item.text}</ListBoxItem>}</ListBox>
          </Popover>
        </ComboBox>
        <div className="buttons">
          <Button onPress={props.cancel}>Cancel</Button>
          <Button type="submit">OK</Button>
        </div>
      </form>
    </Dialog>
  );
}
