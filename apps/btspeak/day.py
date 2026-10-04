"""The day and its blocks (§3.6, §3.7).

§16.11: the timeline has no visual metaphor here, and blocks as a time-ordered list lose
nothing. What a timeline shows by empty space becomes a row (§13): free time between blocks,
and now. Sittings sit under their block as a second level, so the same folding works.
"""

from __future__ import annotations

import datetime

from BTSpeak import dialogs

from client import LumennaError
from rows import Tree, describe
from session import Command, Flag, Session, ask, choose, confirm, live_menu, row_item, screen
import tasks


KINDS = {"work": "Work, takes tasks", "break": "Break", "event": "Event"}


def length(minutes: int) -> str:
    """`1 hour 30 minutes`, as the phone and the command line say it."""
    hours, rest = divmod(int(minutes), 60)
    parts = []
    if hours:
        parts.append(f"{hours} hour{'s' if hours != 1 else ''}")
    if rest or not hours:
        parts.append(f"{rest} minute{'s' if rest != 1 else ''}")
    return " ".join(parts)


def spoken_day(iso: str) -> str:
    """`Sunday 4 October 2026`: the day as a person says it, rather than as ISO."""
    try:
        day = datetime.date.fromisoformat(iso)
    except ValueError:
        return iso
    return f"{day:%A} {day.day} {day:%B} {day.year}"


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

    # §13: opening the day lands on now, not at midnight — so the first build decides where.
    rows = plan_rows(session.call("plan"))
    now = next(
        (i for i, row in enumerate(rows) if row["role"] == "now" or row.get("when") == "now"),
        0,
    )
    with screen("lumenna-day"):
        live_menu(
            session, build, lambda: shown["heading"], default=now, moved=moved,
            main=lambda row: plan_main(session, date(), row),
            context=day_commands(session, date),
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
            state = [length(block["duration_mins"]), f"{block['kind']} block"]
            if block.get("when"):
                state.append(block["when"])
            if block.get("changed_for_this_day"):
                state.append("changed for this day")
            if block["kind"] == "work":
                count = len(block.get("assignments", []))
                state.append(
                    "nothing assigned" if count == 0
                    else "1 task assigned" if count == 1
                    else f"{count} tasks assigned"
                )
            rows.append({
                "id": block["id"], "role": "block", "depth": 0, "block": block,
                "when": block.get("when", ""),
                "title": f"{block['start']} to {block['end']}, {block['title']}",
                "state": state,
            })
            for sitting in block.get("assignments", []):
                state = sitting_state(sitting)
                if sitting.get("minutes"):
                    state.append(f"{length(sitting['minutes'])} logged")
                if sitting.get("capped"):
                    state.append("capped, the timer looks forgotten")
                rows.append({
                    "id": sitting["id"], "role": "assignment", "depth": 1,
                    "sitting": sitting, "block": block,
                    "title": sitting["title"], "state": state,
                })
        elif item["item"] == "free":
            rows.append({
                "id": f"free@{item['start']}", "role": "free", "depth": 0, "free": item,
                "title": f"Free, {length(item['minutes'])}",
                "value": f"{item['start']} to {item['end']}",
            })
        elif item["item"] == "now":
            rows.append({"id": "now", "role": "now", "depth": 0, "title": f"Now, {item['time']}"})
    for cancelled in plan.get("cancelled", []):
        rows.append({
            "id": f"cancelled@{cancelled['series']}", "role": "cancelled", "depth": 0,
            "cancelled": cancelled,
            "title": f"{cancelled['start']}, {cancelled['title']}",
            "state": ["cancelled for this day"],
        })
    return rows


def sitting_state(sitting: dict) -> list[str]:
    """The status, with the planned length beside it: "planned for 45 minutes" before it
    starts and "45 minutes planned" after, as the phone and the command line say it, so the
    two never read as "planned, planned"."""
    planned = sitting.get("planned_mins")
    if not planned:
        return [sitting["status"]]
    if sitting["status"] == "planned":
        return [f"planned for {length(planned)}"]
    return [sitting["status"], f"{length(planned)} planned"]


def ask_length(prompt: str, current: int | None = None) -> tuple[bool, int | None]:
    """A sitting's planned length: (answered, minutes), where minutes is None for none. Left
    empty, it is none — so when assigning, Enter alone skips it."""
    text = dialogs.request_input(
        f"{prompt}, in minutes. Leave it empty for no planned length",
        default_text=str(current) if current else "",
    )
    if text is None:
        return False, None
    text = text.strip()
    if not text:
        return True, None
    if not text.isdigit() or int(text) == 0:
        dialogs.show_message("That is not a number of minutes")
        return False, None
    return True, int(text)


def assign_to(session: Session, task: str, block: dict, date: str) -> str:
    """Puts a task in a block, asking how long the sitting is meant to take (§3.7)."""
    answered, minutes = ask_length("How long is this sitting meant to take")
    if not answered:
        return ""
    params = {"task": task, "block": block["id"], "date": date}
    if minutes:
        params["minutes"] = minutes
    return session.write("assign", **params)


def plan_main(session: Session, date: str, row: dict) -> str:
    """What Enter does on a row of the day."""
    role = row["role"]
    if role == "block":
        return edit_block(session, row["block"], date)
    if role == "assignment":
        return tasks.show(session, {"id": row["sitting"]["task"]})
    if role == "free":
        free = row["free"]
        return add_block(session, date=date, at=free["start"], minutes=min(free["minutes"], 720))
    if role == "cancelled":
        return session.write("block.restore", id=row["cancelled"]["series"], date=date)
    return describe(row)


def day_commands(session: Session, date) -> list[Command]:
    """What the context menu offers on each kind of row of the day. A letter means one thing
    on each kind of row, so the same letter can serve a block and a sitting."""
    def role(*roles):
        return lambda row: row.get("role") in roles

    def block(row):
        return row["block"]

    def sitting(row):
        return row["sitting"]

    def assign(row):
        task = tasks.pick_task(session, f"Assign to {block(row)['title']}")
        return assign_to(session, task, block(row), date()) if task else ""

    def toggle_timer(row):
        running = sitting(row)["status"] == "in progress"
        return session.write("stop" if running else "start", assignment=sitting(row)["id"])

    def log_minutes(row):
        text = ask(f"Minutes on {sitting(row)['title']}, the whole of this sitting", "")
        if text is None:
            return ""
        if not text.isdigit():
            return "That is not a number of minutes"
        return session.write("stop", assignment=sitting(row)["id"], minutes=int(text))

    def plan_length(row):
        answered, minutes = ask_length(f"Planned length of {sitting(row)['title']}", sitting(row).get("planned_mins"))
        return session.write("length", assignment=sitting(row)["id"], minutes=minutes) if answered else ""

    return [
        # A block
        Command("Edit", lambda row: edit_block(session, block(row), date()), key="e", applies=role("block")),
        Command("Assign a task", assign, key="i", applies=lambda row: role("block")(row) and block(row)["kind"] == "work"),
        Command(
            "Cancel this day",
            lambda row: session.write("block.cancel", id=block(row)["series"], date=date()),
            key="x",
            applies=lambda row: role("block")(row) and block(row).get("repeats"),
        ),
        Command(
            "Put this day back as the series has it",
            lambda row: session.write("block.restore", id=block(row)["series"], date=date()),
            key="o",
            applies=lambda row: role("block")(row) and block(row).get("changed_for_this_day"),
        ),
        Command(
            "Delete the block",
            lambda row: delete_block(session, block(row)["series"], block(row)["title"], block(row).get("repeats", False)),
            applies=role("block"),
            deletes=True,
        ),
        # A sitting
        Command(
            lambda row: "Stop the timer" if sitting(row)["status"] == "in progress" else "Start the timer",
            toggle_timer, key="s", applies=role("assignment"),
        ),
        Command("Planned length", plan_length, key="l", applies=role("assignment")),
        Command("Log minutes by hand", log_minutes, key="m", applies=role("assignment")),
        Command(
            "The task itself",
            lambda row: tasks.show(session, {"id": sitting(row)["task"]}),
            applies=role("assignment"),
        ),
        Command(
            "Take it out of the block",
            lambda row: session.write("unassign", assignment=sitting(row)["id"]),
            applies=role("assignment"),
            deletes=True,
        ),
        # Free time, and a cancelled day
        Command(
            "Add a block here",
            lambda row: add_block(session, date=date(), at=row["free"]["start"], minutes=min(row["free"]["minutes"], 720)),
            key="a",
            applies=role("free"),
        ),
        Command(
            "Put this day back",
            lambda row: session.write("block.restore", id=row["cancelled"]["series"], date=date()),
            key="o",
            applies=role("cancelled"),
        ),
    ]


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
            main=lambda row: edit_series(session, row["id"]),
            context=[
                Command("Edit every occurrence", lambda row: edit_series(session, row["id"]), key="e"),
                Command("Delete", lambda row: series_delete(session, row), deletes=True),
            ],
            app=[Command("Add a block", lambda _: add_block(session), key="a"), *tasks.undo_commands(session)],
            empty="No blocks yet. Press a to add one.",
            app_title="Blocks menu",
        )
    return ""


