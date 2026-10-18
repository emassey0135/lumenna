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
import type { Setting, SettingView, SettingsView } from "./core";
import { sayAll, send } from "./session";
import { remindersOn, setRemindersOn } from "./Reminders";
import { say } from "./say";

export function Settings(props: {
  isOpen: boolean;
  onClose: () => void;
  revision: number;
  settings: SettingsView | undefined;
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
            <Planning settings={props.settings} />
          </TabPanel>
          <TabPanel id="devices">
            <DevicesPage revision={props.revision} syncing={props.syncing} onPaired={props.onPaired} />
          </TabPanel>
          <TabPanel id="backups">
            <Backups onChanged={props.onChanged} />
          </TabPanel>
          <TabPanel id="export">
            <Exports settings={props.settings} onChanged={props.onChanged} />
          </TabPanel>
        </Tabs>
        <div className="buttons">
          <Button onPress={props.onClose}>Close</Button>
        </div>
      </Dialog>
    </Modal>
  );
}

function Planning(props: { settings: SettingsView | undefined }) {
  const all = props.settings?.settings ?? [];
  const settings = all.filter((row) => row.setting.syncs);
  // Reminders on this device, which a browser keeps itself: the core's name and hint for it.
  const reminders = all.find((row) => row.setting.key === "reminders")?.setting;
  const [on, setOn] = useState(remindersOn());

  const set = async (key: string, value: string) => {
    const turn = await send({ Settings: { Set: { key, value } } });
    sayAll(turn.said);
  };

  const control = ({ setting, typed, problem }: SettingView) => {
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
    const description = hint || (setting.kind === "time" ? "A time, such as 8:00 or 22:30." : "");
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
  return (
    <TextField
      className="field"
      value={text}
      isInvalid={props.problem !== undefined && props.problem !== null}
      validationBehavior="aria"
      onChange={(value) => {
        setText(value);
        void send({ Settings: { Type: { key: setting.key, value } } });
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
          const turn = await send({ Settings: { Import: { name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) } } });
          sayAll(turn.said);
          props.onChanged();
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
          onPress={async () => sayAll((await send({ Settings: "Backup" })).said)}
        >
          Download a backup
        </Button>
        <ReadFile label="Restore from a backup" accept=".lumbak" onChanged={props.onChanged} />
      </div>
      <p className="quiet">Restoring merges: it adds what this browser lacks, and never takes back a later change.</p>
    </>
  );
}

function Exports(props: { settings: SettingsView | undefined; onChanged: () => void }) {
  const choices = props.settings?.exports ?? [];
  return (
    <>
      <p>An export is what is there now: no history, and nothing from the trash.</p>
      <div className="buttons stacked">
        {choices.map((choice) => (
          <Button key={choice.format} onPress={async () => sayAll((await send({ Settings: { Export: choice.format } })).said)}>
            {choice.label}
          </Button>
        ))}
        <ReadFile label="Import a JSON export or a backup" accept=".json,.lumbak" onChanged={props.onChanged} />
      </div>
    </>
  );
}
