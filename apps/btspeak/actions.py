"""What can be done to a row: the core's, offered as the device offers things.

Every listed record carries its `actions` — which apply now, in order, under their spoken
names, each with the question it asks. This file turns them into the row's context menu
(M-Chord with Dot 7), its letters and its delete keys, asks each question with the device's
own dialogs, and hands the answer back through `act`. It decides none of it: which actions a
row has, what deleting asks, and what a picker offers are the core's, the same on every app.

A letter stands for a kind of action (`KEYS`), so `c` is Mark Done on a task that is open
and Mark Not Done on one that is finished, and does nothing on a row that offers neither.
The forms are this app's own (`Question::Form`): the task and block forms, and the new
filter's, registered in `FORMS` by the screens that have them.
"""

from __future__ import annotations

import datetime

from BTSpeak import dialogs

from client import LumennaError
from rows import clock
from session import Command, Session, choose, confirm


#: The letter for each kind of action: the same letter for the same thing on every screen.
#: Two kinds share a letter only where no row offers both.
KEYS = {
    "mark_done": "c",
    "mark_not_done": "c",
    "edit": "e",
    "edit_task": "e",
    "put_in_block": "b",
    "move_to_project": "m",
    "make_subtask_of": "s",
    "move_to_top_level": "t",
    "wait_for": "w",
    "stop_waiting": "n",
    "restore": "r",
    "assign_task": "i",
    "cancel_day": "x",
    "restore_day": "o",
    "start_timer": "s",
    "pause_timer": "s",
    "resume_timer": "s",
    "stop_timer": "t",
    "planned_length": "l",
    "log_minutes": "m",
    "add_block": "a",
    "rename": "r",
    "new_inside": "n",
    "move_under": "m",
    "move_up": ",",
    "move_down": ".",
    "weight": "w",
    "merge_into": "m",
    "colour": "c",
    "change_query": "q",
}

#: The kinds the delete keys do — Control-D and the D chord.
DELETES = {"delete", "delete_for_good", "unassign", "unpair"}

#: How each form opens: `(subject, kind)` to a function of the session, the action and the
#: row it was offered on. The screens that have the forms fill this in.
FORMS: dict = {}


def spoken_day(iso: str) -> str:
    """`Sunday 4 October 2026`: the day as a person says it, rather than as ISO."""
    try:
        day = datetime.date.fromisoformat(iso)
    except ValueError:
        return iso
    return f"{day:%A} {day.day} {day:%B} {day.year}"


def choice_line(choice: dict) -> str:
    """Something to choose, as a line: a block's day and times, else its title and detail."""
    if choice.get("date"):
        return f"{spoken_day(choice['date'])}, {clock(choice['start'])} to {clock(choice['end'])}, {choice['title']}"
    return ", ".join(part for part in (choice["title"], choice.get("detail")) if part)


def named(action: dict) -> str:
    """What an action is called here: its sentence-case name, as the device's own menus
    word things and as braille wants, a capital costing a cell."""
    return action.get("sentence") or action["title"]


def prompt(*parts: str) -> str:
    """Parts of a question as one prompt, each sentence ended once."""
    return ". ".join(part.strip().rstrip(".") for part in parts if part and part.strip())


def act(session: Session, action: dict, row=None) -> str:
    """Asks `action`'s question with the device's dialogs, then does it; what to say.

    A cancelled question does nothing and says nothing. The core's refusals are its own
    sentences, said as they come.
    """
    question = action.get("question", {})
    ask = question.get("ask")
    if ask == "form":
        form = FORMS.get((action["subject"], action["kind"]))
        if form is None:
            return f"{named(action)} has no form in this app yet"
        return form(session, action, row)
    if ask == "immediate":
        answer = {"answer": "yes"}
    elif ask == "confirm":
        # Cancel is the default, as for anything that cannot be taken back lightly.
        if not confirm(f"{question['title']} {question['message']}"):
            return ""
        answer = {"answer": "yes"}
    elif ask == "text":
        text = ask_text(session, action, question)
        if text is None:
            return ""
        answer = {"answer": "text", "text": text}
    elif ask == "pick":
        answer = ask_pick(session, action, question)
        if isinstance(answer, str):
            return answer
    elif ask == "choose":
        picked = choose(
            {a["id"]: session.sentence(a["title"]) for a in question.get("answers", [])},
            prompt(question.get("title", ""), question.get("message", "")),
        )
        if picked is None:
            return ""
        answer = {"answer": "picked", "id": picked}
    else:
        return f"This app cannot ask that question yet; update it to {named(action)}"
    return session.write("act", action=action, answer=answer)


def text_prompt(question: dict) -> str:
    """What a text question says before its line: its title in sentence case, then its hint."""
    return prompt(question.get("sentence") or question.get("title", ""), question.get("hint", ""))


def ask_line(session: Session, method: str) -> str | None:
    """The line one of the app's own questions asks for, in the core's words (`form.go_to_day`,
    `form.length`); None if cancelled."""
    question = session.words(method)
    return dialogs.request_input(text_prompt(question), default_text=question.get("initial", ""))


def ask_text(session: Session, action: dict, question: dict) -> str | None:
    """The line a text question asks for, sent as typed even when empty; None if cancelled."""
    said = text_prompt(question)
    initial = question.get("initial", "")
    if action["kind"] == "change_query":
        import tasks  # here, since tasks imports this module

        return tasks.assisted_input(session, said, "filter", history_key="lumenna-filter", default=initial)
    return dialogs.request_input(said, default_text=initial)


