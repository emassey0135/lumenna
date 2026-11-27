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
from actions import L
import core
from core import refused
from rows import Row, items
from session import Command, Session, choose, command_title, empty_then, heading, live_menu, screen


# ---------------------------------------------------------------------------------------
# Lists
# ---------------------------------------------------------------------------------------


def undo_commands(session: Session) -> list[Command]:
    """Undo and redo, on the main menu of a screen that is no place (a reminder list, a settings
    page), as the app runs them, with the letters the places give them."""
    return [
        Command(command_title(command), lambda _, command=command: actions.key(session, command),
                key=L.command_letter(command) or "", through=True)
        for command in ("undo", "redo")
    ]


def own_commands(session: Session) -> dict:
    """What this app runs itself of a screen's own actions, by id: the filter typed over the list,
    Key help, About and Reminders due, which the core leaves to the shell, and Sync now, whose
    answer is waited for under the device's activity sound."""
    import menus  # here: menus imports this module
    import preferences
    import reminder_lists

    return {
        "filter": query_tasks,
        "keyboard-help": menus.key_help,
        "about": menus.about,
        "reminders-due": reminder_lists.due,
        "sync-now": preferences.sync_now,
    }


def place_commands(session: Session, skip=()) -> list[Command]:
    """The place shown's own actions, the core's (`screen_actions`), as its main menu."""
    return actions.screen_commands(session, own_commands(session), skip)


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


def task_list(session: Session, place=None, title: str = "", query: str | None = None, main=None) -> str:
    """One of the app's places as a foldable list of tasks, with `query` typed over it.

    Enter opens a task's form; its context menu (M-Chord with Dot 7) has everything else, and the main
    menu (M-Chord) is what the place offers of its own, the core's: New task, starting the line as
    the app says this place does (`#Work ` in a project's list, so a task added there lands
    there), the filter, New project inside on a project's list, undo and redo.
    """
    place = place or L.PlaceTasks()
    title = title or L.place_title(place)
    state = {"heading": title}

    def build():
        session.show(place, query)
        rows = list_rows(session)
        state["heading"] = heading(title, readback(session), bool(rows))
        return items(rows, fold=lambda row, open_: actions.fold(session, row, open_))

    with screen("lumenna-tasks"):
        live_menu(
            session, build, lambda: state["heading"],
            # Enter is `Key("open")`, as on every row: a task's form, a trashed task restored, a
            # project the review lists opened, which this screen leaves for (`opened`).
            main=main or (lambda row: opened(session, place, actions.press(session, "open", row))),
            context=lambda rows: task_commands(session, rows),
            app=lambda: place_commands(session),
            bar=(place, False),
            empty=empty_then(lambda: readback(session), L.press_to_add(L.command_letter("new-task"))),
            app_title=L.menu_of(title),
            focus=lambda: session.view().focus,
        )
    return ""


def opened(session: Session, place, said: str) -> str:
    """What Enter said, and, where it took the app to another place (a project the review lists,
    `OpenProject`), this screen closed for Home's loop to show that one (`session.moved_on`)."""
    if session.view().place != place:
        session.moved_on = True
    return said


def query_tasks(session: Session) -> str:
    """A filter typed now rather than saved; `search: words` looks through titles and notes."""
    words = core.words().quick_add
    query = assisted_input(session, actions.prompt(words.filter, words.filter_hint), L.Syntax.FILTER,
                           history_key="lumenna-filter")
    if not query:
        return ""
    return task_list(session, L.PlaceTasks(), core.words().query, query=query)


def task_commands(session: Session, rows) -> list[Command]:
    """What can be done to a task: the app's actions for it, then its details."""
    return actions.commands(session, rows, after=[Command(core.words().details.title, lambda row: show(session, row))])


def history(session: Session) -> str:
    """What was done over the last two weeks, newest first, each a task's row: Enter shows its
    details."""
    return task_list(session, L.PlaceHistory())


