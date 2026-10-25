"""What can be done to a row: the app's, offered as the device offers things.

Every row the app shows carries its `actions` — which apply now, in order, under their spoken
names. This file turns them into the row's context menu (M-Chord with Dot 7) and its
letters, and gives it its delete keys, which run what the app says the delete key does;
running one sends it to the app with the row in hand, then answers whatever the app opens
with the device's own dialogs, one after another, until nothing is open: a question (and the
one a pick asks after, how long), the block form, a proposal, a reminder list. Which actions a row has, what each asks, what a pick offers and why an answer is refused
are the app's, the same on every client.

A letter stands for a kind of action (`KEYS`), so `c` is Mark Done on a task that is open
and Mark Not Done on one that is finished, and does nothing on a row that offers neither.
The task form is this app's own (`Question::Form`), registered in `FORMS` by `tasks.py`.
"""

from __future__ import annotations

from BTSpeak import dialogs

from core import native, refused
from session import Command, Session, choose, confirm, sentences

L = native()
K = L.ActionKind

#: The letter for each kind of action: the same letter for the same thing on every screen.
#: Two kinds share a letter only where no row offers both.
KEYS = {
    K.MARK_DONE: "c",
    K.MARK_NOT_DONE: "c",
    K.EDIT: "e",
    K.EDIT_TASK: "e",
    K.PUT_IN_BLOCK: "b",
    K.MOVE_TO_PROJECT: "m",
    K.MAKE_SUBTASK_OF: "s",
    K.MOVE_TO_TOP_LEVEL: "t",
    K.WAIT_FOR: "w",
    K.STOP_WAITING: "n",
    K.RESTORE: "r",
    K.ASSIGN_TASK: "i",
    K.CANCEL_DAY: "x",
    K.RESTORE_DAY: "o",
    K.START_TIMER: "s",
    K.PAUSE_TIMER: "s",
    K.RESUME_TIMER: "s",
    K.STOP_TIMER: "t",
    K.PLANNED_LENGTH: "l",
    K.LOG_MINUTES: "m",
    K.ADD_BLOCK: "a",
    K.RENAME: "r",
    K.NEW_INSIDE: "n",
    K.MOVE_UNDER: "m",
    K.MOVE_UP: ",",
    K.MOVE_DOWN: ".",
    K.WEIGHT: "w",
    K.MERGE_INTO: "m",
    K.COLOUR: "c",
    K.CHANGE_QUERY: "q",
    K.REMINDERS: "r",
    # A sitting's Carry Forward and a block's Carry the Rest Forward: no row offers both.
    K.CARRY_FORWARD: "f",
    K.CARRY_REST: "f",
}

#: The letters for the day's own actions, on the Day menu: none a row of the day uses.
DAY_KEYS = {
    K.WHAT_NOW: "w",
}

#: Folding, which is the left and right keys' rather than the context menu's.
FOLDS = {K.EXPAND, K.COLLAPSE}

#: How each of this app's own forms opens: `(subject, kind)` to a function of the session, the
#: action and the row it was offered on. The screens that have the forms fill this in.
FORMS: dict = {}


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
        else:
            break
    return joined(*said)


