// Settings, as the desktop apps have them: Planning, Backups, and Export and Import —
// each a tab. A setting applies as it is made (a text field when it is left), and says so.
//
// A browser has no folder to back up into and nothing running to do it on a schedule, so a
// backup here is a download, asked for; restoring and importing read a file the person chooses.

import { useEffect, useRef, useState } from "react";
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
  Tab,
  TabList,
  TabPanel,
  Tabs,
  Text,
  TextField,
} from "react-aria-components";
import { core } from "./core";
import type { ExportChoice } from "./core";
import { say } from "./say";

const VERBOSITIES = [
  { id: "full", name: "Full sentences" },
  { id: "terse", name: "Terse" },
];

const WEEKDAYS = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"].map((day) => ({
  id: day,
  name: day[0].toUpperCase() + day.slice(1),
}));

const TIMES = [
  { key: "day-start", label: "Day starts" },
  { key: "day-end", label: "Day ends" },
  { key: "all-day-reminder-hour", label: "All-day reminders at" },
];

/** Offers `data` as a file to save, called `name`. */
function download(name: string, data: BlobPart, type: string) {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export function Settings(props: { isOpen: boolean; onClose: () => void; revision: number; onChanged: () => void }) {
  return (
    <Modal isDismissable isOpen={props.isOpen} onOpenChange={(open) => !open && props.onClose()}>
      <Dialog>
        <Heading slot="title">Settings</Heading>
        <Tabs>
          <TabList aria-label="Settings pages">
            <Tab id="planning">Planning</Tab>
            <Tab id="backups">Backups</Tab>
            <Tab id="export">Export and Import</Tab>
          </TabList>
          <TabPanel id="planning">
            <Planning revision={props.revision} />
          </TabPanel>
          <TabPanel id="backups">
            <Backups onChanged={props.onChanged} />
          </TabPanel>
          <TabPanel id="export">
            <Exports onChanged={props.onChanged} />
          </TabPanel>
        </Tabs>
        <div className="buttons">
          <Button onPress={props.onClose}>Close</Button>
        </div>
      </Dialog>
    </Modal>
  );
}

function Planning(props: { revision: number }) {
  const [known, setKnown] = useState<Record<string, string>>({});
  const [times, setTimes] = useState<Record<string, string>>({});
  const [reload, setReload] = useState(0);

  useEffect(() => {
    void core.settings().then((settings) => {
      setKnown(settings);
      setTimes(Object.fromEntries(TIMES.map(({ key }) => [key, settings[key] ?? ""])));
    });
  }, [props.revision, reload]);

  const set = async (key: string, value: string) => {
    try {
      say(await core.setSetting(key, value));
    } catch (error) {
      say((error as Error).message);
    }
    setReload((r) => r + 1);
  };

  return (
    <>
      <Checkbox
        className="check"
        isSelected={known["cascade-complete-subtasks"] === "true"}
        onChange={(on) => void set("cascade-complete-subtasks", on ? "true" : "false")}
      >
        <span className="box" aria-hidden="true" />
        Completing a task completes its subtasks
      </Checkbox>
      {TIMES.map(({ key, label }) => (
        <TextField
          key={key}
          className="field"
          value={times[key] ?? ""}
          onChange={(value) => setTimes((now) => ({ ...now, [key]: value }))}
          // Applied when the field is left, as the desktop apps' are.
          onBlur={() => {
            if ((times[key] ?? "") !== (known[key] ?? "")) void set(key, times[key] ?? "");
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (times[key] ?? "") !== (known[key] ?? "")) void set(key, times[key] ?? "");
          }}
        >
          <Label>{label}</Label>
          <Input />
          <Text slot="description" className="quiet">
            A time, such as 8:00 or 22:30.
          </Text>
        </TextField>
      ))}
      <Select className="field" value={known.verbosity ?? "full"} onChange={(key) => void set("verbosity", String(key))}>
        <Label>Announcements</Label>
        <Button>
          <SelectValue />
        </Button>
        <Popover>
          <ListBox items={VERBOSITIES}>{(item) => <ListBoxItem id={item.id}>{item.name}</ListBoxItem>}</ListBox>
        </Popover>
      </Select>
      <Select className="field" value={known["week-start"] ?? "monday"} onChange={(key) => void set("week-start", String(key))}>
        <Label>Week starts on</Label>
        <Button>
          <SelectValue />
        </Button>
        <Popover>
          <ListBox items={WEEKDAYS}>{(item) => <ListBoxItem id={item.id}>{item.name}</ListBoxItem>}</ListBox>
        </Popover>
      </Select>
      <p className="quiet">These sync to all your devices.</p>
    </>
  );
}

/** A button that reads a file the person chooses, and says what reading it did. */
function ReadFile(props: { label: string; accept: string; onChanged: () => void }) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <>
      <Button onPress={() => input.current?.click()}>{props.label}</Button>
      <input
        ref={input}
        type="file"
        accept={props.accept}
        hidden
        aria-hidden="true"
        tabIndex={-1}
        onChange={async (event) => {
          const file = event.target.files?.[0];
          event.target.value = "";
          if (!file) return;
          try {
            say(await core.importFile(file.name, new Uint8Array(await file.arrayBuffer())));
            props.onChanged();
          } catch (error) {
            say((error as Error).message);
          }
        }}
      />
    </>
  );
}

function Backups(props: { onChanged: () => void }) {
  return (
    <>
      <p>
        A backup is everything, its whole history and the trash included. This browser takes none by itself:
        download one now and then, and keep it somewhere safe.
      </p>
      <div className="buttons">
        <Button
          onPress={async () => {
            try {
              const file = await core.backup();
              download(file.name, file.bytes as Uint8Array<ArrayBuffer>, "application/octet-stream");
              say(file.said);
            } catch (error) {
              say((error as Error).message);
            }
          }}
        >
          Download a Backup
        </Button>
        <ReadFile label="Restore from a Backup…" accept=".lumbak" onChanged={props.onChanged} />
      </div>
      <p className="quiet">Restoring merges: it adds what this browser lacks, and never takes back a later change.</p>
    </>
  );
}

function Exports(props: { onChanged: () => void }) {
  const [choices, setChoices] = useState<ExportChoice[]>([]);

  useEffect(() => {
    void core.exportChoices().then(setChoices);
  }, []);

  return (
    <>
      <p>An export is what is there now: no history, and nothing from the trash.</p>
      <div className="buttons stacked">
        {choices.map((choice) => (
          <Button
            key={choice.format}
            onPress={async () => {
              try {
                download(choice.name, await core.exportContent(choice.format), "text/plain;charset=utf-8");
                say(`Exported as ${choice.name}`);
              } catch (error) {
                say((error as Error).message);
              }
            }}
          >
            {choice.label}
          </Button>
        ))}
        <ReadFile label="Import a JSON Export or a Backup…" accept=".json,.lumbak" onChanged={props.onChanged} />
      </div>
    </>
  );
}
