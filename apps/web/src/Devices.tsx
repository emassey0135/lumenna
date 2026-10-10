// Paired devices and pairing: Settings' Devices page, as in the other apps.
//
// A browser pairs by code: it has no local network to find another device on. Either it waits
// and its code is entered on the other device, or it enters the code the other device shows.
// Either way both show three words, and only if the person says they match is anything
// paired. Then it syncs while it is open, through a relay that cannot read what it carries.

import * as Comlink from "comlink";
import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { Button, Input, Label, ListBox, ListBoxItem, Text, TextField } from "react-aria-components";
import type { Selection } from "react-aria-components";
import { core } from "./core";
import type { Action, PairingWords } from "./core";
import { byKind, perform } from "./actions";
import { choose } from "./Prompts";
import { say } from "./say";

interface Device {
  id: string;
  name: string;
  thisDevice: boolean;
  text: string;
  /** What can be done to it, as the core offers it: no Unpair on this device's own row. */
  actions: Action[];
}

/** What this browser is called on the other devices until someone renames it. */
function browserName(): string {
  const agent = navigator.userAgent;
  const browser = /Edg\//.test(agent)
    ? "Edge"
    : /Firefox\//.test(agent)
      ? "Firefox"
      : /Chrome\//.test(agent)
        ? "Chrome"
        : /Safari\//.test(agent)
          ? "Safari"
          : "Web browser";
  const system = /Windows/.test(agent)
    ? "Windows"
    : /CrOS/.test(agent)
      ? "ChromeOS"
      : /Android/.test(agent)
        ? "Android"
        : /iPhone|iPad/.test(agent)
          ? "iOS"
          : /Mac OS/.test(agent)
            ? "macOS"
            : /Linux/.test(agent)
              ? "Linux"
              : "";
  return system ? `${browser} on ${system}` : browser;
}

/** Settings' Devices page: the paired devices, how syncing is going, and pairing. */
export function DevicesPage(props: {
  revision: number;
  /** Whether this browser is keeping in sync. */
  syncing: boolean;
  /** Something about the devices changed: syncing may need to start. */
  onPaired: () => void;
}) {
  const [devices, setDevices] = useState<Device[]>([]);
  const [empty, setEmpty] = useState("");
  const [selected, setSelected] = useState<string | undefined>();
  const [pairing, setPairing] = useState(false);
  const [reload, setReload] = useState(0);
  const [status, setStatus] = useState("");
  const [words, setWords] = useState<PairingWords | undefined>();

  useEffect(() => {
    void core.pairingWords().then(setWords);
  }, []);

  // Read again whenever the store changes, a sync round's outcome included, so a device's
  // status is current without Sync Now.
  useEffect(() => {
    void core.devices().then(
      (found) => {
        setDevices(found.list);
        setEmpty(found.empty);
      },
      (error: Error) => say(error.message),
    );
    // How syncing is going, in the surface's words, given whether this browser's loop runs.
    void core.syncStatus().then(setStatus, (error: Error) => setStatus(error.message));
  }, [props.revision, props.syncing, reload]);

  const others = devices.filter((d) => !d.thisDevice).length;

  const chosen = devices.find((d) => d.id === selected);
  const refresh = () => setReload((r) => r + 1);

  const syncNow = async () => {
    say("Syncing");
    try {
      say(await core.syncNow());
    } catch (error) {
      say((error as Error).message);
    }
    refresh();
  };

  const run = async (action: Action) => {
    const done = await perform(action);
    if (!done) return;
    if (action.kind === "unpair" && done.changed) setSelected(undefined);
    say(done.said);
    refresh();
  };

  const keys = (event: KeyboardEvent) => {
    // Delete unpairs, as it removes in every other list.
    const action = event.key === "Delete" ? byKind(chosen?.actions, "unpair") : undefined;
    if (action) {
      event.preventDefault();
      void run(action);
    }
  };

  return (
    <>
      {pairing && words ? (
        <Pairing
          words={words}
          onDone={(paired) => {
            setPairing(false);
            refresh();
            if (paired) props.onPaired();
          }}
        />
      ) : (
        <>
          <p id="sync-status">{status}</p>
          <div onKeyDownCapture={keys}>
            <ListBox
              aria-label="Paired devices"
              aria-describedby="devices-hint"
              items={devices}
              selectionMode="single"
              selectedKeys={selected ? [selected] : []}
              onSelectionChange={(keys: Selection) => {
                if (keys !== "all") setSelected([...keys].map(String)[0]);
              }}
              renderEmptyState={() => <p className="quiet">{empty}</p>}
            >
              {(device) => <ListBoxItem textValue={device.text}>{device.text}</ListBoxItem>}
            </ListBox>
          </div>
          <p id="devices-hint" className="quiet">
            Delete unpairs the selected device.
          </p>
          <div className="buttons">
            <Button isDisabled={others === 0 || !props.syncing} onPress={() => void syncNow()}>
              Sync now
            </Button>
            <Button isDisabled={!words} onPress={() => setPairing(true)}>
              {words?.title ?? "Pair a device"}…
            </Button>
            {chosen?.actions.map((action) => (
              <Button
                key={action.kind}
                className={action.destructive ? "destructive" : undefined}
                onPress={() => void run(action)}
              >
                {action.sentence ?? action.title}
              </Button>
            ))}
          </div>
        </>
      )}
    </>
  );
}

