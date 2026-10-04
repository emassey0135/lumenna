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
from session import Flag, Session, ask, choose, confirm, live_menu
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
    """A day as it is lived, opening on now."""
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
        rows = plan_rows(plan)
        tree = Tree(rows)
        leading = [
            dialogs.DynamicMenuItem(title="Previous day", action=lambda: step(-1)),
            dialogs.DynamicMenuItem(title="Next day", action=lambda: step(1)),
            dialogs.DynamicMenuItem(title="Go to a day", action=go_to),
            dialogs.DynamicMenuItem(
                title="Add a block", action=lambda: add_block(session, date=plan["date"])
            ),
        ]
        return leading + tree.items(lambda row: plan_actions(session, plan, row))

    # §13: opening the day lands on now, not at midnight — so the first build decides where.
    plan = session.call("plan")
    rows = plan_rows(plan)
    now = next(
        (i for i, row in enumerate(rows) if row["role"] == "now" or row.get("when") == "now"),
        0,
    )
    live_menu(session, build, lambda: shown["heading"], default=4 + now, moved=moved)
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
                state = [sitting["status"]]
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


def plan_actions(session: Session, plan: dict, row: dict) -> str:
    """What can be done from one row of the day."""
    role = row["role"]
    date = plan["date"]
    if role == "block":
        return block_actions(session, row["block"], date)
    if role == "assignment":
        return sitting_actions(session, row["sitting"])
    if role == "free":
        free = row["free"]
        return add_block(session, date=date, at=free["start"], minutes=min(free["minutes"], 720))
    if role == "cancelled":
        cancelled = row["cancelled"]
        choice = choose({"restore": "Put this day back"}, describe(row))
        if choice == "restore":
            return session.write("block.restore", id=cancelled["series"], date=date)
        return ""
    return describe(row)


def block_actions(session: Session, block: dict, date: str) -> str:
    actions = {}
    if block["kind"] == "work":
        actions["assign"] = "Assign a task"
    actions["edit"] = "Edit"
    if block.get("repeats"):
        actions["cancel"] = "Cancel this day"
    if block.get("changed_for_this_day"):
        actions["restore"] = "Put this day back as the series has it"
    actions["rm"] = "Delete the block"
    choice = choose(actions, f"{block['title']}, {block['start']} to {block['end']}")
    if choice == "assign":
        task = tasks.pick_task(session, f"Assign to {block['title']}")
        return session.write("assign", task=task, block=block["id"], date=date) if task else ""
    if choice == "edit":
        return edit_block(session, block, date)
    if choice == "cancel":
        return session.write("block.cancel", id=block["series"], date=date)
    if choice == "restore":
        return session.write("block.restore", id=block["series"], date=date)
    if choice == "rm":
        return delete_block(session, block["series"], block["title"], block.get("repeats", False))
    return ""


def sitting_actions(session: Session, sitting: dict) -> str:
    running = sitting["status"] == "in progress"
    actions = {
        "timer": "Stop the timer" if running else "Start the timer",
        "log": "Log minutes by hand",
        "task": "The task itself",
        "unassign": "Take it out of the block",
    }
    choice = choose(actions, sitting["title"])
    if choice == "timer":
        return session.write("stop" if running else "start", assignment=sitting["id"])
    if choice == "log":
        text = ask(f"Minutes on {sitting['title']}, the whole of this sitting", "")
        if text is None:
            return ""
        if not text.isdigit():
            return "That is not a number of minutes"
        return session.write("stop", assignment=sitting["id"], minutes=int(text))
    if choice == "task":
        return tasks.task_actions(session, {"id": sitting["task"], "title": sitting["title"]})
    if choice == "unassign":
        return session.write("unassign", assignment=sitting["id"])
    return ""


# ---------------------------------------------------------------------------------------
# Blocks
# ---------------------------------------------------------------------------------------


def blocks(session: Session) -> str:
    """Every block series, and a way to make one."""
    state = {"heading": "Blocks"}

    def build():
        listing = session.call("block.list")
        state["heading"] = f"Blocks, {listing.get('announcement', '')}"
        items = [dialogs.DynamicMenuItem(title="Add a block", action=lambda: add_block(session))]
        for row in listing.get("rows", []):
            items.append(
                dialogs.DynamicMenuItem(
                    title=describe(row),
                    action=(lambda row=row: series_actions(session, row)),
                    delete=(lambda row=row: series_delete(session, row)),
                )
            )
        return items

    live_menu(session, build, lambda: state["heading"])
    return ""


def series_actions(session: Session, row: dict) -> str:
    choice = choose({"edit": "Edit every occurrence", "rm": "Delete"}, describe(row))
    if choice == "edit":
        return edit_series(session, row["id"])
    if choice == "rm":
        return series_delete(session, row)
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
