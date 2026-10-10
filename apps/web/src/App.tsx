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
import { KeyboardShortcuts, asksForKeys, commandFor } from "./Keys";
import { defer, focusIn, movePane, runCommand, useCommand } from "./commands";
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
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [keysOpen, setKeysOpen] = useState(false);
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
      const devices = (await core.devices()).list;
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

  // "?" opens the keyboard shortcuts from anywhere but a field or a dialog. Taken before a
  // list sees it, where it would find a row by typing.
  useEffect(() => {
    if (!ready) return;
    const keys = (event: KeyboardEvent) => {
      if (!asksForKeys(event)) return;
      event.preventDefault();
      event.stopPropagation();
      setKeysOpen(true);
    };
    window.addEventListener("keydown", keys, true);
    return () => window.removeEventListener("keydown", keys, true);
  }, [ready]);

  // The shared commands, on the web's keys (Keys.tsx), from anywhere but a dialog, whose keys
  // are its own. The screen shown answers first; what it cannot, the window does here.
  useEffect(() => {
    if (!ready) return;
    const keys = (event: KeyboardEvent) => {
      const id = commandFor(event);
      if (!id) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest("[role='dialog'], [role='alertdialog']")) return;
      event.preventDefault();
      event.stopPropagation();
      runCommand(id);
    };
    window.addEventListener("keydown", keys, true);
    return () => window.removeEventListener("keydown", keys, true);
  }, [ready]);

  // Goes to a place, focus on its list once it has drawn.
  const focusMain = useRef(false);
  const goTo = (to: Place) => {
    if (to === place) {
      focusIn(".app > main", false);
      return;
    }
    focusMain.current = true;
    setPlace(to);
  };
  useEffect(() => {
    if (!focusMain.current) return;
    focusMain.current = false;
    focusIn(".app > main", false);
  }, [shown]);
  // Goes to `to` first, where the screen there runs command `id`.
  const thenRun = (to: Place, id: string) => {
    focusMain.current = false;
    defer(id);
    setPlace(to);
  };

  useCommand("new-task", () => setAdding(true), -1);
  useCommand("new-block", () => thenRun("Today", "new-block"), -1);
  useCommand("settings", () => setSettingsOpen(true), -1);
  useCommand(
    "sync-now",
    () =>
      void (async () => {
        say("Syncing");
        try {
          say(await core.syncNow());
          changed();
        } catch (error) {
          say((error as Error).message);
        }
      })(),
    -1,
  );
  useCommand("filter", () => thenRun("Tasks", "filter"), -1);
  useCommand("go-today", () => goTo("Today"), -1);
  useCommand("go-tasks", () => goTo("Tasks"), -1);
  useCommand("go-blocks", () => goTo("Blocks"), -1);
  useCommand("go-trash", () => goTo("Trash"), -1);
  useCommand("next-pane", () => movePane(1), -1);
  useCommand("previous-pane", () => movePane(-1), -1);
  for (const id of ["previous-day", "next-day", "go-to-now", "go-to-day"]) {
    // eslint-disable-next-line react-hooks/rules-of-hooks -- a fixed list, called in the same order every time
    useCommand(id, () => thenRun("Today", id), -1);
  }
  for (const id of ["mark-done", "save-task", "put-in-block", "move-to-project"]) {
    // eslint-disable-next-line react-hooks/rules-of-hooks -- a fixed list, called in the same order every time
    useCommand(id, () => say("No task is selected."), -1);
  }

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
        <Button onPress={() => setAdding(true)}>New task</Button>
        <Button onPress={() => void undo(false)}>Undo</Button>
        <Button onPress={() => void undo(true)}>Redo</Button>
        <Button onPress={() => setSettingsOpen(true)}>Settings</Button>
        <Button onPress={() => setKeysOpen(true)}>Keyboard shortcuts</Button>
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
      <Settings
        isOpen={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        revision={revision}
        onChanged={changed}
        syncing={syncing}
        onPaired={() => {
          changed();
          void startSyncing();
        }}
      />
      <KeyboardShortcuts isOpen={keysOpen} onClose={() => setKeysOpen(false)} />
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
