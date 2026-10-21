// The page's handle on the core, which runs in a worker (worker.ts). Every call is a promise.

import * as Comlink from "comlink";
import type { Api } from "./worker";

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

export const core: Comlink.Remote<Api> = Comlink.wrap<Api>(worker);

export type { Offered, Said, Screen, Shown, ShownDay, ShownDayItem, ShownDetails, ShownAsking, ShownChoice, ShownDevice, ShownItem, ShownLine, ShownReminders } from "./worker";
export type {
  Action,
  Answer,
  Asking,
  BlockFormView,
  Due,
  DueEvent,
  Notice,
  PairEvent,
  PairingView,
  SettingView,
  SettingsView,
  ProposalView,
  ShellEvent,
  Choice,
  DueReminder,
  ReminderList,
  ReminderView,
  Setting,
  BlockFields,
  SidebarEntry,
  HeldView,
  ExportChoice,
  FormField,
  PairingWords,
  PickerWords,
  ProposalWords,
  Question,
  Place,
  PlanAssignment,
  PlanBlock,
  RowView,
  KeyCommand,
  KeyGroup,
  TaskDetail,
  TaskFields,
} from "./core/lumenna_web.js";

// The core's enumerations, which are numbers: compared by their names here, never by value.
export {
  ActionKind,
  Capitals,
  DayKind,
  DayMove,
  ExportFormat,
  FieldKind,
  SettingKind,
  Side,
  SidebarGroup,
  Subject,
  Syntax,
  TimeStyle,
} from "./core/lumenna_web.js";
