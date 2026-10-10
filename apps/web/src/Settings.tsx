// Settings, as the desktop apps have them: Planning, Devices, Backups, and Export and Import —
// each a tab. A setting applies as it is made (a text field when it is left), and says so.
//
// Each setting describes itself (`Setting`): its name, its control, what it can be and what
// it does. Planning lists those that sync; the rest are a device's own — where its backups go,
// how often, its clock — which a browser does not have: it backs up by download and words
// times as its locale does.
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
import { DevicesPage } from "./Devices";
import type { ExportChoice, Setting } from "./core";
import { say } from "./say";

/** Offers `data` as a file to save, called `name`. */
function download(name: string, data: BlobPart, type: string) {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export function Settings(props: {
  isOpen: boolean;
  onClose: () => void;
  revision: number;
  onChanged: () => void;
  /** Whether this browser is keeping in sync. */
  syncing: boolean;
  /** A device was paired: syncing may need to start. */
  onPaired: () => void;
}) {
  return (
    <Modal isDismissable isOpen={props.isOpen} onOpenChange={(open) => !open && props.onClose()}>
      <Dialog>
        <Heading slot="title">Settings</Heading>
        <Tabs>
          <TabList aria-label="Settings pages">
            <Tab id="planning">Planning</Tab>
            <Tab id="devices">Devices</Tab>
            <Tab id="backups">Backups</Tab>
            <Tab id="export">Export and import</Tab>
          </TabList>
          <TabPanel id="planning">
            <Planning revision={props.revision} />
          </TabPanel>
          <TabPanel id="devices">
            <DevicesPage revision={props.revision} syncing={props.syncing} onPaired={props.onPaired} />
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
  const [settings, setSettings] = useState<Setting[]>([]);
  // What is typed in a text field and not yet applied, by key.
  const [typed, setTyped] = useState<Record<string, string>>({});
  const [reload, setReload] = useState(0);

  useEffect(() => {
    void core.settings().then((all) => {
      const shown = all.filter((setting) => setting.syncs);
      setSettings(shown);
      setTyped(Object.fromEntries(shown.map((setting) => [setting.key, setting.value])));
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

  const control = (setting: Setting) => {
    const { key, title, value, hint } = setting;
    const options = setting.options ?? [];
    if (setting.kind === "toggle") {
      const on = options[0]?.id ?? "true";
      const off = options[1]?.id ?? "false";
      return (
        <div key={key}>
          <Checkbox className="check" isSelected={value === on} onChange={(checked) => void set(key, checked ? on : off)}>
            <span className="box" aria-hidden="true" />
            {title}
          </Checkbox>
          {hint && <p className="quiet">{hint}</p>}
        </div>
      );
    }
    if (setting.kind === "choice") {
      // A value set elsewhere that is not among the options is still the value.
      const items = options.some((o) => o.id === value) ? options : [...options, { id: value, title: value, depth: 0 }];
      return (
        <Select key={key} className="field" value={value} onChange={(chosen) => void set(key, String(chosen))}>
          <Label>{title}</Label>
          <Button>
            <SelectValue />
          </Button>
          {hint && (
            <Text slot="description" className="quiet">
              {hint}
            </Text>
          )}
          <Popover>
            <ListBox items={items}>{(item) => <ListBoxItem id={item.id}>{item.title}</ListBoxItem>}</ListBox>
          </Popover>
        </Select>
      );
    }
    const changed = () => (typed[key] ?? "") !== value;
    const description = hint || (setting.kind === "time" ? "A time, such as 8:00 or 22:30." : "");
    return (
      <TextField
        key={key}
        className="field"
        value={typed[key] ?? ""}
        onChange={(text) => setTyped((now) => ({ ...now, [key]: text }))}
        // Applied when the field is left, as the desktop apps' are.
        onBlur={() => {
          if (changed()) void set(key, typed[key] ?? "");
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" && changed()) void set(key, typed[key] ?? "");
        }}
      >
        <Label>{title}</Label>
        <Input />
        {description && (
          <Text slot="description" className="quiet">
            {description}
          </Text>
        )}
      </TextField>
    );
  };

  return (
    <>
      {settings.map(control)}
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
          Download a backup
        </Button>
        <ReadFile label="Restore from a backup…" accept=".lumbak" onChanged={props.onChanged} />
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
        <ReadFile label="Import a JSON export or a backup…" accept=".json,.lumbak" onChanged={props.onChanged} />
      </div>
    </>
  );
}