function Pairing(props: { words: PairingWords; onDone: (paired: boolean) => void }) {
  const { words } = props;
  const [name, setName] = useState(browserName);
  const [theirs, setTheirs] = useState("");
  const [mine, setMine] = useState<string | undefined>();
  const [running, setRunning] = useState(false);
  // Whether the pairing running is waiting to be found, rather than dialling a code.
  const [waiting, setWaiting] = useState(false);
  const [problem, setProblem] = useState<string | undefined>();
  const code = useRef<HTMLInputElement>(null);
  // A code entered while waiting: the wait is given up, and this is dialled once it ends.
  const then = useRef<string | undefined>(undefined);

  // The code is what the person needs next: focus goes to it, selected, ready to copy.
  useEffect(() => {
    if (mine) {
      code.current?.focus();
      code.current?.select();
    }
  }, [mine]);

  const start = async (dial: string | undefined) => {
    setProblem(undefined);
    setRunning(true);
    setWaiting(dial === undefined);
    say(dial ? words.connecting : words.opening);
    try {
      const said = await core.pair(
        dial,
        name.trim() || browserName(),
        Comlink.proxy((shown: string) => {
          setMine(shown);
          say(words.waiting);
        }),
        Comlink.proxy(async (three: string[]) => {
          const detail = `${words.match_message} ${three.join(", ")}.`;
          const match = (await choose(words.match_title, detail, [words.match_yes, words.match_no])) === 0;
          say(match ? words.finishing : words.refusing);
          return match;
        }),
      );
      say(said);
      props.onDone(true);
    } catch (error) {
      setRunning(false);
      setMine(undefined);
      const next = then.current;
      then.current = undefined;
      if (next) {
        void start(next);
        return;
      }
      setProblem((error as Error).message);
      say((error as Error).message);
    }
  };

  const cancel = () => {
    if (running) void core.cancelPairing();
    else props.onDone(false);
  };

  return (
    <>
      <h3>{words.title}</h3>
      <p>{words.intro}</p>
      <TextField className="field" value={name} onChange={setName} isDisabled={running} autoFocus>
        <Label>Name for this browser</Label>
        <Input />
        <Text slot="description" className="quiet">
          What your other devices call it.
        </Text>
      </TextField>
      {mine ? (
        <>
          <TextField className="field" value={mine} isReadOnly>
            <Label>{words.my_code}</Label>
            <Input ref={code} />
          </TextField>
          <div className="buttons">
            <Button
              onPress={() =>
                void navigator.clipboard.writeText(mine).then(
                  () => say(words.copied),
                  () => say("Could not copy the code; select it and copy it instead."),
                )
              }
            >
              {words.copy_code}
            </Button>
          </div>
        </>
      ) : (
        <div className="buttons">
          <Button isDisabled={running} onPress={() => void start(undefined)}>
            {words.wait}
          </Button>
        </div>
      )}
      <TextField className="field" value={theirs} onChange={setTheirs} isDisabled={running && !waiting}>
        <Label>{words.their_code}</Label>
        <Input />
        <Text slot="description" className="quiet">
          {words.empty_means}
        </Text>
      </TextField>
      {problem && (
        <p role="alert" className="problem">
          {problem}
        </p>
      )}
      <div className="buttons">
        <Button onPress={cancel}>Cancel</Button>
        <Button
          isDisabled={running && !waiting}
          onPress={async () => {
            let code = theirs.trim();
            if (!code) {
              // The clipboard, which is how a code sent from the other device usually arrives —
              // but not this tab's own, which Copy Code put there: that would pair it with
              // itself. A browser may ask first, or refuse; then a code has to be typed.
              const pasted = (await navigator.clipboard?.readText?.().catch(() => "")) ?? "";
              if (pasted.trim() && pasted.trim() !== mine) {
                code = pasted.trim();
                setTheirs(code);
              }
            }
            if (!code) {
              setProblem(words.need_code);
              return;
            }
            // One pairing at a time: a code entered while waiting gives up the wait first.
            if (running) {
              then.current = code;
              void core.cancelPairing();
              say(words.switching);
              return;
            }
            void start(code);
          }}
        >
          {words.join}
        </Button>
      </div>
    </>
  );
}
