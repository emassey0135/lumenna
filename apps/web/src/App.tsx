// The web client's window: places, the list for the place chosen, and the chosen
// task's details, as the desktop apps have them — here as landmarks, navigation, main and
// complementary, which screen readers move between with their own keys.

import * as Comlink from "comlink";
import { useEffect, useRef, useState } from "react";
import { Button } from "react-aria-components";
import { Prompts } from "./Prompts";
import { BlockForms } from "./BlockForm";
import { Blocks } from "./Blocks";
import { core } from "./core";
import type { Place } from "./core";
import { Day } from "./Day";
import { Devices } from "./Devices";
import { Details } from "./Details";
import type { DetailsHandle } from "./Details";
import { QuickAdd } from "./QuickAdd";
import { say } from "./say";
import { Settings } from "./Settings";
import { Sidebar } from "./Sidebar";
import { TaskList } from "./TaskList";

/** The store's name in this browser: one, unless a page asks for another (`?profile=`). */
const profile = new URLSearchParams(location.search).get("profile") ?? "default";

export function App() {
  const [ready, setReady] = useState(false);
  const [failure, setFailure] = useState<string | undefined>();
  const [notice, setNotice] = useState<string | undefined>();
  const [revision, setRevision] = useState(0);
  const [place, setPlace] = useState<Place>("Tasks");
  const [shown, setShown] = useState({ title: "Tasks", query: "", quickAddPrefix: "" });
  const [selected, setSelected] = useState<string | undefined>();
  const [adding, setAdding] = useState(false);
  const [devicesOpen, setDevicesOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const details = useRef<DetailsHandle>(null);

  useEffect(() => {
    core.open(profile).then(
      () => setReady(true),
      (error: Error) => setFailure(error.message),
    );
    // In a local-first app the browser may hold changes that are nowhere else yet: storage it
    // may evict under pressure is data loss. Ask for it to be kept.
    void navigator.storage?.persist?.().then((kept) => {
      if (!kept) {
        setNotice(
          "This browser may clear Lumenna's data when it runs short of space. Installing Lumenna, or allowing it to keep data, prevents that.",
        );
      }
    });
  }, []);

  // Syncing runs while the page is open, once there is another device to sync with.
  // What arrives is in the store already: everything showing it reads it again.
  const startSyncing = async () => {
    try {
      const devices = await core.devices();
      if (!devices.some((device) => !device.thisDevice)) return;
      await core.startSync(Comlink.proxy(() => setRevision((r) => r + 1)));
      setSyncing(true);
    } catch (error) {
      say(`Syncing could not start: ${(error as Error).message}`);
    }
  };

  useEffect(() => {
    if (ready) void startSyncing();
  }, [ready]);

  // Another tab or process wrote: everything showing the store reads it again.
  useEffect(() => {
    if (!ready) return;
    let last: number | undefined;
    const timer = setInterval(() => {
      void core.outsideVersion().then((version) => {
        if (last !== undefined && version !== last) setRevision((r) => r + 1);
        last = version;
      });
    }, 1000);
    return () => clearInterval(timer);
  }, [ready]);

  useEffect(() => {
    if (!ready) return;
    void core.place(place).then(setShown);
    setSelected(undefined);
  }, [ready, place]);

  const changed = () => setRevision((r) => r + 1);

  const undo = async (redo: boolean) => {
    try {
      say(await (redo ? core.redo() : core.undo()));
      changed();
    } catch (error) {
      say((error as Error).message);
    }
  };

  // Ctrl+Z undoes and Ctrl+Shift+Z or Ctrl+Y redoes, from anywhere but a field, where they
  // undo typing as they do in the desktop apps' fields, or a dialog, whose question is not the
  // store's. Command for Ctrl on a Mac.
  useEffect(() => {
    if (!ready) return;
    const keys = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
      const key = event.key.toLowerCase();
      const redo = (key === "z" && event.shiftKey) || (key === "y" && !event.shiftKey);
      if (key !== "z" && !redo) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, [contenteditable='true'], [role='dialog'], [role='alertdialog']")) return;
      event.preventDefault();
      void undo(redo);
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  }, [ready]);

  if (failure) {
    return (
      <main className="app">
        <h1>Lumenna</h1>
        <p role="alert">{failure}</p>
      </main>
    );
  }
  if (!ready) {
    return <p role="status">Opening Lumenna…</p>;
  }

  return (
    <div className="app">
      <header>
        <h1>Lumenna</h1>
        <Button onPress={() => setAdding(true)}>New Task</Button>
        <Button onPress={() => void undo(false)}>Undo</Button>
        <Button onPress={() => void undo(true)}>Redo</Button>
        <Button onPress={() => setDevicesOpen(true)}>Devices…</Button>
        <Button onPress={() => setSettingsOpen(true)}>Settings…</Button>
        {/* In the banner, so landmark navigation does not skip it. */}
        {notice && (
          <p className="notice" role="note">
            {notice}
          </p>
        )}
      </header>
      <nav aria-label="Places">
        <Sidebar revision={revision} place={place} onPlace={setPlace} onChanged={changed} />
      </nav>
      <main aria-label={shown.title}>
        {place === "Today" ? (
          <Day
            revision={revision}
            onChanged={changed}
            onSelectTask={setSelected}
            onOpenTask={(id) => {
              setSelected(id);
              details.current?.focus();
            }}
          />
        ) : place === "Blocks" ? (
          <Blocks revision={revision} onChanged={changed} />
        ) : (
          <TaskList
            revision={revision}
            place={place}
            title={shown.title}
            query={shown.query}
            selected={selected}
            onSelect={setSelected}
            onOpen={() => details.current?.focus()}
            onChanged={changed}
          />
        )}
      </main>
      <aside aria-label="Task details">
        <Details ref={details} id={selected} revision={revision} onChanged={changed} />
      </aside>
      <Devices
        isOpen={devicesOpen}
        onClose={() => setDevicesOpen(false)}
        revision={revision}
        syncing={syncing}
        onPaired={() => {
          changed();
          void startSyncing();
        }}
      />
      <Settings isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} revision={revision} onChanged={changed} />
      <Prompts />
      <BlockForms />
      <QuickAdd
        prefix={shown.quickAddPrefix}
        isOpen={adding}
        onClose={() => setAdding(false)}
        onAdded={(id) => {
          changed();
          if (id) setSelected(id);
        }}
      />
    </div>
  );
}
