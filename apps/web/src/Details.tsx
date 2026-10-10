// The selected task's details.
//
// A form of labelled fields. Saving sends only the fields that changed, worked out by the
// surface's `task_edit` against the task the form started from, so a concurrent edit to
// another field on another device is not reverted. The fields follow the store while nobody is
// editing them; typing not yet saved is kept when the store changes underneath.

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
import { perform } from "./actions";
import { core } from "./core";
import type { Action, Choice, FormField, TaskDetail, TaskFields } from "./core";
import { say } from "./say";

export interface DetailsHandle {
  /** Puts focus on the first field. */
  focus(): void;
}

function same(a: TaskFields, b: TaskFields): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export const Details = forwardRef<DetailsHandle, { id: string | undefined; revision: number; onChanged: () => void }>(
  function Details(props, ref) {
    const [task, setTask] = useState<TaskDetail | undefined>();
    const [original, setOriginal] = useState<TaskFields | undefined>();
    const [fields, setFields] = useState<TaskFields | undefined>();
    const [state, setState] = useState("");
    const [projects, setProjects] = useState<Choice[]>([]);
    const [form, setForm] = useState<FormField[]>([]);
    const [failure, setFailure] = useState<string | undefined>();
    const title = useRef<HTMLInputElement>(null);

    useImperativeHandle(ref, () => ({ focus: () => title.current?.focus() }));

    useEffect(() => {
      if (!props.id) {
        setTask(undefined);
        setFields(undefined);
        return;
      }
      // Another task, or the store changed: read it again — unless the person is part way
      // through editing this one, which would lose their typing.
      const editing = task?.id === props.id && fields && original && !same(fields, original);
      if (editing) return;
      let current = true;
      void Promise.all([core.task(props.id), core.projectOptions(), core.taskForm()]).then(
        ([shown, projects, form]) => {
          if (!current) return;
          setFailure(undefined);
          setTask(shown.task);
          setOriginal(shown.fields);
          setFields(shown.fields);
          setState(shown.state);
          setProjects(projects);
          setForm(form);
        },
        (error: Error) => {
          if (!current) return;
          setTask(undefined);
          setFailure(error.message);
        },
      );
      return () => {
        current = false;
      };
    }, [props.id, props.revision]);

    if (!task || !fields) {
      return <p className="quiet">{(props.id && failure) || "No task selected"}</p>;
    }

    const set = (change: Partial<TaskFields>) => setFields({ ...fields, ...change });

    const run = async (operation: Promise<string | undefined>, reread = true) => {
      try {
        const said = await operation;
        if (said === undefined) {
          say("Nothing changed");
          return;
        }
        if (reread) setOriginal(undefined);
        props.onChanged();
        say(said);
      } catch (error) {
        say((error as Error).message);
      }
    };

    // One of the task's own actions, as the core offers them: this screen is its form, so
    // Edit Details is not among them.
    const act = async (action: Action) => {
      const done = await perform(action);
      if (!done) return;
      props.onChanged();
      say(done.said);
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
                ...(projects.some((option) => option.id === fields.project)
                  ? []
                  : [{ id: fields.project, title: fields.project, depth: 0 }]),
              ]
            : (field.options ?? []).map((option) => ({
                id: key === "priority" ? Number(option.id) : option.id,
                title: option.title,
                depth: 0,
              }));
        return (
          <Select
            key={field.key}
            className="field"
            value={fields[key]}
            onChange={(chosen) => set({ [key]: key === "priority" ? Number(chosen) : String(chosen) })}
          >
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
        <TextField key={field.key} className="field" value={String(fields[key])} onChange={(text) => set({ [key]: text })}>
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
          void run(core.save(task, fields));
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
        <TextField className="field" value={state} isReadOnly>
          <Label>State</Label>
          <Input />
        </TextField>
        <div className="buttons">
          <Button type="submit">Save</Button>
          {(task.actions ?? []).map((action) => (
            <Button
              key={`${action.kind}:${action.other ?? ""}`}
              className={action.destructive ? "destructive" : undefined}
              onPress={() => void act(action)}
            >
              {action.sentence ?? action.title}
            </Button>
          ))}
        </div>
      </Form>
    );
  },
);
