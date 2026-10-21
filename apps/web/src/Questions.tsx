// The question open, drawn: whether to go ahead, which of a few, a line of text, one of many.
//
// Which question is open, what it says and why an answer was refused are the app's (the screen's
// `asking`); a dialog here shows it while it is open and sends what is answered, closing when the
// app closes it. They are React Aria dialogs: focus moves in, Tab stays inside, Escape cancels,
// and focus goes back where it was when they close.
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
import type { Action, ShownAsking, ShownChoice } from "./core";
import { DayPicker } from "./Pickers";
import { send } from "./session";

/**
 * Which of `options` to go ahead with: an alert dialog, Cancel first and focused. `answer` hears
 * the position of the one pressed, or undefined for Cancel or Escape.
 */
export function ChooseDialog(props: {
  heading: string;
  detail: string;
  options: string[];
  answer: (index?: number) => void;
  isOpen?: boolean;
}) {
  return (
    <Modal isDismissable isOpen={props.isOpen ?? true} onOpenChange={(isOpen) => !isOpen && props.answer(undefined)}>
      <Dialog role="alertdialog" aria-describedby="prompt-detail">
        <Heading slot="title">{props.heading}</Heading>
        <p id="prompt-detail">{props.detail}</p>
        <div className="buttons">
          <Button autoFocus onPress={() => props.answer(undefined)}>
            Cancel
          </Button>
          {props.options.map((option, index) => (
            <Button key={option} onPress={() => props.answer(index)}>
              {option}
            </Button>
          ))}
        </div>
      </Dialog>
    </Modal>
  );
}

/** Where the question open is shown: rendered once, in the app. A form's question is the
 * client's own screen, which `form` opens. */
export function Questions(props: { asking: ShownAsking | null; form: (action: Action) => void }) {
  const asking = props.asking;
  const cancel = () => void send({ tag: "Cancel" });

  // A form of the client's own: opened, and the question let go.
  useEffect(() => {
    if (asking?.question.tag !== "Form") return;
    if (asking.action) props.form(asking.action);
    cancel();
  }, [asking?.question.tag, asking?.action?.kind, asking?.action?.target]);

  if (!asking || asking.question.tag === "Form" || asking.question.tag === "Immediate") return null;
  const question = asking.question;
  // A new question is a new dialog, so what was typed in the last one does not carry over.
  const key = JSON.stringify([asking.action?.kind, asking.action?.target, question]);

  if (question.tag === "Confirm" || question.tag === "Choose") {
    const answers = question.tag === "Confirm" ? [{ id: "yes", title: question.yes }] : question.answers;
    return (
      <ChooseDialog
        key={key}
        heading={question.title}
        detail={question.message}
        options={answers.map((answer) => answer.title)}
        answer={(index) => {
          const chosen = index === undefined ? undefined : answers[index];
          if (!chosen) cancel();
          else void send({ tag: "Answer", value0: question.tag === "Confirm" ? { tag: "Yes" } : { tag: "Picked", id: chosen.id, length: null } });
        }}
      />
    );
  }
  return (
    <Modal isDismissable isOpen onOpenChange={(isOpen) => !isOpen && cancel()}>
      {question.tag === "Text" ? (
        <TextQuestion
          key={key}
          heading={question.sentence || question.title}
          label={question.label}
          description={question.hint}
          initial={question.initial}
          yes={question.yes}
          refusal={asking.refusal ?? undefined}
          day={asking.day}
          cancel={cancel}
        />
      ) : (
        <PickQuestion key={key} heading={question.title} items={asking.lines ?? []} yes={question.yes ?? ""} cancel={cancel} />
      )}
    </Modal>
  );
}

function TextQuestion(props: {
  heading: string;
  label: string;
  description: string;
  initial: string;
  yes: string;
  refusal: string | undefined;
  /** When it asks for a day, the day a calendar beside the line opens on. */
  day?: string | null;
  cancel: () => void;
}) {
  const [text, setText] = useState(props.initial);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    void send({ tag: "Answer", value0: { tag: "Text", text } });
  };
  return (
    <Dialog>
      <Heading slot="title">{props.heading}</Heading>
      <form onSubmit={submit}>
        <TextField
          className="field"
          value={text}
          onChange={setText}
          isInvalid={props.refusal !== undefined}
          validationBehavior="aria"
          autoFocus
        >
          <Label>{props.label}</Label>
          <Input />
          {props.description && (
            <Text slot="description" className="quiet">
              {props.description}
            </Text>
          )}
          <FieldError>{props.refusal}</FieldError>
        </TextField>
        {props.day && <DayPicker line={text} from={props.day} onPick={setText} />}
        <div className="buttons">
          <Button onPress={props.cancel}>Cancel</Button>
          <Button type="submit">{props.yes}</Button>
        </div>
      </form>
    </Dialog>
  );
}

function PickQuestion(props: { heading: string; items: ShownChoice[]; yes: string; cancel: () => void }) {
  const [chosen, setChosen] = useState<Key | null>(null);
  const [problem, setProblem] = useState<string | undefined>();

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (chosen === null) {
      setProblem(props.items.length ? "Choose one first." : "There is nothing to choose.");
      return;
    }
    void send({ tag: "Answer", value0: { tag: "Picked", id: String(chosen), length: null } });
  };

  return (
    // Named by its heading outright: a heading given an id of its own is no longer the slot's,
    // and React Aria then found the dialog without a title.
    <Dialog aria-labelledby="pick-heading">
      <Heading slot="title" id="pick-heading">
        {props.heading}
      </Heading>
      <form onSubmit={submit}>
        <ComboBox
          aria-labelledby="pick-heading"
          className="field"
          items={props.items}
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
          <Input />
          <Text slot="description" className="quiet">
            {props.items.length === 1 ? "1 to choose from." : `${props.items.length} to choose from.`} Type part of one to
            narrow them.
          </Text>
          <FieldError>{problem}</FieldError>
          <Popover>
            <ListBox>
              {(item: ShownChoice) => (
                <ListBoxItem textValue={item.text} style={{ paddingInlineStart: `${0.5 + (item.depth ?? 0)}em` }}>
                  {item.reason ? (
                    <>
                      <Text slot="label">{item.text}</Text>
                      <Text slot="description" className="quiet choice-reason">
                        {item.reason}
                      </Text>
                    </>
                  ) : (
                    item.text
                  )}
                </ListBoxItem>
              )}
            </ListBox>
          </Popover>
        </ComboBox>
        <div className="buttons">
          <Button onPress={props.cancel}>Cancel</Button>
          <Button type="submit">{props.yes}</Button>
        </div>
      </form>
    </Dialog>
  );
}
