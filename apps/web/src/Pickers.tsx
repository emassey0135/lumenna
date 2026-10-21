// The browser's own day and time pickers, beside the line they fill: the line stays first and
// the way in ("next friday", "9am"), which the core reads; a day or time picked here is put in
// it, ISO or HH:MM, for the core to read the same way. The picker is the browser's own control,
// which says its value and opens its own calendar or clock; only its name is the core's.

import { useEffect, useState } from "react";
import { core } from "./core";
import type { PickerWords } from "./core";

let known: PickerWords | undefined;
let asked: Promise<PickerWords> | undefined;

/** The core's words for a picker, in sentence case, once the worker has given them. */
function usePickerWords(): PickerWords | undefined {
  const [words, setWords] = useState(known);
  useEffect(() => {
    if (known) return;
    asked ??= core.pickerWords().then((words) => (known = words));
    void asked.then(setWords);
  }, []);
  return words;
}

const isoDay = /^\d{4}-\d{2}-\d{2}$/;
const clockTime = /^(\d{1,2}):(\d{2})$/;

/** A calendar for `line`: open on the day it names, if it is a date, else `from`. */
export function DayPicker(props: { line: string; from?: string | null; onPick: (day: string) => void }) {
  const words = usePickerWords();
  const line = props.line.trim();
  const value = isoDay.test(line) ? line : (props.from ?? "");
  return (
    <label className="picker">
      {words?.day}
      <input type="date" value={value} onChange={(event) => event.target.value && props.onPick(event.target.value)} />
    </label>
  );
}

/** A clock for `line`: showing the time it holds, if it is one in HH:MM. */
export function TimePicker(props: { line: string; onPick: (time: string) => void }) {
  const words = usePickerWords();
  const shown = clockTime.exec(props.line.trim());
  const value = shown ? `${shown[1].padStart(2, "0")}:${shown[2]}` : "";
  return (
    <label className="picker">
      {words?.time}
      <input type="time" value={value} onChange={(event) => event.target.value && props.onPick(event.target.value)} />
    </label>
  );
}