#: The choice that asks for another day's work blocks, beside the week's.
ANOTHER_DAY = "\0another day"


def ask_pick(session: Session, action: dict, question: dict):
    """The answer to a pick, or what to say instead: why nothing is offered, or nothing at
    all when the person cancelled."""
    try:
        offered = session.call("choices", action=action)
    except LumennaError as error:
        return error.message
    choices = offered.get("choices", [])
    if not choices:
        return offered.get("announcement", "")
    options = {c["id"]: choice_line(c) for c in choices}
    if action["kind"] == "put_in_block":
        # The core offers this week's; a block further off is asked for by its day.
        options[ANOTHER_DAY] = "Another day"
    picked = choose(options, question.get("title") or named(action))
    if picked is None:
        return ""
    if picked == ANOTHER_DAY:
        picked = another_day(session)
        if not picked or picked.startswith("\0"):
            return picked[1:] if picked else ""
    length = None
    if question.get("length"):
        length = ask_line(session, "form.length")
        if length is None:
            return ""
    return {"answer": "picked", "id": picked, "length": length}


def another_day(session: Session) -> str:
    """A work block on a day named, from what the core offers for it: its identifier, empty
    if cancelled, or a sentence to say marked with a leading NUL."""
    when = ask_line(session, "form.go_to_day")
    if not when:
        return ""
    try:
        offered = session.call("choices", **{"from": when, "days": 1})
    except LumennaError as error:
        return "\0" + error.message
    choices = offered.get("choices", [])
    if not choices:
        return "\0" + offered.get("announcement", "")
    return choose({c["id"]: choice_line(c) for c in choices}, "Put it in") or ""


def of_kind(row, *kinds: str) -> list[dict]:
    """The actions `row` offers of any of `kinds`, in its order."""
    return [a for a in (row or {}).get("actions", []) if a["kind"] in kinds]


def run_kind(session: Session, row, *kinds: str) -> str:
    """Does the row's action of one of `kinds`; where it offers several, asks which."""
    matching = of_kind(row, *kinds)
    if not matching:
        return ""
    if len(matching) > 1:
        index = choose({i: named(a) for i, a in enumerate(matching)}, "Which?")
        if index is None:
            return ""
        return act(session, matching[index], row)
    return act(session, matching[0], row)


def not_offered(session: Session, kind: str, row, subject=None) -> str:
    """Why `row` does not offer `kind`, in the core's words."""
    own = (row or {}).get("actions", [])
    subject = own[0]["subject"] if own else subject
    if subject is None:
        return "Nothing to do here"
    try:
        return session.call(
            "form.not_offered", kind=kind, subject=subject, this_device=bool((row or {}).get("this_device")),
        ).get("value", "")
    except LumennaError as error:
        return error.message


def _slots(rows) -> list[tuple[str, int]]:
    """Every `(kind, nth)` any row offers, merged so that each row's actions keep the core's
    order: a slot new to the list goes straight after the row's previous one."""
    merged: list[tuple[str, int]] = []
    for row in rows:
        seen: dict[str, int] = {}
        after = -1
        for action in row.get("actions", []):
            nth = seen.get(action["kind"], 0)
            seen[action["kind"]] = nth + 1
            slot = (action["kind"], nth)
            if slot in merged:
                after = merged.index(slot)
            else:
                after += 1
                merged.insert(after, slot)
    return merged


def _nth(row, kind: str, nth: int):
    found = of_kind(row, kind)
    return found[nth] if nth < len(found) else None


def commands(session: Session, rows, before=(), after=()) -> list[Command]:
    """The rows' actions as context commands, between this screen's own `before` (Show its
    tasks) and `after`: each offered on the rows that have it, under the row's own title."""
    made = []
    # What each kind is done to on this screen, for a row with no actions of its own.
    subjects = {}
    for row in rows:
        for action in row.get("actions", []):
            subjects.setdefault(action["kind"], action["subject"])
    for kind, nth in _slots(rows):
        key = KEYS.get(kind, "") if nth == 0 else ""
        made.append(Command(
            lambda row, kind=kind, nth=nth: named(_nth(row, kind, nth)),
            lambda row, kind=kind, nth=nth: act(session, _nth(row, kind, nth), row),
            key=key,
            applies=lambda row, kind=kind, nth=nth: _nth(row, kind, nth) is not None,
            deletes=kind in DELETES and nth == 0,
            # A letter with several of its kind on the row asks which.
            by_key=(lambda row, kind=kind: run_kind(session, row, kind)) if key else None,
            refuse=(lambda row, kind=kind: not_offered(session, kind, row, subjects.get(kind))) if key else None,
        ))
    return [*before, *made, *after]


def heading_action(session: Session, group: str):
    """The action the places' heading for `group` offers: "Projects", "Labels", "Filters"."""
    for entry in session.call("places").get("entries", []):
        if entry.get("kind", {}).get("Group") == group:
            found = entry.get("actions", [])
            return found[0] if found else None
    return None


def add_from_heading(session: Session, group: str) -> str:
    """Adds a project, label or saved filter as its heading in the places offers."""
    action = heading_action(session, group)
    return act(session, action) if action else ""


def heading_command(session: Session, group: str, key: str = "a") -> list[Command]:
    """The command adding a project, label or saved filter, under the core's name for it
    ("New project"), as a screen's main menu offers it; none if the heading offers none."""
    action = heading_action(session, group)
    if action is None:
        return []
    return [Command(named(action), lambda _: add_from_heading(session, group), key=key)]
