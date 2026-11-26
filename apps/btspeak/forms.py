"""The core's dialogs, drawn with the device's own: a question, the task and block forms, quick
add and a settings page, each as the app describes it (`lumenna_app::dialog`, `DialogShown`).

The app gives every dialog as its controls in tab order, each saying what it is, what it holds
and what it sends; this file chooses which of the device's dialogs draws it (an input line, a
choice, a confirmation, a form) and hands what the person did to `control_event`, sending what
comes back. No event is built here, and no field is read by its key: a field added to a form
reaches this device with no change here.
"""

from __future__ import annotations

from BTSpeak import dialogs

import core
from core import native
from session import Session, choose, confirm

L = native()
T = L.ControlType


def opened(session: Session):
    """The dialog the app has open on top (`ScreenView::dialog`): a question, the block form,
    quick add; None for none."""
    described = session.described()
    return getattr(described, "dialog", None)


def send(session: Session, control, value: str = "", how=None) -> str:
    """What doing `how` (pressed, unless given) to `control`, holding `value`, sends, sent: what
    the app said."""
    events = L.control_event(control.sends, value, how if how is not None else L.ControlMovePressed())
    return session.send(*events) if events else ""


def cancel(session: Session, dialog) -> str:
    """Closes `dialog` unanswered, as its Cancel does."""
    return session.send(*dialog.cancel) if dialog.cancel else ""


def of_kind(dialog, *kinds) -> list:
    return [control for control in dialog.controls if control.kind in kinds]


def text_of(dialog, key: str) -> str:
    """What the line to read `key` says, or nothing."""
    return next((control.value for control in dialog.controls if control.key == key and control.kind == T.TEXT), "")


def default_button(dialog):
    return next((control for control in dialog.controls if control.kind == T.BUTTON and control.default), None)


def buttons(dialog) -> list:
    """The dialog's buttons but its Cancel."""
    return [control for control in dialog.controls if control.kind == T.BUTTON and not control.cancel]


def prompt(*parts: str) -> str:
    """A refusal before what is asked, each sentence ended once, as the core joins them
    (`prompt_of`)."""
    return L.prompt_of([part for part in parts if part])


def is_form(dialog) -> bool:
    """Whether `dialog` is a form of several fields (the task form, the block form) rather than a
    question asked in one line or one choice."""
    fields = of_kind(dialog, T.LINE, T.LINES, T.CHOICE, T.CHECK)
    return len(fields) > 1 or bool(of_kind(dialog, T.CHECK, T.CHOICE))


# ---------------------------------------------------------------------------------------
# A question
# ---------------------------------------------------------------------------------------


def ask(session: Session, dialog, asked: str) -> str:
    """A question the app has open, asked as the device asks one: a line to type, a choice from
    a list, a yes or no, or one of several answers. `asked` is what is asked, its parts joined
    (`QuestionShown::prompt`); a refusal goes in front of it. What the app said."""
    if is_form(dialog):
        return form(session, dialog)
    wrong = text_of(dialog, "refusal")
    listed = of_kind(dialog, T.LIST)
    lines = of_kind(dialog, T.LINE, T.LINES)
    pressing = buttons(dialog)
    if listed:
        options = listed[0]
        picked = choose(
            {option.id: option.title for option in options.options},
            prompt(wrong, asked or options.label),
            hints={option.id: option.reason for option in options.options},
        )
        if picked is None:
            return cancel(session, dialog)
        return send(session, options, picked)
    if lines:
        line = lines[0]
        said = prompt(wrong, asked)
        if line.syntax is not None:
            import tasks  # here: tasks imports this module

            typed = tasks.assisted_input(session, said, line.syntax, history_key="lumenna-filter", default=line.value)
        else:
            typed = dialogs.request_input(said, default_text=line.value)
        if typed is None:
            return cancel(session, dialog)
        button = default_button(dialog) or line
        return send(session, button, typed)
    if len(pressing) == 1:
        # A yes or no whose default is no: Cancel, for anything that cannot be taken back lightly.
        if not confirm(prompt(wrong, asked)):
            return cancel(session, dialog)
        return send(session, pressing[0])
    if pressing:
        picked = choose({control.key: control.label for control in pressing}, prompt(wrong, asked))
        if picked is None:
            return cancel(session, dialog)
        return send(session, next(control for control in pressing if control.key == picked))
    return cancel(session, dialog)


# ---------------------------------------------------------------------------------------
# A form: the task form, the block form
# ---------------------------------------------------------------------------------------


def _yes_no():
    return {choice.id: choice.title for choice in core.words().yes_no}


def _field(control) -> dialogs.InputField:
    """One of a form's controls as the device's form takes it: a choice where it has options, a
    check as a yes or no, else a line, or several lines; named, and hinted, in the core's words."""
    key = control.key
    if control.kind == T.CHECK:
        return dialogs.InputField(key=key, prompt=control.label, field_type="choice", choices=_yes_no(),
                                  default_text="yes" if control.checked else "no")
    if control.kind == T.CHOICE:
        # Each option as the core titles it: a project with its level after it where the level
        # changes, so that a choice is still found by its first letter (`lines::with_level`).
        offered = {option.id: option.title for option in control.options}
        return dialogs.InputField(key=key, prompt=control.label, field_type="choice", choices=offered,
                                  default_text=selected_id(control))
    return dialogs.InputField(
        key=key, prompt=control.label, default_text=control.value, format_hint=control.hint or None,
        field_type="multiline" if control.kind == T.LINES else "text",
    )


def selected_id(control) -> str:
    """The id of the option a choice holds, or what it holds where none is selected."""
    if control.selected is not None and control.selected < len(control.options):
        return control.options[control.selected].id
    return control.value


def shown_value(control) -> str:
    """What a control holds, as it would be sent: a choice's option's id, a check's `true` or
    `false`, a line's text."""
    if control.kind == T.CHECK:
        return "true" if control.checked else "false"
    if control.kind == T.CHOICE:
        return selected_id(control)
    return control.value


def form(session: Session, dialog) -> str:
    """A form of the app's (the task form, the block form), asked with the device's form: each
    field the person changed sent as the core says it is (`control_event`, set), then its default
    button pressed (Save, Add). Left, its Cancel. Fields left as they were are not sent: the app
    saves only what changed, and a block's kind brings its own settings with it unless they were
    changed too. A refusal is said first. What was said."""
    wrong = text_of(dialog, "refusal")
    if wrong:
        dialogs.show_message(wrong)
    fields = of_kind(dialog, T.LINE, T.LINES, T.CHOICE, T.CHECK)
    answers = dialogs.request_form([_field(control) for control in fields])
    if answers is None:
        return cancel(session, dialog)
    for control in fields:
        typed = answers.get(control.key)
        if typed is None:
            continue
        if control.kind == T.CHECK:
            typed = "true" if typed == "yes" else "false"
        if typed != shown_value(control):
            send(session, control, str(typed), L.ControlMoveSet())
    button = default_button(dialog)
    return send(session, button) if button is not None else ""


def form_until_closed(session: Session, dialog_id: str) -> str:
    """The app's form `dialog_id` (the block form), asked again while saving is refused: what
    was said once it closed."""
    said = ""
    while True:
        dialog = opened(session)
        if dialog is None or dialog.id != dialog_id:
            return said
        said = form(session, dialog)
