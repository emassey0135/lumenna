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
import type { PairEvent, PairingView, PairingWords, Screen, ShortAction } from "./core";
import { Side } from "./core";
import { actOn, isDelete, keyPressed, rowHeld } from "./actions";
import { ChooseDialog } from "./Questions";
import { say } from "./say";
import { send } from "./session";


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
export function DevicesPage(props: { devices: Screen["devices"]; held: Screen["devicesHeld"] }) {
  // The device in hand is the app's (`devicesHeld`): choosing one tells it, and after an unpair
  // it says which is in hand instead.
  const selected = props.held.focus ?? undefined;
  // Pairing's words come with the Devices page, in the web's capitals.
  const words = props.devices?.words;
  const devices = props.devices?.list?.devices ?? [];
  const pairing = props.devices?.pairing;

  // Shown: the app lists the devices and asks how syncing is going, and keeps both current.
  useEffect(() => {
    void send({ tag: "Devices", value0: { tag: "Show" } });
  }, []);

  const others = devices.filter((d) => !d.thisDevice).length;
  const chosen = devices.find((d) => d.nodeId === selected);

  const syncNow = () => void send({ tag: "Devices", value0: { tag: "SyncNow" } });

  const run = async (action: ShortAction) => {
    if (chosen) await send(actOn(rowHeld(Side.Devices, chosen.nodeId), action));
  };

  const keys = (event: KeyboardEvent) => {
    // Delete unpairs, as it removes in every other list: the core's, on the device held here.
    if (!isDelete(event) || !chosen) return;
    event.preventDefault();
    void send({ tag: "Hold", value0: Side.Devices, value1: chosen.nodeId }).then(() => send(keyPressed("delete")));
  };

  return (
    <>
      {pairing && words ? (
        <Pairing words={words} pairing={pairing} />
      ) : (
        <>
          <p id="sync-status">{props.devices?.status ?? ""}</p>
          <div onKeyDownCapture={keys} data-list="devices">
            <ListBox
              aria-label="Paired devices"
              aria-describedby="devices-hint"
              items={devices.map((device) => ({ ...device, id: device.nodeId }))}
              selectionMode="single"
              selectedKeys={selected ? [selected] : []}
              onSelectionChange={(keys: Selection) => {
                const [key] = keys === "all" ? [] : [...keys].map(String);
                if (key) void send({ tag: "Hold", value0: Side.Devices, value1: key });
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
            <Button isDisabled={!words} onPress={() => void send({ tag: "Devices", value0: { tag: "Pair", value0: { tag: "Open", value0: browserName() } } })}>
              {words?.title ?? "Pair a device"}
            </Button>
            {chosen?.offered?.map((action) => (
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

/**
 * The words to compare, asked while the app holds them: answered, the app lets them go. Drawn at
 * the app's root beside Settings, not inside its dialog: a modal nested in another's content
 * left Settings stuck closing, its overlay over the page, once the pairing ended.
 */
export function PairingQuestion(props: { devices: Screen["devices"] }) {
  const words = props.devices?.words;
  const three = props.devices?.pairing?.words;
  return (
    <ChooseDialog
      isOpen={Boolean(words && three)}
      heading={words?.matchTitle ?? ""}
      detail={words && three ? `${words.matchMessage} ${three.join(", ")}.` : ""}
      options={words ? [words.matchYes, words.matchNo] : []}
      answer={(index) => void pair({ tag: "Confirm", value0: index === 0 })}
    />
  );
}

/** Sends what is done while pairing, and says what the app says about it. */
async function pair(event: PairEvent) {
  await send({ tag: "Devices", value0: { tag: "Pair", value0: event } });
}

function Pairing(props: { words: PairingWords; pairing: PairingView }) {
  const { words, pairing } = props;
  const code = useRef<HTMLInputElement>(null);
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

  return (
    <>
      <h3>{words.title}</h3>
      {/* The introduction in sentence case, as the web writes. */}
      <p>{words.introSentence || words.intro}</p>
      <TextField
        className="field"
        value={name}
        onChange={(value) => {
          setName(value);
          void send({ tag: "Devices", value0: { tag: "Pair", value0: { tag: "Name", value0: value } } });
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
            <Label>{words.myCode}</Label>
            <Input ref={code} />
          </TextField>
          <div className="buttons">
            <Button
              onPress={() =>
                void navigator.clipboard.writeText(pairing.mine ?? "").then(
                  () => say(words.copied),
                  () => say("Could not copy the code; select it and copy it instead."),
                )
              }
            >
              {words.copyCode}
            </Button>
          </div>
        </>
      ) : (
        <div className="buttons">
          <Button isDisabled={pairing.running} onPress={() => void pair({ tag: "Wait" })}>
            {words.wait}
          </Button>
        </div>
      )}
      <TextField
        className="field"
        value={theirs}
        onChange={(value) => {
          setTheirs(value);
          void send({ tag: "Devices", value0: { tag: "Pair", value0: { tag: "Code", value0: value } } });
        }}
        isDisabled={pairing.running && !pairing.waiting}
      >
        <Label>{words.theirCode}</Label>
        <Input />
      </TextField>
      {pairing.problem && (
        <p role="alert" className="problem">
          {pairing.problem}
        </p>
      )}
      <div className="buttons">
        <Button onPress={() => void pair({ tag: "Cancel" })}>Cancel</Button>
        {/* Typed or pasted: the clipboard is never read unasked, which a browser would have to
            prompt for, and which would send whatever happened to be copied. */}
        <Button isDisabled={pairing.running && !pairing.waiting} onPress={() => void pair({ tag: "Join" })}>
          {words.join}
        </Button>
      </div>
    </>
  );
}
