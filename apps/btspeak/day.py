"""The day and its blocks.

The timeline has no visual metaphor here, and blocks as a time-ordered list lose
nothing. What a timeline shows by empty space becomes a row: free time between blocks,
and now. Sittings sit under their block as a second level, so the same folding works.
"""

from __future__ import annotations

import datetime

from BTSpeak import dialogs

import actions
from actions import spoken_day
from client import LumennaError
from rows import Tree, clock, describe
from session import Command, Flag, Session, ask, choose, live_menu, row_item, screen
import tasks


KINDS = {"work": "Work, takes tasks", "break": "Break", "event": "Event"}

#: A block form's yes-or-no fields, as a choice.
YES_NO = {"yes": "Yes", "no": "No"}

#: The block's settings a kind brings with it.
FLAGS = (("accepts_tasks", "Takes tasks"), ("counts_capacity", "Counts toward hours for work"), ("anchored", "Anchored, so it never moves"))


def length(minutes: int) -> str:
    """`1 hour 30 minutes`, as the phone and the command line say it."""
    hours, rest = divmod(int(minutes), 60)
    parts = []
    if hours:
        parts.append(f"{hours} hour{'s' if hours != 1 else ''}")
    if rest or not hours:
        parts.append(f"{rest} minute{'s' if rest != 1 else ''}")
    return " ".join(parts)


# ---------------------------------------------------------------------------------------
# The planner
# ---------------------------------------------------------------------------------------


def day_plan(session: Session) -> str:
    """A day as it is lived, opening on now.

    Enter does a row's main thing — edits a block, shows a sitting's task, adds a block in free
    time, puts a cancelled day back. The rest is on the row's context menu (M-Chord with
    Dot 7); moving between days and adding a block are on the main menu (M-Chord), and on
    their keys.
    """
    shown = {"date": "", "plan": {}, "heading": ""}
    moved = Flag()

    def turn(to: str) -> str:
        shown["date"] = to
        moved.set()
        return ""

    def step(days: int) -> str:
        current = shown["plan"].get("date")
        if not current:
            return ""
        return turn((datetime.date.fromisoformat(current) + datetime.timedelta(days=days)).isoformat())

    def go_to() -> str:
        day = ask("Go to which day?", "tomorrow")
        return turn(day) if day else ""

    def build():
        try:
            plan = session.call("plan", date=shown["date"]) if shown["date"] else session.call("plan")
        except LumennaError as error:
            # A day that does not read leaves the last one shown.
            shown["date"] = shown["plan"].get("date", "")
            return error.message
        shown["plan"] = plan
        shown["heading"] = f"{spoken_day(plan['date'])}, {plan.get('summary') or plan.get('announcement', '')}"
        return Tree(plan_rows(plan)).items()

    def date() -> str:
        return shown["plan"].get("date", "")

    # Opening the day lands on now, not at midnight — so the first build decides where.
    rows = plan_rows(session.call("plan"))
    now = next(
        (i for i, row in enumerate(rows) if row["role"] == "now" or row.get("when") == "now"),
        0,
    )
    with screen("lumenna-day"):
        live_menu(
            session, build, lambda: shown["heading"], default=now, moved=moved,
            main=lambda row: plan_main(session, row),
            context=lambda rows: actions.commands(session, rows),
            app=[
                Command("Add a block", lambda _: add_block(session, date=date() or "today"), key="a"),
                Command("Previous day", lambda _: step(-1), key="p"),
                Command("Next day", lambda _: step(1), key="n"),
                Command("Go to a day", lambda _: go_to(), key="g"),
                Command("Today", lambda _: turn(""), key="t"),
                *tasks.undo_commands(session),
            ],
            empty="Nothing planned.",
            app_title="Day menu",
        )
    return ""


def plan_rows(plan: dict) -> list[dict]:
    """The timeline as rows: blocks with their sittings, free time, now, and what was
    cancelled for the day, each said the way the phone says it."""
    blocks = {block["row"]: block for block in plan.get("blocks", [])}
    timeline = plan.get("timeline") or [{"item": "block", "row": row} for row in blocks]
    rows: list[dict] = []
    for item in timeline:
        if item["item"] == "block" and item["row"] in blocks:
            block = blocks[item["row"]]
            # The core words the details for every app.
            state = list(block.get("details", []))
            rows.append({
                "id": block["id"], "role": "block", "depth": 0, "block": block,
                "when": block.get("when", ""),
                "title": f"{clock(block['start'])} to {clock(block['end'])}, {block['title']}",
                "state": state, "actions": block.get("actions", []),
            })
            for sitting in block.get("assignments", []):
                state = list(sitting.get("details", []))
                rows.append({
                    "id": sitting["id"], "role": "assignment", "depth": 1,
                    "sitting": sitting, "block": block,
                    "title": sitting["title"], "state": state, "actions": sitting.get("actions", []),
                })
        elif item["item"] == "free":
            rows.append({
                "id": f"free@{item['start']}", "role": "free", "depth": 0, "free": item,
                "title": f"Free, {length(item['minutes'])}",
                "value": f"{clock(item['start'])} to {clock(item['end'])}",
                "actions": item.get("actions", []),
            })
        elif item["item"] == "now":
            rows.append({"id": "now", "role": "now", "depth": 0, "title": f"Now, {clock(item['time'])}"})
    for cancelled in plan.get("cancelled", []):
        rows.append({
            "id": f"cancelled@{cancelled['series']}", "role": "cancelled", "depth": 0,
            "cancelled": cancelled,
            "title": f"{clock(cancelled['start'])}, {cancelled['title']}",
            "state": ["cancelled for this day"], "actions": cancelled.get("actions", []),
        })
    return rows


