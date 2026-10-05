"""Tasks: lists, what can be done to one, capture, and the trash.

Every field is sent as text and read by core. `due` and `repeat` go through the same date
grammar the quick-add line uses, so "next friday" means here exactly what it means there. The
dialog library has its own `request_date`, with its own parsing — using it would put a second
date parser in the product, which §6.2 warns against by name.
"""

from __future__ import annotations

from BTSpeak import dialogs

from client import LumennaError
from rows import Tree, describe
from session import Command, Session, ask, choose, confirm, live_menu, screen, spoken


# ---------------------------------------------------------------------------------------
# Lists
# ---------------------------------------------------------------------------------------


def undo_commands(session: Session) -> list[Command]:
    """Undo and redo, on every screen's main menu, with the same keys everywhere."""
    return [
        Command("Undo", lambda _: session.write("undo"), key="u"),
        Command("Redo", lambda _: session.write("redo"), key="y"),
    ]


def task_list(session: Session, query: str = "", title: str = "Tasks", prefix: str = "") -> str:
    """A filtered list of tasks, as a foldable tree.

    Enter shows a task; its context menu (M-Chord with Dot 7) has everything else, and the main
    menu (M-Chord) adds a task. `prefix` starts the quick-add line — `#Work ` in a project's
    list, so a task added there lands there, as on the phone.
    """
    state = {"heading": title}

    def build():
        try:
            result = session.call("task.list", query=query) if query else session.call("task.list")
        except LumennaError as error:
            return error.message
        heading = title
        readback = (result.get("query") or {}).get("description")
        if readback:
            # §6.2: a mis-parsed filter shows wrong results silently, and wrong results are
            # invisible. So the query is read back before its results are.
            heading = f"{title}: {readback}"
        said = [heading, result.get("announcement", "")]
        for unresolved in (result.get("query") or {}).get("unresolved", []):
            suggestion = unresolved.get("suggestion")
            hint = f", did you mean {suggestion}?" if suggestion else ""
            said.append(f"no {unresolved['kind']} called {unresolved['name']}{hint}")
        state["heading"] = ", ".join(part for part in said if part)
        return Tree(result.get("rows", [])).items()

    with screen("lumenna-tasks"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: show(session, row),
            context=task_commands(session),
            app=[
                Command("Add a task", lambda _: add_task(session, prefix), key="a"),
                Command("Search or filter", lambda _: query_tasks(session), key="/"),
                *undo_commands(session),
            ],
            empty="No tasks here. Press a to add one.",
            app_title=f"{title} menu",
        )
    return ""


def query_tasks(session: Session) -> str:
    """A filter typed now rather than saved; `search: words` looks through titles and notes."""
    query = assisted_input(session, "Filter, or search: and words", "filter", history_key="lumenna-filter")
    if not query:
        return ""
    return task_list(session, query, "Query")


def task_commands(session: Session) -> list[Command]:
    """What can be done to a task, on its context menu and by its keys."""
    def detail(row):
        return session.call("task.show", id=row["id"])

    def done(row):
        return row.get("checked") is True

    def stop_waiting(row):
        depends = detail(row).get("depends", [])
        if not depends:
            return "It waits for nothing"
        other = choose({d["id"]: d["title"] for d in depends}, "Stop waiting for")
        return session.write("task.depend.rm", id=row["id"], on=other) if other else ""

    def wait_for(row):
        task = detail(row)
        waiting = {row["id"]} | {d["id"] for d in task.get("depends", [])}
        other = pick_task(session, "Wait for", excluding=waiting)
        return session.write("task.depend.add", id=row["id"], on=other) if other else ""

    def subtask(row):
        parent = pick_task(session, "Make it a subtask of", excluding={row["id"]})
        return session.write("task.move", id=row["id"], parent=parent) if parent else ""

    return [
        Command(
            lambda row: "Mark not done" if done(row) else "Complete",
            lambda row: session.write("task.undone" if done(row) else "task.done", id=row["id"]),
            key="c",
        ),
        Command("Edit", lambda row: edit_task(session, detail(row)), key="e"),
        Command("Details", lambda row: show(session, row)),
        Command("Put it in a block", lambda row: assign_task(session, row["id"]), key="b"),
        Command("Move to a project", lambda row: move_to_project(session, row["id"]), key="m"),
        Command("Make it a subtask of another task", subtask, key="s"),
        Command(
            "Move it to the top level",
            lambda row: session.write("task.move", id=row["id"], top=True),
            key="t",
            applies=lambda row: row.get("depth", 0) > 0,
        ),
        Command("Wait for another task", wait_for, key="w"),
        Command("Stop waiting for another task", stop_waiting, key="n"),
        Command("Delete", lambda row: session.write("task.rm", id=row["id"]), deletes=True),
    ]


def show(session: Session, row: dict) -> str:
    """A task's details, as lines to pan through."""
    try:
        return task_details(session.call("task.show", id=row["id"]))
    except LumennaError as error:
        return error.message


