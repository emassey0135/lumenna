"""Tasks: lists, what can be done to one, capture, and the trash.

Every list is one of the app's places, shown as the app lists it: its rows, each worded by the
core in this device's clock, folded as the app folds them, the row in hand the app's. Every
field is sent as text and read by the core, so "next friday" means here exactly what it means
in the quick-add line. The dialog library has its own `request_date`, with its own parsing —
using it would put a second date parser in the product, and two parsers would disagree.
"""

from __future__ import annotations

from BTSpeak import dialogs

import actions
import options
from actions import K, L
from core import refused
from rows import Row, items
from session import Command, Session, choose, empty_then, heading, live_menu, screen


# ---------------------------------------------------------------------------------------
# Lists
# ---------------------------------------------------------------------------------------


def undo_commands(session: Session) -> list[Command]:
    """Undo and redo, on every screen's main menu, with the same keys everywhere."""
    return [
        Command("Undo", lambda _: session.send(L.ShellEventUndo()), key="u"),
        Command("Redo", lambda _: session.send(L.ShellEventRedo()), key="y"),
    ]


def list_rows(session: Session) -> list[Row]:
    """The rows of the list the app shows, not folded away, each with its line."""
    view, lines = session.view(), session.lines()
    if view.list is None:
        return []
    rows = []
    offered = view.list.offered
    for index, item in enumerate(view.list.items):
        if item.hidden:
            continue
        line = lines.list[index] if index < len(lines.list) else None
        # A list's rows name their actions in short, each held once in the list.
        rows.append(Row(
            item.row.id, line.text if line else item.row.title, [offered[at] for at in item.actions], item.row,
            level=item.level, expanded=item.row.expanded, title=item.row.title,
            braille=line.braille if line else "",
        ))
    return rows


def readback(session: Session) -> str:
    """What the list shown says of itself: the query understood, what it found, its notices."""
    return session.lines().readback


def task_list(
    session: Session, place=None, title: str = "Tasks", query: str | None = None, more: list | None = None,
    main=None,
) -> str:
    """One of the app's places as a foldable list of tasks, with `query` typed over it.

    Enter shows a task; its context menu (M-Chord with Dot 7) has everything else, and the main
    menu (M-Chord) adds a task, starting the line as the app says this place does — `#Work ` in
    a project's list, so a task added there lands there. `more` adds to the main menu: a
    project's list can add a project inside it.
    """
    place = place or L.PlaceTasks()
    state = {"heading": title}

    def build():
        session.show(place, query)
        rows = list_rows(session)
        state["heading"] = heading(title, readback(session), bool(rows))
        return items(rows, fold=lambda row, open_: actions.fold(session, row, open_))

    with screen("lumenna-tasks"):
        live_menu(
            session, build, lambda: state["heading"],
            main=main or (lambda row: show(session, row)),
            context=lambda rows: task_commands(session, rows),
            app=[
                Command("Add a task", lambda _: add_task(session, session.view().quick_add), key="a"),
                Command("Search or filter", lambda _: query_tasks(session), key="/"),
                *(more or []),
                *undo_commands(session),
            ],
            empty=empty_then(lambda: readback(session), L.press_to_add("a")),
            app_title=L.menu_of(title),
            focus=lambda: session.view().focus,
        )
    return ""


def query_tasks(session: Session) -> str:
    """A filter typed now rather than saved; `search: words` looks through titles and notes."""
    words = L.quick_add_words(True)
    query = assisted_input(session, actions.prompt(words.filter, words.filter_hint), L.Syntax.FILTER,
                           history_key="lumenna-filter")
    if not query:
        return ""
    return task_list(session, L.PlaceTasks(), "Query", query=query)


def task_commands(session: Session, rows) -> list[Command]:
    """What can be done to a task: the app's actions for it, then its details."""
    return actions.commands(session, rows, after=[Command(L.details_words(True).title, lambda row: show(session, row))])


def history(session: Session) -> str:
    """What was done over the last two weeks, newest first, each a task's row: Enter shows its
    details."""
    return task_list(session, L.PlaceHistory(), "History")


def trash(session: Session) -> str:
    """Deleted tasks: Enter puts one back; deleting one from the trash is its context menu's,
    or the delete keys'."""
    return task_list(
        session, L.PlaceTrash(), "Trash",
        main=lambda row: actions.press(session, "restore-task", row),
    )


