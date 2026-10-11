// The page's handle on the core, which runs in a worker (worker.ts). Every call is a promise.

import * as Comlink from "comlink";
import type { Api } from "./worker";

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

export const core: Comlink.Remote<Api> = Comlink.wrap<Api>(worker);

export type { DayRow, Line, Proposed } from "./worker";
export type {
  Action,
  ActionKind,
  Choice,
  DueReminder,
  ReminderList,
  ReminderView,
  Setting,
  BlockFields,
  BlockShown,
  SidebarEntry,
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
