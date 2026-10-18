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
import { byKind } from "./actions";
import { useCommand } from "./commands";
import type { ActionKind, Event, FormField, Screen, TaskFields } from "./core";
import { run, sayAll, send } from "./session";

export interface DetailsHandle {
  /** Puts focus on the first field. */
  focus(): void;
}

export const Details = forwardRef<DetailsHandle, { screen: Screen; onChanged: () => void }>(function Details(props, ref) {
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

  /** Runs `event` through the app from here, and says what it did. */
  const act = async (event: Event) => {
    const turn = await run(event);
    sayAll(turn.said);
    props.onChanged();
  };
  const save = () => void act({ Details: "Save" });

  // The Task menu's commands on the task shown, wherever it was chosen: a list's own come
  // first (TaskList), so focus lands as its row's action does.
  const onTask = (...kinds: ActionKind[]) => () => {
    const action = details && byKind(details.task.actions, ...kinds);
    if (!action) return false;
    void act({ Perform: action });
  };
  useCommand("mark-done", onTask("mark_done", "mark_not_done"));
  useCommand("put-in-block", onTask("put_in_block"));
  useCommand("move-to-project", onTask("move_to_project"));
  useCommand("save-task", () => {
    if (!details) return false;
    save();
  });

  if (!details || !fields) {
    return <p className="quiet">No task selected</p>;
  }
  const { task, projects, form } = details;

  const set = (key: keyof TaskFields, value: string | number) => {
    setFields({ ...fields, [key]: value });
    pending.current++;
    void send({ Details: { Field: { key, value: String(value) } } }).then((turn) => {
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
    if (field.kind === "choice") {
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
    const multiline = field.kind === "lines";
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
    const wide = field.kind === "lines";
    const last = groups[groups.length - 1];
    if (!wide && last && last[0].kind !== "lines") last.push(field);
    else groups.push([field]);
  }

  return (
    <Form
      aria-label="Task details"
      onSubmit={(event) => {
        event.preventDefault();
        save();
      }}
    >
      {groups.map((group) =>
        group.length === 1 && group[0].kind === "lines" ? (
          control(group[0])
        ) : (
          <div key={group[0].key} className="pair">
            {group.map(control)}
          </div>
        ),
      )}
      <section aria-labelledby="waits-for">
        <h3 id="waits-for">Waits for</h3>
        {task.depends.length === 0 ? (
          <p className="quiet">Nothing.</p>
        ) : (
          <ul className="plain">
            {task.depends.map((other) => (
              <li key={other.id}>{other.title}</li>
            ))}
          </ul>
        )}
      </section>
      {/* What has happened to it: on its own screen only, never in a list. */}
      {(task.history ?? []).length > 0 && (
        <section aria-labelledby="task-history">
          <h3 id="task-history">History</h3>
          <ul className="plain">
            {(task.history ?? []).map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      )}
      <TextField className="field" value={details.state} isReadOnly>
        <Label>State</Label>
        <Input />
      </TextField>
      <div className="buttons">
        <Button type="submit">Save</Button>
        {(task.actions ?? []).map((action) => (
          <Button
            key={`${action.kind}:${action.other ?? ""}`}
            className={action.destructive ? "react-aria-Button destructive" : "react-aria-Button"}
            onPress={() => void act({ Perform: action })}
          >
            {action.sentence ?? action.title}
          </Button>
        ))}
      </div>
    </Form>
  );
});
