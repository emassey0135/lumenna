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
from rows import Tree, clock
from session import Command, Session, choose, empty_then, heading as list_heading, live_menu, screen, spoken


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
        said = [list_heading(heading, result)]
        for unresolved in (result.get("query") or {}).get("unresolved", []):
            suggestion = unresolved.get("suggestion")
            hint = f", did you mean {suggestion}?" if suggestion else ""
            said.append(f"no {unresolved['kind']} called {unresolved['name']}{hint}")
        state["heading"] = ", ".join(part for part in said if part)
        state["empty"] = result.get("empty", "")
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
            empty=empty_then(state, "Press a to add one."),
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
        task = session.call("task.show", id=row["id"])
    except LumennaError as error:
        return error.message
    return task_details(task, priorities(session), {f["key"]: f["label"] for f in session.words("form.task_form")})


def trash(session: Session) -> str:
    """Deleted tasks: Enter puts one back; deleting one from the trash is its context menu's,
    or the delete keys'."""
    state = {"heading": "Trash"}

    def build():
        result = session.call("task.list", query="deleted")
        state["heading"] = list_heading("Trash", result)
        state["empty"] = result.get("empty", "")
        return Tree(result.get("rows", [])).items()

    with screen("lumenna-tasks"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: actions.run_kind(session, row, "restore"),
            context=lambda rows: actions.commands(session, rows),
            app=undo_commands(session),
            empty=empty_then(state),
            app_title="Trash menu",
        )
    return ""


# ---------------------------------------------------------------------------------------
# One task
# ---------------------------------------------------------------------------------------


def priorities(session: Session) -> dict:
    """The priorities as the core words them, by number."""
    return {c["id"]: c["title"] for c in session.call("form.priorities").get("value", [])}


def task_details(task: dict, named: dict | None = None, labels: dict | None = None) -> str:
    """Everything about one task, as lines to pan through, each named as the task form
    names its field (`labels`, from `form.task_form`).

    A detail view is where the near-universal states are worth having, so this shows the full
    set rather than the notable ones a list line carries. `named` words the priority.
    """
    labels = labels or {}

    def line(key: str, value: str) -> str:
        return f"{labels.get(key, key.capitalize())}: {value}"

    lines = [task["title"]]
    if task.get("project"):
        lines.append(line("project", task["project"]))
    if task.get("priority", 4) != 4:
        lines.append((named or {}).get(str(task["priority"]), line("priority", str(task["priority"]))))
    if task.get("due"):
        due = task["due"]
        if task.get("due_time"):
            due = f"{due} at {clock(task['due_time'])}"
        lines.append(line("due", due))
    if task.get("repetition"):
        lines.append(line("repeat", task["repetition"]))
    elif task.get("recurrence"):
        lines.append(line("repeat", f"by the rule {task['recurrence']}"))
    if task.get("estimate_mins"):
        lines.append(line("estimate", f"{task['estimate_mins']} minutes"))
    if task.get("labels"):
        lines.append(line("labels", ", ".join(task["labels"])))
    for dependency in task.get("depends", []):
        lines.append(f"Waits for: {dependency['title']}")
    if task.get("state"):
        lines.append("State: " + ", ".join(task["state"]))
    if task.get("notes"):
        lines.append(line("notes", task["notes"]))
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
    projects = project_choices(session)
    fields = []
    for field in session.words("form.task_form"):
        key = field["key"]
        if key == "project":
            # Chosen from the projects the core offers, by id. An archived one is offered to no
            # task, but the one this task is in stays on its list, or the field would lose it.
            choices = dict(projects)
            if before["project"] and before["project"] not in choices:
                choices[before["project"]] = before["project"]
            fields.append(form_field(field, str(before[key]), choices=choices))
            continue
        fields.append(form_field(field, str(before[key])))
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


def project_choices(session: Session) -> dict:
    """The projects the task form offers (`form.project_options`), in tree order, as the
    device's list takes them: name to what is said, with the level where a project sits
    under another, since a list says nothing of indentation."""
    return {
        option["id"]: option["title"] if not option.get("depth") else f"{option['title']}, level {option['depth'] + 1}"
        for option in session.call("form.project_options").get("value", [])
    }


def form_field(field: dict, value: str, choices=None) -> dialogs.InputField:
    """One of the core's form fields (`form.task_form`, `form.block_form`) as the device's
    form takes it: a choice where it has options, else a line, or several lines where it
    may run to them; named, and hinted, in the core's words."""
    options = choices or {o["id"]: o["title"] for o in field.get("options", [])}
    kind = field["kind"]
    if options:
        return dialogs.InputField(
            key=field["key"], prompt=field["label"], field_type="choice", choices=options, default_text=value,
        )
    multiline = kind == "lines"
    return dialogs.InputField(
        key=field["key"], prompt=field["label"], default_text=value,
        field_type="multiline" if multiline else "text", required=field["key"] == "title",
        format_hint=field.get("hint", ""),
    )


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