def plan_main(session: Session, row: dict) -> str:
    """What Enter does on a row of the day: edits a block, shows a sitting's task, adds a
    block in free time, puts a cancelled day back — each the row's own action."""
    role = row["role"]
    if role == "block":
        return actions.run_kind(session, row, "edit")
    if role == "assignment":
        return tasks.show(session, {"id": row["sitting"]["task"]})
    if role == "free":
        return actions.run_kind(session, row, "add_block")
    if role == "cancelled":
        return actions.run_kind(session, row, "restore_day")
    return describe(row)


# ---------------------------------------------------------------------------------------
# Blocks
# ---------------------------------------------------------------------------------------


def blocks(session: Session) -> str:
    """Every block series: Enter edits one, the delete keys delete it, a adds one."""
    state = {"heading": "Blocks"}

    def build():
        listing = session.call("block.list")
        state["heading"] = f"Blocks, {listing.get('announcement', '')}"
        return [row_item(row, describe(row)) for row in listing.get("rows", [])]

    with screen("lumenna-day"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: actions.run_kind(session, row, "edit"),
            context=lambda rows: actions.commands(session, rows),
            app=[Command("Add a block", lambda _: add_block(session), key="a"), *tasks.undo_commands(session)],
            empty="No blocks yet. Press a to add one.",
            app_title="Blocks menu",
        )
    return ""


# ---------------------------------------------------------------------------------------
# The block form, on the core's rules: the fields it starts from (`form.block_fields`,
# `form.day_block_fields`), a kind's own settings (`form.block_defaults`), and what saving
# sends (`form.block_edit`, `form.new_block`).
# ---------------------------------------------------------------------------------------


def value(session: Session, method: str, **params):
    """What a form function computes from `params`."""
    return session.call(method, **params).get("value")


def flag_fields(fields: dict) -> list:
    return [
        dialogs.InputField(
            key=key, prompt=name, field_type="choice", choices=YES_NO,
            default_text="yes" if fields[key] else "no",
        )
        for key, name in FLAGS
    ]


def with_kind(session: Session, before: dict, after: dict) -> dict:
    """`after`, its settings brought by its kind where the kind changed and they did not, as
    a form's check boxes go back to a new kind's own."""
    if after["kind"] != before["kind"]:
        defaults = value(session, "form.block_defaults", kind=after["kind"]) or {}
        for key, _ in FLAGS:
            if after[key] == before[key] and key in defaults:
                after[key] = defaults[key]
    return after


def answered(before: dict, answers: dict) -> dict:
    """The form's fields after it was filled in: text as typed, yes or no as a flag."""
    after = dict(before)
    for key, typed in answers.items():
        if key in before:
            after[key] = typed == "yes" if isinstance(before[key], bool) else typed
    return after


def add_block(session: Session, date: str = "today", at: str = "9am", minutes: int = 60) -> str:
    """One block, once or repeating."""
    answers = dialogs.request_form(
        [
            dialogs.InputField(key="title", prompt="Name", required=True),
            dialogs.InputField(
                key="start", prompt="Starts at", default_text=at,
                format_hint="a time such as 9am or 14:30",
            ),
            dialogs.InputField(
                key="minutes", prompt="Minutes", default_text=str(minutes),
                format_hint="a whole number of minutes",
            ),
            dialogs.InputField(
                key="kind", prompt="Kind", field_type="choice", choices=KINDS, default_text="work"
            ),
            dialogs.InputField(key="date", prompt="Starting on", default_text=date),
            dialogs.InputField(
                key="repeat", prompt="Repeats",
                format_hint="such as every weekday, or empty for a block that happens once",
            ),
        ]
    )
    if answers is None:
        return ""
    try:
        defaults = value(session, "form.block_defaults", kind=answers["kind"]) or {}
        fields = {
            "title": answers["title"], "start": answers["start"], "minutes": answers["minutes"],
            "kind": answers["kind"], "repeat": answers["repeat"],
            "until": "", "min_minutes": "", "task_filter": "", "colour": "", "notes": "",
            **{key: bool(defaults.get(key)) for key, _ in FLAGS},
        }
        block = value(session, "form.new_block", fields=fields, date=answers["date"])
    except LumennaError as error:
        return error.message
    return session.write("block.add", **{k: v for k, v in block.items() if v is not None})


