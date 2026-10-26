// Settings, as the desktop apps have them: the core's pages (`SettingsView::pages`), each a tab,
// in its order and with its titles, captions and buttons. A setting applies as it is made (a
// text field when it is left), and says so.
//
// Each setting describes itself (`Setting`): its name, its control, what it can be and what
// it does. A page lists the settings the store holds here: a device's own clock and backup
// folder a browser does not have, since it words times as its locale does and backs up by
// download, so a page with nothing else on it is left out.
//
// A browser has no folder to back up into and nothing running to do it on a schedule, so a
// backup here is a download, asked for; restoring and importing read a file the person chooses.
//
// What is done to the store as a whole (`SettingsView::offered`: Erase Everything Deleted) is a
// button each beside the backups, which hold what it erases, the core's caption beneath and
// describing them: handed back to the app, which asks its question as any row's.

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
import { actOn } from "./actions";
import { DevicesPage } from "./Devices";
import type { Screen, Setting, SettingView, SettingsPage, SettingsView } from "./core";
import { core, SettingKind } from "./core";
import * as folders from "./backups";
import { TimePicker } from "./Pickers";
import { send } from "./session";
import { remindersOn, setRemindersOn } from "./Reminders";
import { say } from "./say";
import { commandTitle } from "./words";

export function Settings(props: {
  isOpen: boolean;
  onClose: () => void;
  settings: SettingsView | null;
  /** The Devices page, as the app has it. */
  devices: Screen["devices"];
  /** The devices list's row in hand. */
  held: Screen["devicesHeld"];
}) {
  // The core's pages, in its order, each setting on one; a page with nothing a browser has (the
  // device's own clock and backup folder) is left out.
  const pages = (props.settings?.pages ?? []).filter((page) => SHOWN.has(page.id) || present(props.settings, page).length > 0);
  return (
    <Modal isDismissable isOpen={props.isOpen} onOpenChange={(open) => !open && props.onClose()}>
      <Dialog>
        <Heading slot="title">{commandTitle("settings")}</Heading>
        <Tabs>
          <TabList aria-label="Settings pages">
            {pages.map((page) => (
              <Tab key={page.id} id={page.id}>
                {page.title}
              </Tab>
            ))}
          </TabList>
          {pages.map((page) => (
            <TabPanel key={page.id} id={page.id}>
              <Page page={page} settings={props.settings} devices={props.devices} held={props.held} />
            </TabPanel>
          ))}
        </Tabs>
        <div className="buttons">
          <Button onPress={props.onClose}>Close</Button>
        </div>
      </Dialog>
    </Modal>
  );
}

/** The pages a browser shows whatever settings they hold: what is on them is the page's own. */
const SHOWN = new Set(["devices", "backups", "export"]);

/** The settings of `page` a browser has, in the page's order: those that sync, and reminders on
 * this device, which it keeps itself. A device's own others (where backups go, how often) are a
 * device's with a file system. */
function present(settings: SettingsView | null, page: SettingsPage): SettingView[] {
  const all = (settings?.settings ?? []).filter((row) => row.setting.syncs || row.setting.key === "reminders");
  return page.keys.flatMap((key) => all.filter((row) => row.setting.key === key));
}

/** A page's own button, by its id: its name is the core's. */
function button(page: SettingsPage, id: string): string {
  return page.buttons.find((b) => b.id === id)?.title ?? id;
}

function Page(props: { page: SettingsPage; settings: SettingsView | null; devices: Screen["devices"]; held: Screen["devicesHeld"] }) {
  const { page } = props;
  const rows = present(props.settings, page);
  return (
    <>
      {page.id === "devices" && <DevicesPage devices={props.devices} held={props.held} />}
      {page.id === "backups" && <Backups page={page} settings={props.settings} />}
      {page.id === "export" && <Exports page={page} settings={props.settings} />}
      {rows.length > 0 && <SettingRows rows={rows} settings={props.settings} />}
      {/* What the page says of its settings, beneath them: the export page's is of its buttons. */}
      {rows.length > 0 && page.caption && <p className="quiet">{page.caption}</p>}
    </>
  );
}

