"""Tasks: lists, what can be done to one, capture, and the trash.

Every field is sent as text and read by core. `due` and `repeat` go through the same date
grammar the quick-add line uses, so "next friday" means here exactly what it means there. The
dialog library has its own `request_date`, with its own parsing — using it would put a second
date parser in the product, and two parsers would disagree.
"""

from __future__ import annotations

from BTSpeak import dialogs

import actions
import options
from client import LumennaError
from rows import Tree
from session import Command, Session, choose, live_menu, screen, spoken


# ---------------------------------------------------------------------------------------
# Lists
# ---------------------------------------------------------------------------------------


def undo_commands(session: Session) -> list[Command]:
    """Undo and redo, on every screen's main menu, with the same keys everywhere."""
    return [
        Command("Undo", lambda _: session.write("undo"), key="u"),
        Command("Redo", lambda _: session.write("redo"), key="y"),
    ]


def task_list(
    session: Session, query: str = "", title: str = "Tasks", prefix: str = "", more: list | None = None
) -> str:
    """A filtered list of tasks, as a foldable tree.

    Enter shows a task; its context menu (M-Chord with Dot 7) has everything else, and the main
    menu (M-Chord) adds a task. `prefix` starts the quick-add line — `#Work ` in a project's
    list, so a task added there lands there, as on the phone. `more` adds to the main menu:
    a project's list can add a project inside it.
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
            # A mis-parsed filter shows wrong results silently, and wrong results are
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
            context=lambda rows: task_commands(session, rows),
            app=[
                Command("Add a task", lambda _: add_task(session, prefix), key="a"),
                Command("Search or filter", lambda _: query_tasks(session), key="/"),
                *(more or []),
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


def task_commands(session: Session, rows) -> list[Command]:
    """What can be done to a task: the core's actions for it, then its details."""
    return actions.commands(session, rows, after=[Command("Details", lambda row: show(session, row))])


def show(session: Session, row: dict) -> str:
    """A task's details, as lines to pan through."""
    try:
        return task_details(session.call("task.show", id=row["id"]), priorities(session))
    except LumennaError as error:
        return error.message


def trash(session: Session) -> str:
    """Deleted tasks: Enter puts one back; deleting one from the trash is its context menu's,
    or the delete keys'."""
    state = {"heading": "Trash"}

    def build():
        result = session.call("task.list", query="deleted")
        state["heading"] = f"Trash, {result.get('announcement', '')}"
        return Tree(result.get("rows", [])).items()

    with screen("lumenna-tasks"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: actions.run_kind(session, row, "restore"),
            context=lambda rows: actions.commands(session, rows),
            app=undo_commands(session),
            empty="The trash is empty.",
            app_title="Trash menu",
        )
    return ""


# ---------------------------------------------------------------------------------------
# One task
# ---------------------------------------------------------------------------------------


def priorities(session: Session) -> dict:
    """The priorities as the core words them, by number."""
    return {c["id"]: c["title"] for c in session.call("form.priorities").get("value", [])}


def task_details(task: dict, named: dict | None = None) -> str:
    """Everything about one task, as lines to pan through.

    A detail view is where the near-universal states are worth having, so this shows the full
    set rather than the notable ones a list line carries. `named` words the priority.
    """
    lines = [task["title"]]
    if task.get("project"):
        lines.append(f"project: {task['project']}")
    if task.get("priority", 4) != 4:
        lines.append((named or {}).get(str(task["priority"]), f"priority: {task['priority']}"))
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


def edit_task(session: Session, task: dict) -> str:
    """The task form: its fields as the core says a form starts from them, sending what the
    core says changed — so a concurrent edit to another field on another device is not
    overwritten with what this form happened to show."""
    try:
        before = session.call("form.task_fields", task=task)["value"]
    except LumennaError as error:
        return error.message
    projects = [row["title"] for row in session.call("project.list").get("rows", [])]
    fields = [
        dialogs.InputField(key="title", prompt="Title", default_text=before["title"], required=True),
        dialogs.InputField(
            key="due", prompt="Due", default_text=before["due"],
            format_hint="a date such as tomorrow, next friday or 2026-12-01, empty for none",
        ),
        dialogs.InputField(
            key="repeat", prompt="Repeats", default_text=before["repeat"],
            format_hint="such as every monday or every! 2 weeks, empty for no repetition",
        ),
        dialogs.InputField(
            key="priority", prompt="Priority", field_type="choice", choices=priorities(session),
            default_text=str(before["priority"]),
        ),
        dialogs.InputField(
            key="estimate", prompt="Estimate", default_text=before["estimate"],
            format_hint="such as 45m or 1h30m, empty for none",
        ),
    ]
    if before["project"] in projects:
        # A project this device does not know, perhaps not synced yet, is not offered: the
        # field stays as it was, so it is left where it is.
        fields.append(
            dialogs.InputField(
                key="project", prompt="Project", field_type="choice", choices=projects,
                default_text=before["project"],
            )
        )
    fields += [
        dialogs.InputField(
            key="labels", prompt="Labels", default_text=before["labels"],
            format_hint="names separated by commas; a new name becomes a label",
        ),
        dialogs.InputField(
            key="notes", prompt="Notes", field_type="multiline", default_text=before["notes"],
        ),
    ]
    answers = dialogs.request_form(fields)
    if answers is None:
        return ""
    after = dict(before)
    after.update({key: value for key, value in answers.items() if key in before})
    after["priority"] = int(after["priority"])
    try:
        edit = session.call("form.task_edit", task=task, fields=after)["value"]
    except LumennaError as error:
        return error.message
    if not edit:
        return "Nothing changed"
    return session.write("task.edit", id=task["id"], **{k: v for k, v in edit.items() if v is not None})


def open_task_form(session: Session, action: dict, row=None) -> str:
    """The task form, wherever an action asks for it: a task's Edit Details, a sitting's
    Edit Task Details."""
    try:
        task = session.call("task.show", id=action["target"])
    except LumennaError as error:
        return error.message
    return edit_task(session, task)


actions.FORMS[("task", "edit")] = open_task_form
actions.FORMS[("task", "edit_task")] = open_task_form


# ---------------------------------------------------------------------------------------
# Capture
# ---------------------------------------------------------------------------------------


def add_task(session: Session, prefix: str = "") -> str:
    """Capture, in the quick-add grammar.

    The line is previewed before it is written, because an unknown project is an error and an
    unknown label is a new label — and being told which of those just happened *after* the
    fact is not the same thing.
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

    if options.get(options.READ_BACK, False):
        said = ". ".join(
            [preview.get("announcement", "")]
            + [diagnostic["message"] for diagnostic in preview.get("diagnostics", [])]
        )
        answer = choose({"add": "Add it", "change": "Change it"}, f"{said}. Add it?")
        if answer is None:
            return ""
        if answer == "change":
            return add_task(session, prefix=text)

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
    name that was started and not finished. It is the honest version of completion on this
    toolkit: worth having for `#wo` when you cannot remember whether the project is Work
    or Workshop, and not pretending to be inline.

    Making it inline means subclassing `InputDialog` to bind a key to `complete`, which is
    the same shape of change `rows.py` describes for the menu's speech and braille, and worth
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