def ask(session: Session, asking, row=None) -> str:
    """Asks the app's open question with the device's dialogs and sends the answer, or Cancel
    when the person cancels; what the app said. A refused answer stays in the question, the
    app's sentence first."""
    question = asking.question
    action = asking.action
    wrong = asking.refusal or ""
    cancel = L.ShellEventCancel()

    def answer(given) -> str:
        return session.send(L.ShellEventAnswer(given))

    if isinstance(question, L.QuestionForm):
        form = FORMS.get((action.subject, action.kind)) if action is not None else None
        session.send(cancel)
        if form is None:
            return f"{named(action)} has no form in this app yet" if action is not None else ""
        return form(session, action, row)
    if isinstance(question, L.QuestionImmediate):
        return answer(L.AnswerYes())
    if isinstance(question, L.QuestionConfirm):
        # Cancel is the default, as for anything that cannot be taken back lightly.
        if not confirm(prompt(wrong, f"{question.title} {question.message}")):
            return session.send(cancel)
        return answer(L.AnswerYes())
    if isinstance(question, L.QuestionText):
        said = prompt(wrong, question.title, question.hint, question.label)
        # A saved filter's query is the filter language, which the app says: completion is offered.
        if asking.syntax is not None:
            import tasks  # here, since tasks imports this module

            text = tasks.assisted_input(session, said, asking.syntax, history_key="lumenna-filter",
                                        default=question.initial)
        else:
            text = dialogs.request_input(said, default_text=question.initial)
        if text is None:
            return session.send(cancel)
        return answer(L.AnswerText(text))
    if isinstance(question, L.QuestionPick):
        return ask_pick(session, asking, question, wrong)
    if isinstance(question, L.QuestionChoose):
        picked = choose(
            {a.id: a.title for a in question.answers},
            prompt(wrong, question.title, question.message),
        )
        if picked is None:
            return session.send(cancel)
        return answer(L.AnswerPicked(picked, None))
    session.send(cancel)
    return f"This app cannot ask that question yet; update it to {named(action)}" if action else ""


def ask_pick(session: Session, asking, question, wrong: str) -> str:
    """One of what the app offers, each said as its line, why it is offered its hint. Put in a
    Block's ends in Another Day, the app's own: picked, the app asks which day."""
    offered = asking.choices.choices if asking.choices is not None else []
    lines = session.lines().choices
    options = {choice.id: (lines[i].text if i < len(lines) else choice.title) for i, choice in enumerate(offered)}
    picked = choose(options, prompt(wrong, question.title or named(asking.action)),
                    hints={choice.id: choice.reason for choice in offered})
    if picked is None:
        return session.send(L.ShellEventCancel())
    return session.send(L.ShellEventAnswer(L.AnswerPicked(picked, None)))


def of_kind(row, *kinds) -> list:
    """The actions `row` offers of any of `kinds`, in its order."""
    return [a for a in (getattr(row, "actions", None) or []) if a.kind in kinds]


def run_kind(session: Session, row, *kinds) -> str:
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


def fold(session: Session, row, open_: bool) -> str:
    """The row's own Expand, or Collapse: the app folds, and says which."""
    return run_kind(session, row, K.EXPAND if open_ else K.COLLAPSE)


def not_offered(session: Session, kind, row) -> str:
    """A key `row` does not offer: the app runs it there and says why, in the core's words."""
    said = [session.send(held(row))] if row is not None and getattr(row, "key", None) else []
    said.append(session.send(L.ShellEventRun([kind])))
    return joined(*said)


def press(session: Session, key: str, row) -> str:
    """A key of the shared table pressed on `row` (`"delete"`): the app puts the row in hand and
    runs what that key runs there, or says why it does nothing; then whatever it opens is
    answered."""
    import proposals  # here: proposals imports this module

    proposals.pin(session)
    said = [session.send(held(row), L.ShellEventKey(key))]
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
        key = KEYS.get(kind, "") if nth == 0 else ""
        made.append(Command(
            lambda row, kind=kind, nth=nth: named(_nth(row, kind, nth)),
            lambda row, kind=kind, nth=nth: act(session, _nth(row, kind, nth), row),
            key=key,
            applies=lambda row, kind=kind, nth=nth: _nth(row, kind, nth) is not None,
            # A letter with several of its kind on the row asks which.
            by_key=(lambda row, kind=kind: run_kind(session, row, kind)) if key else None,
            refuse=(lambda row, kind=kind: not_offered(session, kind, row)) if key else None,
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


def heading_command(session: Session, group, key: str = "a") -> list[Command]:
    """The command adding a project, label or saved filter, under the core's name for it
    ("New project"), as a screen's main menu offers it; none if the heading offers none."""
    found = heading_action(session, group)
    if found is None:
        return []

    def run(_row) -> str:
        action, on = heading_action(session, group) or found
        return act(session, action, on=on)

    return [Command(named(found[0]), run, key=key)]
