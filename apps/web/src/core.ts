// The page's handle on the core, which runs in a worker (worker.ts). Every call is a promise.

import * as Comlink from "comlink";
import type { Api } from "./worker";

const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });

export const core: Comlink.Remote<Api> = Comlink.wrap<Api>(worker);

export type { DayRow, Line } from "./worker";
export type {
  Action,
  ActionKind,
  BlockFields,
  BlockShown,
  SidebarEntry,
  ExportChoice,
  Place,
  PlanAssignment,
  PlanBlock,
  RowView,
  TaskDetail,
  TaskFields,
} from "./core/lumenna_web.js";