def trash(session: Session) -> str:
    """Deleted tasks: Enter puts one back; erasing it for good is its context menu's, or the
    delete keys' (§3.2, §9)."""
    state = {"heading": "Trash"}

    def build():
        result = session.call("task.list", query="deleted")
        state["heading"] = f"Trash, {result.get('announcement', '')}"
        return Tree(result.get("rows", [])).items()

    restore = lambda row: session.write("task.restore", id=row["id"])  # noqa: E731
    with screen("lumenna-tasks"):
        live_menu(
            session, build, lambda: state["heading"],
            main=restore,
            context=[
                Command("Restore", restore, key="r"),
                Command("Erase for good", lambda row: erase(session, row), deletes=True),
            ],
            app=undo_commands(session),
            empty="The trash is empty.",
            app_title="Trash menu",
        )
    return ""


def erase(session: Session, row: dict) -> str:
    """Erasing rebuilds the document without the task and cannot be undone (§9), so it asks."""
    if not confirm(f"Erase {row['title']} and its history for good? This cannot be undone."):
        return ""
    return session.write("task.erase", id=row["id"], confirm=True)


# ---------------------------------------------------------------------------------------
# One task
# ---------------------------------------------------------------------------------------


def task_details(task: dict) -> str:
    """Everything about one task, as lines to pan through.

    A detail view is where the near-universal states are worth having, so this shows the full
    set rather than the notable ones a list line carries (§13).
    """
    lines = [task["title"]]
    if task.get("project"):
        lines.append(f"project: {task['project']}")
    if task.get("priority", 4) != 4:
        lines.append(f"priority: {task['priority']}")
    if task.get("due"):
        due = task["due"]
        if task.get("due_time"):
            due = f"{due} at {task['due_time']}"
        lines.append(f"due: {due}")
    if task.get("repetition"):
        lines.append(f"repeats: {task['repetition']}")
    elif task.get("recurrence"):
        lines.append(f"repeats by the rule {task['recurrence']}")
    if task.get("estimate_mins"):
        lines.append(f"estimate: {task['estimate_mins']} minutes")
    if task.get("labels"):
        lines.append("labels: " + ", ".join(task["labels"]))
    for dependency in task.get("depends", []):
        lines.append(f"waits for: {dependency['title']}")
    if task.get("state"):
        lines.append("state: " + ", ".join(task["state"]))
    if task.get("notes"):
        lines.append(f"notes: {task['notes']}")
    dialogs.view_lines(lines, wrap=True)
    return ""


PRIORITIES = {
    "1": "Priority 1, highest",
    "2": "Priority 2",
    "3": "Priority 3",
    "4": "Priority 4, none",
}


def edit_task(session: Session, task: dict) -> str:
    """One form over every field `task.edit` takes, sending only what changed — so that a
    concurrent edit to another field on another device is not overwritten with what this form
    happened to show."""
    due = " ".join(part for part in (task.get("due"), task.get("due_time")) if part)
    estimate = f"{task['estimate_mins']}m" if task.get("estimate_mins") else ""
    labels = ", ".join(task.get("labels", []))
    projects = [row["title"] for row in session.call("project.list").get("rows", [])]
    current = {
        "title": task["title"],
        "due": due,
        "repeat": task.get("repetition") or "",
        "priority": str(task.get("priority", 4)),
        "estimate": estimate,
        "project": task.get("project") or "",
        "labels": labels,
        "notes": task.get("notes", ""),
    }

    fields = [
        dialogs.InputField(key="title", prompt="Title", default_text=current["title"], required=True),
        dialogs.InputField(
            key="due", prompt="Due", default_text=due,
            format_hint="a date such as tomorrow, next friday or 2026-12-01, empty for none",
        ),
        dialogs.InputField(
            key="repeat", prompt="Repeats", default_text=current["repeat"],
            format_hint="such as every monday or every! 2 weeks, empty for no repetition",
        ),
        dialogs.InputField(
            key="priority", prompt="Priority", field_type="choice", choices=PRIORITIES,
            default_text=current["priority"],
        ),
        dialogs.InputField(
            key="estimate", prompt="Estimate", default_text=estimate,
            format_hint="such as 45m or 1h30m, empty for none",
        ),
    ]
    if current["project"] not in projects:
        # Not one this device can name (§3.1: not loaded yet); leave it where it is.
        del current["project"]
    else:
        fields.append(
            dialogs.InputField(
                key="project", prompt="Project", field_type="choice", choices=projects,
                default_text=current["project"],
            )
        )
    fields += [
        dialogs.InputField(
            key="labels", prompt="Labels", default_text=labels,
            format_hint="names separated by commas; a new name becomes a label",
        ),
        dialogs.InputField(
            key="notes", prompt="Notes", field_type="multiline", default_text=current["notes"],
        ),
    ]
    answers = dialogs.request_form(fields)
    if answers is None:
        return ""

    changes = {}
    for key, before in current.items():
        after = answers.get(key, before)
        after = after.strip() if isinstance(after, str) and key != "notes" else after
        if after == (before.strip() if key != "notes" else before):
            continue
        if key == "priority":
            changes[key] = int(after)
        elif key in ("due", "repeat", "estimate"):
            changes[key] = after or "none"
        elif key == "labels":
            changes[key] = [name.strip() for name in after.split(",") if name.strip()]
        else:
            changes[key] = after
    if not changes:
        return "Nothing changed"
    return session.write("task.edit", id=task["id"], **changes)


