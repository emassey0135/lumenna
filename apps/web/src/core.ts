// The page's handle on the core, which runs in a worker (worker.ts). Every call is a promise.

import * as Comlink from "comlink";
import type { Api } from "./worker";

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

export const core: Comlink.Remote<Api> = Comlink.wrap<Api>(worker);

export type { Said, Screen, Shown, ShownDay, ShownDayItem, ShownDetails, ShownAsking, ShownChoice, ShownDevice, ShownItem, ShownLine, ShownReminders } from "./worker";
export type {
  Action,
  ActionKind,
  Answer,
  Asking,
  BlockFormView,
  DayMove,
  Due,
  DueEvent,
  Notice,
  PairEvent,
  PairingView,
  SettingView,
  SettingsView,
  ProposalView,
  Event,
  Choice,
  DueReminder,
  ReminderList,
  ReminderView,
  Setting,
  BlockFields,
  SidebarEntry,
  Side,
  HeldView,
  ExportChoice,
  FormField,
  PairingWords,
  ProposalWords,
  Question,
  Place,
  PlanAssignment,
  PlanBlock,
  RowView,
  Shortcut,
  ShortcutGroup,
  TaskDetail,
  TaskFields,
} from "./core/lumenna_web.js";
