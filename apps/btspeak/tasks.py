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
from session import Session, ask, choose, confirm, live_menu, spoken


# ---------------------------------------------------------------------------------------
# Lists
# ---------------------------------------------------------------------------------------


def task_list(session: Session, query: str = "", title: str = "Tasks", prefix: str = "") -> str:
    """A filtered list of tasks, as a foldable tree, with a way to add one at the top.

    `prefix` starts the quick-add line — `#Work ` in a project's list, so a task added there
    lands there, as on the phone.
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

        add = dialogs.DynamicMenuItem(
            title="Add a task", action=lambda: add_task(session, prefix)
        )
        tree = Tree(result.get("rows", []))
        return [add] + tree.items(
            lambda row: task_actions(session, row),
            on_delete=lambda row: session.write("task.rm", id=row["id"]),
        )

    live_menu(session, build, lambda: state["heading"])
    return ""


def trash(session: Session) -> str:
    """Deleted tasks: put one back, or erase it for good (§3.2, §9)."""
    state = {"heading": "Trash"}

    def build():
        result = session.call("task.list", query="deleted")
        rows = result.get("rows", [])
        state["heading"] = f"Trash, {result.get('announcement', '')}"
        if not rows:
            return "The trash is empty"
        return Tree(rows).items(
            lambda row: trash_actions(session, row),
            on_delete=lambda row: erase(session, row),
        )

    live_menu(session, build, lambda: state["heading"])
    return ""


def trash_actions(session: Session, row: dict) -> str:
    choice = choose({"restore": "Restore", "erase": "Erase for good"}, describe(row))
    if choice == "restore":
        return session.write("task.restore", id=row["id"])
    if choice == "erase":
        return erase(session, row)
    return ""


def erase(session: Session, row: dict) -> str:
    """Erasing rebuilds the document without the task and cannot be undone (§9), so it asks."""
    if not confirm(f"Erase {row['title']} and its history for good? This cannot be undone."):
        return ""
    return session.write("task.erase", id=row["id"], confirm=True)


# ---------------------------------------------------------------------------------------
# One task
# ---------------------------------------------------------------------------------------


def task_actions(session: Session, row: dict) -> str:
    """What can be done to one task, read fresh so the offer matches what it is now."""
    identifier = row["id"]
    try:
        task = session.call("task.show", id=identifier)
    except LumennaError as error:
        return error.message
    done = "completed" in task.get("state", [])
    actions = {
        "done": "Mark not done" if done else "Complete",
        "edit": "Edit",
        "show": "Details",
        "assign": "Put it in a block",
        "project": "Move to a project",
        "under": "Make it a subtask of another task",
    }
    if task.get("parent"):
        actions["top"] = "Move it to the top level"
    actions["wait"] = "Wait for another task"
    for other in task.get("depends", []):
        actions[f"unwait:{other['id']}"] = f"Stop waiting for {other['title']}"
    actions["rm"] = "Delete"

    choice = choose(actions, describe(row))
    if choice is None:
        return ""
    try:
        if choice == "done":
            return session.write("task.undone" if done else "task.done", id=identifier)
        if choice == "edit":
            return edit_task(session, task)
        if choice == "show":
            return task_details(task)
        if choice == "assign":
            return assign_task(session, identifier)
        if choice == "project":
            return move_to_project(session, identifier)
        if choice == "under":
            parent = pick_task(session, "Make it a subtask of", excluding={identifier})
            return session.write("task.move", id=identifier, parent=parent) if parent else ""
        if choice == "top":
            return session.write("task.move", id=identifier, top=True)
        if choice == "wait":
            waiting = {identifier} | {d["id"] for d in task.get("depends", [])}
            other = pick_task(session, "Wait for", excluding=waiting)
            return session.write("task.depend.add", id=identifier, on=other) if other else ""
        if choice.startswith("unwait:"):
            return session.write("task.depend.rm", id=identifier, on=choice.split(":", 1)[1])
        if choice == "rm":
            return session.write("task.rm", id=identifier)
    except LumennaError as error:
        return error.message
    return ""


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


def assign_task(session: Session, identifier: str) -> str:
    """Puts a task into a work block, on today or another day (§3.7)."""
    day = choose({"": "Today", "tomorrow": "Tomorrow", "other": "Another day"}, "Which day?")
    if day is None:
        return ""
    if day == "other":
        day = ask("Which day?", "next monday")
        if day is None:
            return ""
    try:
        plan = session.call("plan", date=day) if day else session.call("plan")
    except LumennaError as error:
        return error.message
    blocks = [block for block in plan.get("blocks", []) if block["kind"] == "work"]
    if not blocks:
        return f"{plan.get('date', 'That day')} has no work blocks to put it in"
    block = choose(
        {b["id"]: f"{b['title']}, {b['start']} to {b['end']}" for b in blocks}, "Put it in"
    )
    if block is None:
        return ""
    return session.write("assign", task=identifier, block=block, date=plan["date"])


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