def move_to_project(session: Session, identifier: str) -> str:
    """Puts a task in another project; its subtasks follow (§3.2)."""
    names = [row["title"] for row in session.call("project.list").get("rows", [])]
    choice = choose({name: name for name in names}, "Move to")
    return session.write("task.move", id=identifier, project=choice) if choice else ""


def pick_task(session: Session, prompt: str, excluding: set = frozenset()) -> str | None:
    """One open task, by identifier; None if there is none or the person cancelled."""
    rows = [row for row in session.call("task.list").get("rows", []) if row["id"] not in excluding]
    if not rows:
        dialogs.show_message("There are no other open tasks")
        return None
    return choose({row["id"]: describe(row) for row in rows}, prompt)


ANOTHER_DAY = "\0another day"


def assign_task(session: Session, identifier: str) -> str:
    """Puts a task into a work block (§3.7): one of the coming week's, which the core chooses
    as it does for every app (`block.choices`), or one on a day named."""
    import day  # here, since day imports this module

    try:
        week = session.call("block.choices")
    except LumennaError as error:
        return error.message
    blocks = week.get("blocks", [])
    options = {
        b["id"]: f"{day.spoken_day(b['date'])}, {b['start']} to {b['end']}, {b['title']}" for b in blocks
    }
    options[ANOTHER_DAY] = "Another day"
    chosen = choose(options, "Put it in")
    if chosen is None:
        return ""
    if chosen == ANOTHER_DAY:
        when = ask("Which day?", "next monday")
        if when is None:
            return ""
        try:
            other = session.call("block.choices", **{"from": when, "days": 1})
        except LumennaError as error:
            return error.message
        blocks = other.get("blocks", [])
        if not blocks:
            return f"{day.spoken_day(other.get('from', ''))} has no work blocks to put it in"
        chosen = choose({b["id"]: f"{b['title']}, {b['start']} to {b['end']}" for b in blocks}, "Put it in")
        if chosen is None:
            return ""
    block = next(b for b in blocks if b["id"] == chosen)
    return day.assign_to(session, identifier, block, block["date"])


# ---------------------------------------------------------------------------------------
# Capture
# ---------------------------------------------------------------------------------------


def add_task(session: Session, prefix: str = "") -> str:
    """Capture, in the quick-add grammar (§6.1).

    The line is previewed before it is written, because an unknown project is an error and an
    unknown label is a new label — and being told which of those just happened *after* the
    fact is not the same thing (§3.4).
    """
    text = assisted_input(session, "Task", "quick-add", history_key="lumenna-add", default=prefix)
    if not text or not text.strip():
        return ""

    try:
        preview = session.call("preview", text=text)
    except LumennaError as error:
        return error.message

    if preview.get("has_errors"):
        problems = "; ".join(
            diagnostic["message"]
            for diagnostic in preview.get("diagnostics", [])
            if diagnostic["severity"] == "error"
        )
        dialogs.show_message(problems)
        return ""

    try:
        result = session.call("task.add", text=text)
    except LumennaError as error:
        return error.message
    session.wrote()
    return spoken(result)


def char_offset(text: str, byte_offset: int) -> int:
    """A UTF-8 byte offset from the server, as an index into a Python string.

    The protocol counts bytes, as Rust strings do, and Python counts code points. They agree
    only until the first accented letter or dash; after that, slicing with the server's
    number would splice the completion into the wrong place.
    """
    prefix = text.encode("utf-8")[:byte_offset]
    return len(prefix.decode("utf-8", errors="ignore"))


def assisted_input(
    session: Session, prompt: str, syntax: str, history_key: str | None = None, default: str = ""
) -> str | None:
    """Text entry, with a completion pass afterwards.

    `InputDialog` has no hook for completing as you type — Tab is form navigation — so this
    offers candidates once the line is entered instead, whenever the last word looks like a
    name that was started and not finished. It is the honest version of §6.3's completion on
    this toolkit: worth having for `#wo` when you cannot remember whether the project is Work
    or Workshop, and not pretending to be inline.

    Making it inline means subclassing `InputDialog` to bind a key to `complete`, which is
    the same shape of change §16.11 describes for the menu's speech and braille, and worth
    doing once the rest is in daily use.
    """
    text = dialogs.request_input(prompt, default_text=default, history_key=history_key)
    if not text:
        return text

    try:
        found = session.call(
            "complete", text=text, cursor=len(text.encode("utf-8")), syntax=syntax
        )
    except LumennaError:
        return text

    start = char_offset(text, found.get("start", 0))
    end = char_offset(text, found.get("end", 0))
    partial = text[start:end]
    candidates = found.get("candidates", [])
    if not partial or not candidates:
        return text
    if any(candidate["text"].lower() == partial.lower() for candidate in candidates):
        return text

    choice = choose(
        {candidate["text"]: candidate["label"] for candidate in candidates},
        found.get("announcement", "Completions"),
    )
    if choice is None:
        return text
    return text[:start] + choice + text[end:]