def block_scope(block: dict, date: str) -> dict | None:
    """Which days a change to `block` means: asked of a repeating one, never guessed.
    None when the person cancelled."""
    if not block.get("repeats"):
        return {"all": True}
    scope = choose({"day": f"{spoken_day(date)} only", "all": "Every occurrence"}, "Change which?")
    if scope is None:
        return None
    return {"all": True} if scope == "all" else {"date": date}


def edit_block(session: Session, block: dict) -> str:
    """Asks "this day, or every day?" of a repeating block — never guessed."""
    date = block["id"].split("@")[-1]
    scope = block_scope(block, date)
    if scope is None:
        return ""
    if scope.get("all"):
        return edit_series(session, block["series"])
    try:
        before = value(session, "form.day_block_fields", block=block)
    except LumennaError as error:
        return error.message
    fields = [
        dialogs.InputField(key="title", prompt="Name", default_text=before["title"], required=True),
        dialogs.InputField(key="start", prompt="Starts at", default_text=before["start"]),
        dialogs.InputField(key="minutes", prompt="Minutes", default_text=before["minutes"], format_hint="a whole number of minutes"),
        dialogs.InputField(key="kind", prompt="Kind", field_type="choice", choices=KINDS, default_text=before["kind"]),
        *flag_fields(before),
    ]
    return save_block(session, block["series"], before, fields, {"date": date})


def edit_series(session: Session, series: str) -> str:
    """The form for every occurrence of a block."""
    try:
        shown = session.call("block.show", id=series)
        before = value(session, "form.block_fields", block=shown)
    except LumennaError as error:
        return error.message
    fields = [
        dialogs.InputField(key="title", prompt="Name", default_text=before["title"], required=True),
        dialogs.InputField(key="start", prompt="Starts at", default_text=before["start"]),
        dialogs.InputField(key="minutes", prompt="Minutes", default_text=before["minutes"], format_hint="a whole number of minutes"),
        dialogs.InputField(key="kind", prompt="Kind", field_type="choice", choices=KINDS, default_text=before["kind"]),
        *flag_fields(before),
        dialogs.InputField(
            key="repeat", prompt="Repeats", default_text=before["repeat"],
            format_hint="such as every weekday; empty makes it happen once",
        ),
        dialogs.InputField(key="notes", prompt="Notes", default_text=before["notes"]),
        dialogs.InputField(
            key="min_minutes", prompt="Shortest length", default_text=before["min_minutes"],
            format_hint="minutes it may be shortened to; empty for the kind's own",
        ),
        dialogs.InputField(
            key="task_filter", prompt="Tasks from", default_text=before["task_filter"],
            format_hint="a filter such as #Work; empty for any",
        ),
    ]
    if shown.get("repeats"):
        fields.append(dialogs.InputField(
            key="until", prompt="Until", default_text=before["until"],
            format_hint="its last day; empty to repeat for good",
        ))
    fields.append(dialogs.InputField(key="colour", prompt="Colour", default_text=before["colour"]))
    return save_block(session, series, before, fields, {"all": True})


def save_block(session: Session, series: str, before: dict, fields: list, scope: dict) -> str:
    """Asks the form, then sends what the core says changed."""
    answers = dialogs.request_form(fields)
    if answers is None:
        return ""
    after = with_kind(session, before, answered(before, answers))
    try:
        edit = value(session, "form.block_edit", before=before, after=after)
    except LumennaError as error:
        return error.message
    if not edit:
        return "Nothing changed"
    return session.write("block.edit", id=series, **{k: v for k, v in edit.items() if v is not None}, **scope)


def free_time_form(session: Session, action: dict, row=None) -> str:
    """A block added in free time: its day and start from the free time, its length up to
    the free time's end, at most twelve hours."""
    free = (row or {}).get("free") or {}
    return add_block(session, date=action["target"], at=action.get("other") or "9am", minutes=min(free.get("minutes", 60), 720))


actions.FORMS[("block", "edit")] = lambda session, action, row: edit_block(session, row["block"])
actions.FORMS[("series", "edit")] = lambda session, action, row: edit_series(session, action["target"])
actions.FORMS[("free_time", "add_block")] = free_time_form
