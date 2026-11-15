"""What can be done to a row: the app's, offered as the device offers things.

Every row the app shows carries its `actions` — which apply now, in order, under their spoken
names. This file turns them into the row's context menu (M-Chord with Dot 7) and its
letters, and gives it its delete keys, which run what the app says the delete key does;
running one sends it to the app with the row in hand, then answers whatever the app opens
with the device's own dialogs, one after another, until nothing is open: a question (and the
one a pick asks after, how long), the block form, a proposal, a reminder list. Which actions a row has, what each asks, what a pick offers and why an answer is refused
are the app's, the same on every client.

A letter stands for a kind of action (the core's table, `action_letter`), so `c` is Mark Done
on a task that is open and Mark Not Done on one that is finished; a letter is handed to the app
(`ShellEventLetter`), which runs what it runs there or says why it runs nothing.
Every question and form is drawn from the app's description of it (`forms.py`).
"""

from __future__ import annotations

from BTSpeak import dialogs

import core
from core import native, refused
from session import Command, Session, choose, sentences

L = native()
K = L.ActionKind

#: Folding, which is the left and right keys' rather than the context menu's.
FOLDS = {K.EXPAND, K.COLLAPSE}

def named(action) -> str:
    """What an action is called here: its sentence-case name, as the device's own menus
    word things and as braille wants, a capital costing a cell."""
    return action.sentence or action.title


def prompt(*parts: str) -> str:
    """Parts of a question as one prompt, each sentence ended once."""
    return ". ".join(part.strip().rstrip(".") for part in parts if part and part.strip())


def joined(*said: str) -> str:
    """What several steps said, as one thing to say."""
    return sentences(*said)


def held(row):
    """The event putting `row` in hand before its action runs: the main list's `Focus`, or a
    side list's `Hold`, so a change made there lands back there."""
    side = getattr(row, "side", None)
    if side is not None:
        return L.ShellEventHold(side, row.key)
    return L.ShellEventFocus(row.key)


def acting_on(row):
    """Which row an action is for, as the app names it: a side list's row by its key there,
    else a row of the place shown."""
    side = getattr(row, "side", None)
    if side is not None:
        return L.ActingOnHeld(side, row.key)
    return L.ActingOnRow(row.key)


def act(session: Session, action, row=None, on=None) -> str:
    """Runs `action`, a row's in short, on `row` (or what `on` names: the day, the reminder
    list), then answers whatever the app opens; what to say. The app puts the row in hand and
    runs its action whole as it has it now."""
    import proposals  # here: proposals imports this module

    proposals.pin(session)
    said = [session.send(L.ShellEventActOn(on if on is not None else acting_on(row), action))]
    said.append(follow(session, row))
    return joined(*said)


def follow(session: Session, row=None) -> str:
    """Answers what the app has open, one after another, until nothing is: what was said."""
    said = []
    # Each answer either closes what was open or opens the next; a bound in case one does not.
    for _ in range(100):
        view = session.view()
        if view.asking is not None:
            said.append(ask(session, view.asking, row))
        elif view.block_form is not None:
            import day  # here: day imports this module

            said.append(day.block_form(session))
        elif view.proposal is not None:
            import proposals  # here: proposals imports this module

            said.append(proposals.proposal_screen(session))
        elif view.reminders is not None and not getattr(session, "reminders_open", False):
            import reminder_lists  # here: reminder_lists imports this module

            said.append(reminder_lists.reminders_of(session, row))
        elif view.line is not None and not getattr(session, "typing", False):
            # The quick-add line the app opened (New task): asked here, as the app started it.
            import tasks  # here: tasks imports this module

            said.append(tasks.add_task(session))
        elif view.settings is not None and not getattr(session, "settings_open", False):
            # Settings the app opened (Settings): their pages, until left.
            import preferences  # here: preferences imports this module

            said.append(preferences.settings(session))
        else:
            break
    return joined(*said)


def ask(session: Session, asking, row=None) -> str:
    """Asks the app's open question with the device's dialogs, drawn from the app's own
    description of it (`ScreenView::dialog`, `forms.ask`), and sends what the person did as the
    app says it is sent; what the app said. A refused answer stays in the question, the app's
    sentence first. A task's form asked for (Edit details) is the task form, for the row's own
    task: its details shown, the app describes the question as that form."""
    import forms  # here: forms imports session, as this module does

    dialog = forms.opened(session)
    if dialog is None and isinstance(asking.question, L.QuestionForm) and asking.action is not None:
        session.send(L.ShellEventDetails(L.DetailsEventShow(asking.action.target)))
        dialog = forms.opened(session)
    if dialog is None or not dialog.id.startswith("question:"):
        session.send(L.ShellEventCancel())
        return L.cannot_ask_here(named(asking.action)) if asking.action else ""
    # What is asked, its parts joined as the core joins them (`QuestionShown::prompt`).
    return forms.ask(session, dialog, session.prompt() or (named(asking.action) if asking.action else ""))


def of_kind(row, *kinds) -> list:
    """The actions `row` offers of any of `kinds`, in its order."""
    return [a for a in (getattr(row, "actions", None) or []) if a.kind in kinds]


def run_kind(session: Session, row, *kinds) -> str:
    """Does the row's action of one of `kinds`; where it offers several, asks which."""
    matching = of_kind(row, *kinds)
    if not matching:
        return ""
    if len(matching) > 1:
        index = choose({i: named(a) for i, a in enumerate(matching)}, core.words().which)
        if index is None:
            return ""
        return act(session, matching[index], row)
    return act(session, matching[0], row)


