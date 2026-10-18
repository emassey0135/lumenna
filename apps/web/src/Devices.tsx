// Paired devices and pairing: Settings' Devices page, as in the other apps.
//
// A browser pairs by code: it has no local network to find another device on. Either it waits
// and its code is entered on the other device, or it enters the code the other device shows.
// Either way both show three words, and only if the person says they match is anything
// paired. Then it syncs while it is open, through a relay that cannot read what it carries.

import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { Button, Input, Label, ListBox, ListBoxItem, Text, TextField } from "react-aria-components";
import type { Selection } from "react-aria-components";
import { core } from "./core";
import type { Action, PairEvent, PairingView, PairingWords, Screen } from "./core";
import { byKind } from "./actions";
import { run as runEvent, sayAll, send } from "./session";
import { choose } from "./Prompts";


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
export function DevicesPage(props: { devices: Screen["devices"]; onChanged: () => void }) {
  const [selected, setSelected] = useState<string | undefined>();
  const [words, setWords] = useState<PairingWords | undefined>();
  const devices = props.devices?.list?.devices ?? [];
  const pairing = props.devices?.pairing;

  // Shown: the app lists the devices and asks how syncing is going, and keeps both current.
  useEffect(() => {
    void send({ Devices: "Show" });
    void core.pairingWords().then(setWords);
  }, []);

  const others = devices.filter((d) => !d.this_device).length;
  const chosen = devices.find((d) => d.node_id === selected);

  const syncNow = async () => sayAll((await send({ Devices: "SyncNow" })).said);

  const run = async (action: Action) => {
    const turn = await runEvent({ Perform: action });
    if (action.kind === "unpair") setSelected(undefined);
    sayAll(turn.said);
    props.onChanged();
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
        <Pairing words={words} pairing={pairing} />
      ) : (
        <>
          <p id="sync-status">{props.devices?.status ?? ""}</p>
          <div onKeyDownCapture={keys}>
            <ListBox
              aria-label="Paired devices"
              aria-describedby="devices-hint"
              items={devices.map((device) => ({ ...device, id: device.node_id }))}
              selectionMode="single"
              selectedKeys={selected ? [selected] : []}
              onSelectionChange={(keys: Selection) => {
                if (keys !== "all") setSelected([...keys].map(String)[0]);
              }}
              renderEmptyState={() => <p className="quiet">{props.devices?.list?.empty ?? ""}</p>}
            >
              {(device) => <ListBoxItem textValue={device.text}>{device.text}</ListBoxItem>}
            </ListBox>
          </div>
          <p id="devices-hint" className="quiet">
            Delete unpairs the selected device.
          </p>
          <div className="buttons">
            <Button isDisabled={others === 0 || !props.devices?.syncing} onPress={() => void syncNow()}>
              Sync now
            </Button>
            <Button isDisabled={!words} onPress={() => void send({ Devices: { Pair: { Open: browserName() } } })}>
              {words?.title ?? "Pair a device"}
            </Button>
            {chosen?.actions?.map((action) => (
              <Button
                key={action.kind}
                className={action.destructive ? "react-aria-Button destructive" : "react-aria-Button"}
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

/** Sends what is done while pairing, and says what the app says about it. */
async function pair(event: PairEvent) {
  sayAll((await send({ Devices: { Pair: event } })).said);
}

function Pairing(props: { words: PairingWords; pairing: PairingView }) {
  const { words, pairing } = props;
  const code = useRef<HTMLInputElement>(null);
  const asked = useRef<string | undefined>(undefined);
  // What is typed, shown at once; the app has it a moment later.
  const [name, setName] = useState(pairing.name);
  const [theirs, setTheirs] = useState(pairing.theirs);

  // The code is what the person needs next: focus goes to it, selected, ready to copy.
  useEffect(() => {
    if (pairing.mine) {
      code.current?.focus();
      code.current?.select();
    }
  }, [pairing.mine]);

  // The words to compare, asked once each time they come.
  useEffect(() => {
    const three = pairing.words;
    if (!three || asked.current === three.join(" ")) return;
    asked.current = three.join(" ");
    const detail = `${words.match_message} ${three.join(", ")}.`;
    void choose(words.match_title, detail, [words.match_yes, words.match_no]).then((index) => pair({ Confirm: index === 0 }));
  }, [pairing.words]);

  return (
    <>
      <h3>{words.title}</h3>
      <p>{words.intro}</p>
      <TextField
        className="field"
        value={name}
        onChange={(value) => {
          setName(value);
          void send({ Devices: { Pair: { Name: value } } });
        }}
        isDisabled={pairing.running}
        autoFocus
      >
        <Label>Name for this browser</Label>
        <Input />
        <Text slot="description" className="quiet">
          What your other devices call it.
        </Text>
      </TextField>
      {pairing.mine ? (
        <>
          <TextField className="field" value={pairing.mine} isReadOnly>
            <Label>{words.my_code}</Label>
            <Input ref={code} />
          </TextField>
          <div className="buttons">
            <Button
              onPress={() =>
                void navigator.clipboard.writeText(pairing.mine ?? "").then(
                  () => sayAll([words.copied]),
                  () => sayAll(["Could not copy the code; select it and copy it instead."]),
                )
              }
            >
              {words.copy_code}
            </Button>
          </div>
        </>
      ) : (
        <div className="buttons">
          <Button isDisabled={pairing.running} onPress={() => void pair("Wait")}>
            {words.wait}
          </Button>
        </div>
      )}
      <TextField
        className="field"
        value={theirs}
        onChange={(value) => {
          setTheirs(value);
          void send({ Devices: { Pair: { Code: value } } });
        }}
        isDisabled={pairing.running && !pairing.waiting}
      >
        <Label>{words.their_code}</Label>
        <Input />
      </TextField>
      {pairing.problem && (
        <p role="alert" className="problem">
          {pairing.problem}
        </p>
      )}
      <div className="buttons">
        <Button onPress={() => void pair("Cancel")}>Cancel</Button>
        {/* Typed or pasted: the clipboard is never read unasked, which a browser would have to
            prompt for, and which would send whatever happened to be copied. */}
        <Button isDisabled={pairing.running && !pairing.waiting} onPress={() => void pair("Join")}>
          {words.join}
        </Button>
      </div>
    </>
  );
}
