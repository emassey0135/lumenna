// The details of the task in hand, as the app keeps them (crates/app).
//
// A form of labelled fields. What is typed is the app's draft: it follows the store while
// nothing is typed, keeps typing when the store changes underneath, and saving sends only the
// fields that changed against the task as it was when the typing began, so a concurrent edit to
// another field on another device is not reverted.

import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import {
  Button,
  Form,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Select,
  SelectValue,
  Text,
  TextArea,
  TextField,
} from "react-aria-components";
import { actOn } from "./actions";
import { useCommand } from "./commands";
import type { FormField, Screen, ShellEvent, TaskFields } from "./core";
import { FieldKind } from "./core";
import { send } from "./session";
import { words } from "./words";

export interface DetailsHandle {
  /** Puts focus on the first field. */
  focus(): void;
}

export const Details = forwardRef<DetailsHandle, { screen: Screen }>(function Details(props, ref) {
  const details = props.screen.details;
  const title = useRef<HTMLInputElement>(null);
  // What each field shows, set at once as typed so the caret stays put; the app's draft is the
  // same a moment later, and replaces it whenever it says otherwise (another task, a save, the
  // store moving while nothing is typed).
  const [fields, setFields] = useState<TaskFields | undefined>(details?.fields);
  // Typing sent and not yet answered: an answer for earlier typing would undo later typing.
  const pending = useRef(0);
  const shownTask = useRef<string | undefined>(undefined);
  useEffect(() => {
    const another = shownTask.current !== details?.task.id;
    shownTask.current = details?.task.id;
    if (another || pending.current === 0) setFields(details?.fields);
  }, [JSON.stringify(details?.fields), details?.task.id]);

  useImperativeHandle(ref, () => ({ focus: () => (title.current ?? undefined)?.focus() }));

  /** Sends `event`; what comes back is said as every turn is (session.ts). */
  const act = (event: ShellEvent) => void send(event);
  const save = () => void act({ tag: "Details", value0: { tag: "Save" } });

  // The Task menu's commands on the task shown, wherever it was chosen: a list's own come
  // first (TaskList), so focus lands as its row's action does. What each runs on the task, or
  // why it runs nothing, is the core's (`DetailsEvent::Key`).
  const onTask = (id: string) => () => {
    if (!details) return false;
    void act({ tag: "Details", value0: { tag: "Key", value0: id } });
  };
  useCommand("mark-done", onTask("mark-done"));
  useCommand("put-in-block", onTask("put-in-block"));
  useCommand("move-to-project", onTask("move-to-project"));
  useCommand("save-task", () => {
    if (!details) return false;
    save();
  });

  const said = words().details;
  if (!details || !fields) {
    return <p className="quiet">{said.none}</p>;
  }
  const { task, projects, form } = details;

  const set = (key: keyof TaskFields, value: string | number) => {
    setFields({ ...fields, [key]: value });
    pending.current++;
    void send({ tag: "Details", value0: { tag: "Field", key, value: String(value) } }).then((turn) => {
      // The last answer is the form as all the typing left it.
      if (--pending.current === 0 && turn.screen.details) setFields(turn.screen.details.fields);
    });
  };

  // Each field as the core describes it. The title is one line, so Enter saves; the project
  // is a choice of the projects the core offers, found by typing its name. An option has no
  // level of its own in ARIA, so one under another says its level in its name, after the
  // title, as the desktop apps' rows do; it is indented to be seen.
  const control = (field: FormField) => {
    const key = field.key as keyof TaskFields;
    const help = field.hint && (
      <Text slot="description" className="quiet">
        {field.hint}
      </Text>
    );
    if (field.kind === FieldKind.Choice) {
      const items: { id: string | number; title: string; depth: number }[] =
        key === "project"
          ? [
              ...projects.map((option) => ({ id: option.id, title: option.title, depth: option.depth ?? 0 })),
              // An archived project is offered to no task, but the one this task is in stays
              // its value until another is chosen.
              ...(projects.some((option) => option.id === fields.project) ? [] : [{ id: fields.project, title: fields.project, depth: 0 }]),
            ]
          : (field.options ?? []).map((option) => ({
              id: key === "priority" ? Number(option.id) : option.id,
              title: option.title,
              depth: 0,
            }));
      return (
        <Select key={field.key} className="field" value={fields[key]} onChange={(chosen) => set(key, key === "priority" ? Number(chosen) : String(chosen))}>
          <Label>{field.label}</Label>
          <Button>
            <SelectValue />
          </Button>
          {help}
          <Popover>
            <ListBox items={items}>
              {(item) => (
                <ListBoxItem
                  id={item.id}
                  textValue={item.title}
                  aria-label={item.depth > 0 ? `${item.title}, level ${item.depth + 1}` : undefined}
                  style={item.depth > 0 ? { paddingInlineStart: `${item.depth * 1.25 + 0.5}em` } : undefined}
                >
                  {item.title}
                </ListBoxItem>
              )}
            </ListBox>
          </Popover>
        </Select>
      );
    }
    const multiline = field.kind === FieldKind.Lines;
    return (
      <TextField key={field.key} className="field" value={String(fields[key])} onChange={(text) => set(key, text)}>
        <Label>{field.label}</Label>
        {multiline ? (
          <TextArea rows={4} placeholder={field.example || undefined} />
        ) : (
          <Input ref={key === "title" ? title : undefined} placeholder={field.example || undefined} />
        )}
        {help}
      </TextField>
    );
  };

  // Several lines take the width; the rest go two to a row.
  const groups: FormField[][] = [];
  for (const field of form) {
    const wide = field.kind === FieldKind.Lines;
    const last = groups[groups.length - 1];
    if (!wide && last && last[0].kind !== FieldKind.Lines) last.push(field);
    else groups.push([field]);
  }

  return (
    <Form
      aria-label={said.title}
      onSubmit={(event) => {
        event.preventDefault();
        save();
      }}
    >
      {groups.map((group) =>
        group.length === 1 && group[0].kind === FieldKind.Lines ? (
          control(group[0])
        ) : (
          <div key={group[0].key} className="pair">
            {group.map(control)}
          </div>
        ),
      )}
      <section aria-labelledby="waits-for" aria-describedby="waits-for-hint">
        <h3 id="waits-for">{said.waits}</h3>
        <p id="waits-for-hint" className="quiet">
          {said.waitsHint}
        </p>
        {task.depends.length === 0 ? (
          <p className="quiet">{said.waitsNone}</p>
        ) : (
          <ul className="plain">
            {task.depends.map((other) => (
              <li key={other.id}>{other.title}</li>
            ))}
          </ul>
        )}
      </section>
      {/* What the details list besides the form, in the core's words: its state, the rule it
          repeats by, its history, which is on its own screen only, never in a list. What it
          waits for is the list above. */}
      {details.lines
        .filter((line) => line.key !== "waits")
        .map((line) =>
          line.key === "state" ? (
            <TextField key={line.key} className="field" value={line.value} isReadOnly>
              <Label>{line.label}</Label>
              <Input />
            </TextField>
          ) : (
            <section key={line.key} aria-labelledby={`about-${line.key}`}>
              <h3 id={`about-${line.key}`}>{line.label}</h3>
              <p>{line.value}</p>
            </section>
          ),
        )}
      <div className="buttons">
        <Button type="submit">{said.save}</Button>
        {(task.offered ?? []).map((action) => (
          <Button
            key={`${action.kind}:${action.other ?? ""}`}
            className={action.destructive ? "react-aria-Button destructive" : "react-aria-Button"}
            onPress={() => void act(actOn({ tag: "Details" }, action))}
          >
            {action.sentence ?? action.title}
          </Button>
        ))}
      </div>
    </Form>
  );
});