def fold(session: Session, row, open_: bool) -> str:
    """The row's own Expand, or Collapse: the app folds, and says which."""
    return run_kind(session, row, K.EXPAND if open_ else K.COLLAPSE)


def press(session: Session, key: str, row) -> str:
    """A command of the shared table run on `row` (`"delete"`, `"open"`, `"mark-done"`): the
    app puts the row in hand and runs what that command runs there, or says why it does nothing;
    then whatever it opens is answered."""
    import proposals  # here: proposals imports this module

    proposals.pin(session)
    said = [session.send(held(row), L.ShellEventKey(key))]
    said.append(follow(session, row))
    return joined(*said)


def key(session: Session, command: str) -> str:
    """One of the screen's own actions (`ScreenAction`), run as the app runs it (`Event::Key`),
    then whatever it opens answered: a question, the block form, the quick-add line, settings."""
    import proposals  # here: proposals imports this module

    proposals.pin(session)
    said = [session.send(L.ShellEventKey(command))]
    said.append(follow(session))
    return joined(*said)


def screen_commands(session: Session, own: dict, skip=()) -> list[Command]:
    """What the screen shown offers of its own (`ViewModel::screen_actions`), in the core's order
    with its letters, as the screen's main menu (M-Chord): each run as the app runs it
    (`Event::Key`), but those the core marks the shell's (`shell`: the filter, Key help, About,
    Reminders due) and those this app follows its own way (`own`, by id: Sync now's wait),
    which run `own`'s function of the session. One that does nothing now (`enabled` unset) is
    left off the menu; its letter still goes to the app, which says why. `skip` leaves out those
    whose letter the screen offers already (the day's own What should I work on?)."""
    made = []
    for action in session.view().screen_actions:
        if action.letter and action.letter in skip:
            continue
        mine = own.get(action.id)
        if action.shell and mine is None:
            continue
        made.append(Command(
            action.title,
            (lambda _, mine=mine: mine(session)) if mine is not None else (lambda _, id=action.id: key(session, id)),
            key=action.letter,
            through=not action.shell,
            listed=action.enabled,
        ))
    return made


def letter(session: Session, key: str, row) -> str:
    """A letter pressed on `row`, or on an empty list: the app puts the row in hand and runs what
    that letter runs there (`Event::Letter`), or says why it runs nothing, then whatever it opens
    is answered."""
    import proposals  # here: proposals imports this module

    proposals.pin(session)
    moved = [held(row)] if row is not None and getattr(row, "key", None) else []
    said = [session.send(*moved, L.ShellEventLetter(key))]
    said.append(follow(session, row))
    return joined(*said)


def _slots(rows) -> list:
    """Every `(kind, nth)` any row offers, merged so that each row's actions keep the core's
    order: a slot new to the list goes straight after the row's previous one. Folding is the
    left and right keys'."""
    merged: list = []
    for row in rows:
        seen: dict = {}
        after = -1
        for action in row.actions:
            if action.kind in FOLDS:
                continue
            nth = seen.get(action.kind, 0)
            seen[action.kind] = nth + 1
            slot = (action.kind, nth)
            if slot in merged:
                after = merged.index(slot)
            else:
                after += 1
                merged.insert(after, slot)
    return merged


def _nth(row, kind, nth: int):
    found = of_kind(row, kind)
    return found[nth] if nth < len(found) else None


def commands(session: Session, rows, before=(), after=()) -> list[Command]:
    """The rows' actions as context commands, between this screen's own `before` (Show its
    tasks) and `after`: each offered on the rows that have it, under the row's own title."""
    made = []
    for kind, nth in _slots(rows):
        made.append(Command(
            lambda row, kind=kind, nth=nth: named(_nth(row, kind, nth)),
            lambda row, kind=kind, nth=nth: act(session, _nth(row, kind, nth), row),
            key=(L.action_letter(kind) or "") if nth == 0 else "",
            applies=lambda row, kind=kind, nth=nth: _nth(row, kind, nth) is not None,
            through=True,
        ))
    # The delete keys, Control-D and the D chord: what they do to the row is the app's, the same
    # key on every platform (Delete, Delete from trash on a trashed task, Unassign, Unpair).
    made.append(Command("Delete key", lambda row: press(session, "delete", row), deletes=True, listed=False))
    return [*before, *made, *after]


def heading_action(session: Session, group):
    """What the places' heading for `group` adds (New Project, New Label, New Saved Filter),
    and the heading it is on; none if it offers none."""
    for entry in session.view().sidebar:
        if isinstance(entry.kind, L.SidebarKindGroup) and entry.kind.field_0 == group:
            action = next((a for a in entry.offered if a.kind == K.NEW), None)
            return (action, L.ActingOnHeld(L.Side.SIDEBAR, entry.key)) if action is not None else None
    return None


def heading_command(session: Session, group) -> list[Command]:
    """The command adding a project, label or saved filter, under the core's name for it
    ("New project"), as a screen's main menu offers it; none if the heading offers none."""
    found = heading_action(session, group)
    if found is None:
        return []

    def run(_row) -> str:
        action, on = heading_action(session, group) or found
        return act(session, action, on=on)

    return [Command(named(found[0]), run, key=L.action_letter(K.NEW) or "")]