# ---------------------------------------------------------------------------------------
# One task
# ---------------------------------------------------------------------------------------


def details_of(session: Session, task: str):
    """The app's details of `task`, opened."""
    session.send(L.ShellEventDetails(L.DetailsEventShow(task)))
    return session.view().details


def show(session: Session, row) -> str:
    """A task's details, as lines to pan through."""
    details = details_of(session, row.key)
    if details is None:
        return ""
    return task_details(details, session.lines().details)


def task_details(details, about=()) -> str:
    """Everything about one task, as lines to pan through: its title, each field of the task
    form that holds something, named as the form names it and said as the form offers it, then
    what the core says about it besides (`about`, the details' `lines`: its state, a rule it
    repeats by, what it waits for, its history), each its label then what it says."""
    task = details.task
    lines = [task.title]
    for field in details.form:
        value = str(getattr(details.fields, field.key, "") or "")
        if field.key == "title" or not value:
            continue
        options = details.projects if field.key == "project" else field.options
        named = {option.id: option.title for option in options}
        lines.append(f"{field.label}: {named.get(value, value)}")
    lines.extend(line.text for line in about)
    dialogs.view_lines(lines, wrap=True)
    return ""


def edit_task(session: Session, task: str) -> str:
    """The task form, the app's details: each field as the app starts it, and each one
    changed sent as typed, then saved — the app sends only what changed, so a concurrent edit
    to another field on another device is not overwritten with what this form showed."""
    details = details_of(session, task)
    if details is None:
        return ""
    before = details.fields
    fields = []
    for field in details.form:
        value = str(getattr(before, field.key))
        if field.key == "project":
            # Chosen from the projects the app offers, by name, each with its level where it
            # changes, since a list says nothing of indentation.
            levels = L.levels([choice.depth for choice in details.projects])
            choices = {
                choice.id: f"{choice.title}, {level}" if level else choice.title
                for choice, level in zip(details.projects, levels)
            }
            fields.append(form_field(field, value, choices=choices))
            continue
        fields.append(form_field(field, value))
    answers = dialogs.request_form(fields)
    if answers is None:
        return ""
    for field in details.form:
        typed = answers.get(field.key)
        if typed is not None and typed != str(getattr(before, field.key)):
            session.send(L.ShellEventDetails(L.DetailsEventField(field.key, str(typed))))
    return session.send(L.ShellEventDetails(L.DetailsEventSave()))


def form_field(field, value: str, choices=None) -> dialogs.InputField:
    """One of the core's form fields as the device's form takes it: a choice where it has
    options, else a line, or several lines where it may run to them; named, and hinted, in the
    core's words."""
    offered = choices or {option.id: option.title for option in field.options}
    if offered:
        return dialogs.InputField(
            key=field.key, prompt=field.label, field_type="choice", choices=offered, default_text=value,
        )
    return dialogs.InputField(
        key=field.key, prompt=field.label, default_text=value,
        field_type="multiline" if field.kind == L.FieldKind.LINES else "text", required=field.key == "title",
        format_hint=field.hint,
    )


def open_task_form(session: Session, action, row=None) -> str:
    """The task form, wherever an action asks for it: a task's Edit Details, a sitting's
    Edit Task Details."""
    return edit_task(session, action.target)


actions.FORMS[(L.Subject.TASK, K.EDIT)] = open_task_form
actions.FORMS[(L.Subject.TASK, K.EDIT_TASK)] = open_task_form


# ---------------------------------------------------------------------------------------
# Capture
# ---------------------------------------------------------------------------------------