def series_delete(session: Session, row: dict) -> str:
    try:
        shown = session.call("block.show", id=row["id"])
    except LumennaError as error:
        return error.message
    return delete_block(session, shown["id"], shown["title"], shown.get("repeats", False))


def delete_block(session: Session, series: str, title: str, repeats: bool) -> str:
    if repeats:
        question = (
            f"Delete {title}? Every occurrence goes, not only one day; to skip one day, "
            "cancel it instead."
        )
    else:
        question = f"Delete {title}? It goes to the trash with its assignments."
    if not confirm(question):
        return ""
    return session.write("block.rm", id=series)


def add_block(session: Session, date: str = "today", at: str = "9am", minutes: int = 60) -> str:
    """One block, once or repeating."""
    answers = dialogs.request_form(
        [
            dialogs.InputField(key="title", prompt="Name", required=True),
            dialogs.InputField(
                key="at", prompt="Starts at", default_text=at,
                format_hint="a time such as 9am or 14:30",
            ),
            dialogs.InputField(
                key="minutes", prompt="Minutes", default_text=str(minutes),
                validate=lambda text: text.strip().isdigit() and int(text) > 0,
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
    params = {
        "title": answers["title"],
        "at": answers["at"],
        "minutes": int(answers["minutes"]),
        "date": answers["date"],
        "kind": answers["kind"],
    }
    if answers["repeat"].strip():
        params["repeat"] = answers["repeat"].strip()
    return session.write("block.add", **params)


def edit_block(session: Session, block: dict, date: str) -> str:
    """Asks "this day, or every day?" of a repeating block — never guessed (§4.3)."""
    if block.get("repeats"):
        scope = choose(
            {"day": f"{spoken_day(date)} only", "all": "Every occurrence"}, "Change which?"
        )
        if scope is None:
            return ""
        if scope == "all":
            return edit_series(session, block["series"])
        return edit_fields(
            session, block["series"], block["title"], block["start"], block["duration_mins"],
            block["kind"], None, {"date": date},
        )
    return edit_series(session, block["series"])


def edit_series(session: Session, series: str) -> str:
    try:
        shown = session.call("block.show", id=series)
    except LumennaError as error:
        return error.message
    # A rule the grammar cannot say is left out of the form rather than shown as something
    # it is not; saving then keeps it.
    repeat = shown.get("repetition") or ""
    if shown.get("repeats") and not shown.get("repetition"):
        repeat = None
    return edit_fields(
        session, shown["id"], shown["title"], shown["start"], shown["minutes"], shown["kind"],
        repeat, {"all": True},
    )


def edit_fields(
    session: Session, series: str, title: str, start: str, minutes: int, kind: str,
    repeat: str | None, scope: dict,
) -> str:
    """The block form, sending only what changed. `repeat` is None where it may not change:
    one day of a series cannot repeat differently."""
    before = {"title": title, "at": start, "minutes": str(minutes), "kind": kind}
    fields = [
        dialogs.InputField(key="title", prompt="Name", default_text=title, required=True),
        dialogs.InputField(key="at", prompt="Starts at", default_text=start),
        dialogs.InputField(
            key="minutes", prompt="Minutes", default_text=str(minutes),
            validate=lambda text: text.strip().isdigit() and int(text) > 0,
            format_hint="a whole number of minutes",
        ),
        dialogs.InputField(
            key="kind", prompt="Kind", field_type="choice", choices=KINDS, default_text=kind
        ),
    ]
    if repeat is not None:
        before["repeat"] = repeat
        fields.append(
            dialogs.InputField(
                key="repeat", prompt="Repeats", default_text=repeat,
                format_hint="such as every weekday; empty makes it happen once",
            )
        )
    answers = dialogs.request_form(fields)
    if answers is None:
        return ""
    changes = {}
    for key, was in before.items():
        now = str(answers.get(key, was)).strip()
        if now == was:
            continue
        if key == "minutes":
            changes[key] = int(now)
        elif key == "repeat":
            changes[key] = now or "none"
        else:
            changes[key] = now
    if not changes:
        return "Nothing changed"
    return session.write("block.edit", id=series, **changes, **scope)
