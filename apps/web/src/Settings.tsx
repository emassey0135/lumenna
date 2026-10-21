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
  FieldError,
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
import { DevicesPage } from "./Devices";
import type { Screen, Setting, SettingView, SettingsView } from "./core";
import { SettingKind } from "./core";
import { TimePicker } from "./Pickers";
import { send } from "./session";
import { remindersOn, setRemindersOn } from "./Reminders";
import { say } from "./say";

export function Settings(props: {
  isOpen: boolean;
  onClose: () => void;
  settings: SettingsView | null;
  /** The Devices page, as the app has it. */
  devices: Screen["devices"];
  /** The devices list's row in hand. */
  held: Screen["devicesHeld"];
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
            <Planning settings={props.settings} />
          </TabPanel>
          <TabPanel id="devices">
            <DevicesPage devices={props.devices} held={props.held} />
          </TabPanel>
          <TabPanel id="backups">
            <Backups />
          </TabPanel>
          <TabPanel id="export">
            <Exports settings={props.settings} />
          </TabPanel>
        </Tabs>
        <div className="buttons">
          <Button onPress={props.onClose}>Close</Button>
        </div>
      </Dialog>
    </Modal>
  );
}

function Planning(props: { settings: SettingsView | null }) {
  const all = props.settings?.settings ?? [];
  const settings = all.filter((row) => row.setting.syncs);
  // Reminders on this device, which a browser keeps itself: the core's name and hint for it.
  const reminders = all.find((row) => row.setting.key === "reminders")?.setting;
  const [on, setOn] = useState(remindersOn());

  const set = async (key: string, value: string) => {
    await send({ tag: "Settings", value0: { tag: "Set", key, value } });
  };

  const control = ({ setting, typed, problem }: SettingView) => {
    const { key, title, value, hint } = setting;
    const options = setting.options ?? [];
    if (setting.kind === SettingKind.Toggle) {
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
    if (setting.kind === SettingKind.Choice) {
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
    const description = hint || (setting.kind === SettingKind.Time ? "A time, such as 8:00 or 22:30." : "");
    return <Line key={key} setting={setting} typed={typed} problem={problem} description={description} apply={set} />;
  };

  return (
    <>
      {settings.map(control)}
      <p className="quiet">These sync to all your devices.</p>
      {reminders && (
        <div>
          <Checkbox
            className="check"
            isSelected={on}
            onChange={(checked) => {
              setRemindersOn(checked);
              setOn(checked);
              say(checked ? "Reminders are on in this browser" : "Reminders are off in this browser");
            }}
          >
            <span className="box" aria-hidden="true" />
            {reminders.title}
          </Checkbox>
          {reminders.hint && <p className="quiet">{reminders.hint} Reminders are given while Lumenna is open in this tab.</p>}
        </div>
      )}
    </>
  );
}

/** A setting that is a line: applied when it is left, as the desktop apps' are. */
function Line(props: { setting: Setting; typed: string; problem?: string | null; description: string; apply: (key: string, value: string) => Promise<void> }) {
  const { setting } = props;
  // What the field shows, set at once as typed; the app keeps it too, and a refused line stays.
  const [text, setText] = useState(props.typed);
  useEffect(() => setText(props.typed), [props.typed]);
  const changed = () => text !== setting.value;
  const line = (
    <TextField
      className="field"
      value={text}
      isInvalid={props.problem !== undefined && props.problem !== null}
      validationBehavior="aria"
      onChange={(value) => {
        setText(value);
        void send({ tag: "Settings", value0: { tag: "Type", key: setting.key, value } });
      }}
      onBlur={() => {
        if (changed()) void props.apply(setting.key, text);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter" && changed()) void props.apply(setting.key, text);
      }}
    >
      <Label>{setting.title}</Label>
      <Input />
      {props.description && (
        <Text slot="description" className="quiet">
          {props.description}
        </Text>
      )}
      <FieldError>{props.problem}</FieldError>
    </TextField>
  );
  if (setting.kind !== SettingKind.Time) return line;
  // A time has the browser's clock beside its line; a time picked there is applied at once. The
  // two are a group named for the setting, so each clock is told from the others.
  return (
    <div role="group" aria-label={setting.title}>
      {line}
      <TimePicker
        line={text}
        onPick={(time) => {
          setText(time);
          void props.apply(setting.key, time);
        }}
      />
    </div>
  );
}

/** A button that reads a file the person chooses, and says what reading it did. */
function ReadFile(props: { label: string; accept: string }) {
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
          void send({ tag: "Settings", value0: { tag: "Import", name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) } });
        }}
      />
    </>
  );
}

function Backups() {
  return (
    <>
      <p>
        A backup is everything, its whole history and the trash included. This browser takes none by itself:
        download one now and then, and keep it somewhere safe.
      </p>
      <div className="buttons">
        <Button
          onPress={() => void send({ tag: "Settings", value0: { tag: "Backup" } })}
        >
          Download a backup
        </Button>
        <ReadFile label="Restore from a backup" accept=".lumbak" />
      </div>
      <p className="quiet">Restoring merges: it adds what this browser lacks, and never takes back a later change.</p>
    </>
  );
}

function Exports(props: { settings: SettingsView | null }) {
  const choices = props.settings?.exports ?? [];
  return (
    <>
      <p>An export is what is there now: no history, and nothing from the trash.</p>
      <div className="buttons stacked">
        {choices.map((choice) => (
          <Button key={choice.format} onPress={() => void send({ tag: "Settings", value0: { tag: "Export", value0: choice.format } })}>
            {choice.label}
          </Button>
        ))}
        <ReadFile label="Import a JSON export or a backup" accept=".json,.lumbak" />
      </div>
    </>
  );
}