def add_task(session: Session, prefix: str = "", refusal: str = "") -> str:
    """Capture, in the quick-add grammar: the app's quick-add line.

    Where the person asked for it, the line is read back before it is added. A line the core
    refuses — an unknown project — is asked again as typed, the app's sentence first, rather
    than retyped on a braille keyboard.
    """
    text = assisted_input(session, actions.prompt(refusal, L.quick_add_words(True).label), L.Syntax.QUICK_ADD,
                          history_key="lumenna-add", default=prefix)
    if not text or not text.strip():
        return ""
    session.send(
        L.ShellEventTyping(L.TypingOpen()),
        L.ShellEventTyping(L.TypingText(text, len(text.encode("utf-8")))),
    )
    line = session.view().line
    if line is not None and options.get(options.READ_BACK, False):
        words = L.read_back_words(True)
        answer = choose({"add": words.add, "change": words.change}, L.add_it(line.readback))
        if answer != "add":
            session.send(L.ShellEventTyping(L.TypingClose()))
            return "" if answer is None else add_task(session, prefix=text)
    said = session.send(L.ShellEventQuickAdd(text))
    why = session.view().add_refused
    if why:
        # The refusal, said once: as the question asked again.
        session.send(L.ShellEventTyping(L.TypingClose()))
        return add_task(session, prefix=text, refusal=why)
    return said


def char_offset(text: str, byte_offset: int) -> int:
    """A UTF-8 byte offset from the core, as an index into a Python string.

    The core counts bytes, as Rust strings do, and Python counts code points. They agree only
    until the first accented letter or dash; after that, slicing with the core's number would
    splice the completion into the wrong place.
    """
    prefix = text.encode("utf-8")[:byte_offset]
    return len(prefix.decode("utf-8", errors="ignore"))


# Control-L, typed on the braille keyboard as dots 7-8 with L: free in every dialog and in the
# device's own keys, and L for list.
COMPLETE_KEY = "\x0c"


def completing_key(session: Session, syntax) -> dict:
    """The input dialog's hotkey that completes at the cursor, inside the dialog, as every app
    completes on Down: what fits there, chosen from a menu, put in the span the core says it
    replaces. Never by itself, on `#` or `@` or anywhere."""

    def complete(dialog):
        try:
            text = dialog.field_as_text()
            # In literary braille the field holds braille, whose cells are not the text's
            # characters: there it completes the line's end.
            cursor = dialog.cursor() if dialog.input_grade == 0 else len(text)
            found = session.app.complete(text, len(text[:cursor].encode("utf-8")), syntax)
        except Exception:  # noqa: BLE001 - a line that cannot be completed is still a line
            return L.quick_add_words(True).nothing_to_complete
        if not found.candidates:
            return found.announcement or L.quick_add_words(True).nothing_to_complete
        start, end = char_offset(text, found.start), char_offset(text, found.end)
        choice = choose({c.text: L.candidate_line(c) for c in found.candidates},
                        found.announcement or L.quick_add_words(True).show_completions)
        if choice is None:
            return None
        line = text[:start] + choice + text[end:]
        braille = getattr(dialogs, "braille", None)
        dialog.field = line if dialog.input_grade == 0 or braille is None else braille.translateText(line)
        dialog.selected = False
        dialog.update_content()
        dialog.cursor(absolute=start + len(choice) if dialog.input_grade == 0 else len(dialog.field))
        return None

    return {COMPLETE_KEY: complete, ord(COMPLETE_KEY): complete}


def assisted_input(session: Session, prompt: str, syntax, history_key: str | None = None,
                   default: str = "") -> str | None:
    """Text entry that completes: Control-L (dots 7-8 with L) inside the dialog offers what fits
    at the cursor (`completing_key`), and once the line is entered a last pass offers what would
    finish its last word when it looks like a name begun and not finished: worth having for `#wo`
    when you cannot remember whether the project is Work or Workshop.
    """
    text = dialogs.request_input(prompt, default_text=default, history_key=history_key,
                                 hotkeys=completing_key(session, syntax))
    if not text:
        return text
    try:
        found = session.app.complete(text, len(text.encode("utf-8")), syntax)
    except Exception:  # noqa: BLE001 - a line that cannot be completed is still a line
        return text
    start = char_offset(text, found.start)
    end = char_offset(text, found.end)
    partial = text[start:end]
    candidates = found.candidates
    if not partial or not candidates:
        return text
    if any(candidate.text.lower() == partial.lower() for candidate in candidates):
        return text
    choice = choose(
        {candidate.text: L.candidate_line(candidate) for candidate in candidates},
        found.announcement or L.quick_add_words(True).show_completions,
    )
    if choice is None:
        return text
    return text[:start] + choice + text[end:]


def failed(error: Exception) -> str:
    """The core's sentence for a refusal, to say."""
    return refused(error)