def trash(session: Session) -> str:
    """Deleted tasks: Enter puts one back, as Enter does on every platform (`Key("open")`);
    deleting one from the trash is its context menu's, or the delete keys'."""
    return task_list(session, L.PlaceTrash())


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
    form that holds something, as the core reads it (`DetailsView.field_lines`: "Priority:
    High"), then what the core says about it besides (`about`, the details' `lines`: its state,
    a rule it repeats by, what it waits for, its history), each its label then what it says."""
    lines = [details.task.title, *details.field_lines]
    lines.extend(line.text for line in about)
    dialogs.view_lines(lines, wrap=True)
    return ""


# ---------------------------------------------------------------------------------------
# Capture
# ---------------------------------------------------------------------------------------


def add_task(session: Session, prefix=None, refusal: str = "") -> str:
    """Capture, in the quick-add grammar: the app's quick-add line, opened as New task opens it
    (`Key("new-task")`, starting as the place shown starts it) unless it is open already, and
    drawn from the app's own description of it (`ScreenView::dialog`, `quick-add`).

    Where the person asked for it, the line is read back before it is added. A line the core
    refuses — an unknown project — is asked again as typed, the app's sentence first, rather
    than retyped on a braille keyboard. `prefix` is unused: the line starts as the app has it.
    """
    import forms

    if session.view().line is None:
        session.send(L.ShellEventKey("new-task"))
    dialog = forms.opened(session)
    if dialog is None or dialog.id != "quick-add":
        return ""
    line = next(control for control in dialog.controls if control.key == "line")
    refusal = refusal or forms.text_of(dialog, "refused")
    session.typing = True
    try:
        text = assisted_input(session, actions.prompt(refusal, line.label), L.Syntax.QUICK_ADD,
                              history_key="lumenna-add", default=line.value)
    finally:
        session.typing = False
    if not text or not text.strip():
        # Left: the line the app opened is put away.
        return forms.cancel(session, dialog)
    forms.send(session, line, text, L.ControlMoveSet())
    dialog = forms.opened(session)
    readback = forms.text_of(dialog, "readback") if dialog is not None else ""
    if readback and options.get(options.READ_BACK, False):
        words = core.words().read_back
        answer = choose({"add": words.add, "change": words.change}, L.add_it(readback))
        if answer != "add":
            return forms.cancel(session, dialog) if answer is None else add_task(session)
    add = forms.default_button(dialog) if dialog is not None else None
    said = forms.send(session, add, text) if add is not None else ""
    if session.view().line is not None and session.view().add_refused:
        # Refused, the line stays open as typed: asked again, the refusal said once, as its question.
        return add_task(session, refusal=session.view().add_refused)
    return said


def char_offset(text: str, byte_offset: int) -> int:
    """A UTF-8 byte offset from the core, as an index into a Python string: where the cursor goes
    after a completion is put in. The core counts bytes, as Rust strings do, and Python counts code
    points; they agree only until the first accented letter or dash. Putting the choice in is the
    core's (`put_in`), which takes the bytes as they are.
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
            return core.words().quick_add.nothing_to_complete
        if not found.candidates:
            return found.announcement or core.words().quick_add.nothing_to_complete
        choice = choose({c.text: L.candidate_line(c) for c in found.candidates},
                        found.announcement or core.words().quick_add.show_completions)
        if choice is None:
            return None
        line = L.put_in(text, found.start, found.end, choice)
        start = char_offset(text, found.start)
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
    # Whether the line ends in a name begun that something would finish is the core's.
    if not L.ends_unfinished(text, found):
        return text
    choice = choose(
        {candidate.text: L.candidate_line(candidate) for candidate in found.candidates},
        found.announcement or core.words().quick_add.show_completions,
    )
    if choice is None:
        return text
    return L.put_in(text, found.start, found.end, choice)


def failed(error: Exception) -> str:
    """The core's sentence for a refusal, to say."""
    return refused(error)