/** Settings, each a control as the core describes it. */
function SettingRows(props: { rows: SettingView[]; settings: SettingsView | null }) {
  const [on, setOn] = useState(remindersOn());

  const set = async (key: string, value: string) => {
    await send({ tag: "Settings", value0: { tag: "Set", key, value } });
  };

  const control = ({ setting, typed, problem }: SettingView) => {
    const { key, title, value, hint } = setting;
    const options = setting.options ?? [];
    // Reminders on this device, which a browser keeps itself: the core's name and hint for it.
    if (key === "reminders") {
      return (
        <div key={key}>
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
            {title}
          </Checkbox>
          {hint && <p className="quiet">{hint} Reminders are given while Lumenna is open in this tab.</p>}
        </div>
      );
    }
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
    const description = hint || (setting.kind === SettingKind.Time ? (props.settings?.words.timeHint ?? "") : "");
    return <Line key={key} setting={setting} typed={typed} problem={problem} description={description} apply={set} />;
  };

  return <>{props.rows.map(control)}</>;
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

function Backups(props: { page: SettingsPage; settings: SettingsView | null }) {
  return (
    <>
      <p>
        A backup is everything, its whole history and the trash included.{" "}
        {folders.supported
          ? "Choose a folder below and this browser takes them by itself; or download one now and then, and keep it somewhere safe."
          : "This browser takes none by itself: download one now and then, and keep it somewhere safe."}
      </p>
      {folders.supported && <BackupFolder />}
      <div className="buttons">
        {/* Backing up now is a download here: a browser has no folder of its own. */}
        <Button onPress={() => void send({ tag: "Settings", value0: { tag: "Backup" } })}>{button(props.page, "back-up")}</Button>
        <ReadFile label={button(props.page, "restore-backup")} accept=".lumbak" />
      </div>
      <p className="quiet">Restoring merges: it adds what this browser lacks, and never takes back a later change.</p>
      <div className="buttons">
        {(props.settings?.offered ?? []).map((action) => (
          <Button
            key={action.kind}
            className={action.destructive ? "react-aria-Button destructive" : "react-aria-Button"}
            aria-describedby={props.settings?.offeredCaption ? "store-actions-caption" : undefined}
            onPress={() => void send(actOn({ tag: "Settings" }, action))}
          >
            {action.sentence ?? action.title}
          </Button>
        ))}
      </div>
      {props.settings?.offeredCaption && (
        <p id="store-actions-caption" className="quiet">
          {props.settings.offeredCaption}
        </p>
      )}
    </>
  );
}

/** Automatic backups into a folder the person chose, where the browser can be given one. */
function BackupFolder() {
  const [shown, setShown] = useState<{ name: string; allowed: boolean } | undefined>();
  const [words, setWords] = useState("");
  const look = () => void folders.folder().then(setShown, () => setShown(undefined));
  useEffect(() => {
    look();
    void core.folderBackupWords().then(setWords);
  }, []);
  const done = (said: string) => {
    say(said);
    look();
  };
  return (
    <section aria-label="Automatic backups">
      <p>{words}</p>
      {shown ? (
        <>
          <p>{shown.allowed ? `Backing up to the folder ${shown.name}.` : `This browser needs asking again before it backs up to ${shown.name}.`}</p>
          <div className="buttons">
            {shown.allowed ? (
              <Button
                onPress={() =>
                  void folders.backUp(true).then(
                    async (name) => done(name ? await core.backedUpTo(shown.name, name) : await core.backupFailed("")),
                    async (error: Error) => done(await core.backupFailed(error?.message ?? "")),
                  )
                }
              >
                Back up to the folder now
              </Button>
            ) : (
              <Button onPress={() => void folders.allow().then((allowed) => done(allowed ? `Backing up to ${shown.name}` : "Still not allowed"))}>
                Allow backups to {shown.name}
              </Button>
            )}
            <Button onPress={() => void folders.choose().then((name) => name && done(`Backing up to ${name}`))}>Choose another folder</Button>
            <Button onPress={() => void folders.stop().then(() => done("No longer backing up to a folder"))}>Stop backing up to a folder</Button>
          </div>
        </>
      ) : (
        <div className="buttons">
          <Button onPress={() => void folders.choose().then((name) => name && done(`Backing up to ${name}`))}>Choose a folder for backups</Button>
        </div>
      )}
    </section>
  );
}

function Exports(props: { page: SettingsPage; settings: SettingsView | null }) {
  const choices = props.settings?.exports ?? [];
  return (
    <>
      <div className="buttons stacked">
        {choices.map((choice) => (
          <Button key={choice.format} onPress={() => void send({ tag: "Settings", value0: { tag: "Export", value0: choice.format } })}>
            {choice.label}
          </Button>
        ))}
        <ReadFile label={button(props.page, "import")} accept=".json,.lumbak" />
      </div>
      {props.page.caption && <p className="quiet">{props.page.caption}</p>}
    </>
  );
}
