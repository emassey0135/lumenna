// The selected task's details (§16.1: task detail / edit).
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
import { askMinutes, confirm, pick } from "./Prompts";
import { core } from "./core";
import type { TaskDetail, TaskFields } from "./core";
import { say } from "./say";

const PRIORITIES = [
  { id: 1, name: "Priority 1, highest" },
  { id: 2, name: "Priority 2" },
  { id: 3, name: "Priority 3" },
  { id: 4, name: "Priority 4, none" },
];

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
    const [projects, setProjects] = useState<string[]>([]);
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
      void Promise.all([core.task(props.id), core.projects()]).then(
        ([shown, projects]) => {
          if (!current) return;
          setFailure(undefined);
          setTask(shown.task);
          setOriginal(shown.fields);
          setFields(shown.fields);
          setState(shown.state);
          setProjects(projects);
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
    const done = task.state.includes("completed");
    const trashed = task.state.includes("deleted");

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

    // Puts the task into a work block of today or the next six days (§3.7), asking how long
    // the sitting is meant to take. The day reaches any other day, from the block's side.
    const putInBlock = async (task: TaskDetail) => {
      const blocks = await core.workBlocks();
      if (blocks.length === 0) {
        say("There are no work blocks this week. Add one from Today.");
        return;
      }
      const block = await pick(`Put ${task.title} in a Block`, "Block", blocks);
      const chosen = blocks.find((b) => b.id === block);
      if (!chosen) return;
      const minutes = await askMinutes(`How Long Is ${task.title} Meant to Take?`, "", true);
      if (minutes === undefined) return;
      void run(core.assign(task.id, chosen.id, chosen.date, minutes ?? undefined));
    };

    const field = (label: string, value: string, key: keyof TaskFields, description?: string) => (
      <TextField className="field" value={value} onChange={(text) => set({ [key]: text })}>
        <Label>{label}</Label>
        <Input ref={key === "title" ? title : undefined} />
        {description && (
          <Text slot="description" className="quiet">
            {description}
          </Text>
        )}
      </TextField>
    );

    return (
      <Form
        aria-label="Task details"
        onSubmit={(event) => {
          event.preventDefault();
          void run(core.save(task, fields));
        }}
      >
        {field("Title", fields.title, "title")}
        <div className="pair">
          {field("Due", fields.due, "due", "A date, such as tomorrow or next Friday. Empty for none.")}
          {field("Repeats", fields.repeat, "repeat", "Such as every Monday. Empty for no repetition.")}
        </div>
        <div className="pair">
          <Select
            className="field"
            value={fields.priority}
            onChange={(key) => set({ priority: Number(key) })}
          >
            <Label>Priority</Label>
            <Button>
              <SelectValue />
            </Button>
            <Popover>
              <ListBox items={PRIORITIES}>{(item) => <ListBoxItem id={item.id}>{item.name}</ListBoxItem>}</ListBox>
            </Popover>
          </Select>
          {field("Estimate", fields.estimate, "estimate", "Such as 45m or 1h30m. Empty for none.")}
        </div>
        <div className="pair">
          <Select className="field" value={fields.project} onChange={(key) => set({ project: String(key) })}>
            <Label>Project</Label>
            <Button>
              <SelectValue />
            </Button>
            <Popover>
              <ListBox items={projects.map((name) => ({ id: name, name }))}>
                {(item) => <ListBoxItem id={item.id}>{item.name}</ListBoxItem>}
              </ListBox>
            </Popover>
          </Select>
          {field("Labels", fields.labels, "labels", "Names separated by commas.")}
        </div>
        <TextField className="field" value={fields.notes} onChange={(notes) => set({ notes })}>
          <Label>Notes</Label>
          <TextArea rows={4} />
        </TextField>
        <TextField className="field" value={state} isReadOnly>
          <Label>State</Label>
          <Input />
        </TextField>
        <div className="buttons">
          <Button type="submit">Save</Button>
          {trashed ? (
            <>
              <Button onPress={() => void run(core.restore(task.id))}>Restore</Button>
              <Button
                onPress={async () => {
                  // Erasing cannot be undone (§9), so it asks.
                  const detail = "It and its history are deleted for good. This cannot be undone.";
                  if (await confirm(`Erase ${task.title}?`, detail, "Erase")) void run(core.erase(task.id));
                }}
              >
                Erase for Good…
              </Button>
            </>
          ) : (
            <>
              <Button onPress={() => void run(core.complete(task.id, done))}>{done ? "Mark Not Done" : "Mark Done"}</Button>
              <Button onPress={() => void putInBlock(task)}>Put in a Block…</Button>
              <Button onPress={() => void run(core.trash(task.id))}>Move to Trash</Button>
            </>
          )}
        </div>
      </Form>
    );
  },
);
