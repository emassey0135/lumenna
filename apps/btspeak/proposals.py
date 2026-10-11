"""Replan from now and Plan the day: the core proposes, and the person checks what to take.

A menu of the device's dialogs: the title is the whole proposal in one sentence, which the
menu says as it opens, since for someone who cannot glance at a timeline the sentence is the
check. Each change is a row, checked to begin with; Enter turns it off or on. The apply row
makes what is checked, its hint saying what becomes of a change left unchecked, and Cancel
leaves making nothing. Every word is the core's (`form.proposal`).

A change taken alone can leave the day crowded, so applying hands back the core's next
proposal, worked out around what was left unchecked, and the menu shows it in place of the
last; it closes once nothing more is proposed. A proposal the day has moved past is refused:
the core's sentence is said and shown over a new proposal.
"""

from __future__ import annotations

from BTSpeak import dialogs

import actions
from client import LumennaError
from rows import twelve_hour
from session import REFRESH, Flag, Session, spoken


#: For tests: a function of nothing giving now, a local date and time. None is now.
NOW = None


def _now() -> dict:
    return {"now": NOW()} if NOW else {}


def items_of(proposal: dict) -> list[dict]:
    """A replan's changes, or a day plan's sittings."""
    return list(proposal.get("changes") or proposal.get("sittings") or [])


def unfit_of(proposal: dict) -> list[str]:
    """What fits nowhere: a replan's `unplaceable`, a day plan's `unfit`."""
    return list(proposal.get("unplaceable") or proposal.get("unfit") or [])


def ask(session: Session, kind: str, date: str | None, pins=None) -> dict:
    """What the core proposes for `kind`, keeping to `pins`, times in the device's clock."""
    params = {"clock": "12-hour" if twelve_hour() else "24-hour", **_now()}
    if pins is not None:
        params["pins"] = pins
    if kind == "replan":
        return session.call("replan", **params)
    if date:
        params["date"] = date
    return session.call("day_plan", **params)


def proposal_screen(session: Session, kind: str, date: str | None = None) -> str:
    """The proposal of `kind` ("replan", "plan_day"), checked and made; what to say after."""
    words = session.words("form.proposal", kind=kind)
    try:
        first = ask(session, kind, date)
    except LumennaError as error:
        return error.message
    state = {"proposal": first, "checked": [True] * len(items_of(first)), "refused": "", "said": "", "done": False}
    moved = Flag()

    def show(proposal: dict, refused: str = "") -> None:
        state.update(proposal=proposal, checked=[True] * len(items_of(proposal)), refused=refused)
        moved.set()

    def state_word(index: int) -> str:
        return "checked" if state["checked"][index] else "not checked"

    def toggle(index: int) -> str:
        state["checked"][index] = not state["checked"][index]
        return f"{session.sentence(words['item'])}, {state_word(index)}"

    def apply(menu=None) -> str:
        proposal = state["proposal"]
        accepted = [item for item, on in zip(items_of(proposal), state["checked"]) if on]
        try:
            if kind == "replan":
                result = session.call("replan.apply", proposal=proposal, accepted=accepted, **_now())
            else:
                result = session.call("day_plan.apply", plan=proposal, accepted=accepted, **_now())
        except LumennaError as error:
            try:
                again = ask(session, kind, date, proposal.get("pins"))
            except LumennaError as failed:
                return failed.message
            show(again, error.message)
            return ". ".join(part.rstrip(".") for part in (error.message, again.get("announcement", "")) if part)
        session.wrote()
        said = spoken(result)
        following = result.get("next") or {}
        if items_of(following):
            show(following)
            return said
        # Nothing more is proposed: the screen closes, and what was made is said after it.
        state.update(done=True, said=said)
        if menu is not None:
            menu.close()
        return ""

    def take(other: dict) -> str:
        try:
            again = ask(session, kind, date, other.get("pins"))
        except LumennaError as error:
            return error.message
        show(again)
        return spoken(again)

    def cancel(menu) -> None:
        state["done"] = True
        menu.close()

    def build() -> list:
        proposal = state["proposal"]
        rows = []
        if state["refused"]:
            rows.append(dialogs.DynamicMenuItem(title=state["refused"], action=lambda: state["refused"]))
        items = items_of(proposal)
        for index, item in enumerate(items):
            rows.append(dialogs.DynamicMenuItem(
                title=lambda index=index, item=item: f"{item['sentence']}, {state_word(index)}",
                action=lambda index=index: toggle(index),
                # Why it is offered is its description: the device says a row's hint after it.
                hint=item.get("reason") or None,
            ))
        if not items:
            rows.append(dialogs.DynamicMenuItem(title=words["nothing"], action=lambda: words["nothing"]))
        unfit = unfit_of(proposal)
        if unfit:
            rows.append(dialogs.DynamicMenuItem(title=words["unfit"], action=lambda: ""))
            rows.extend(dialogs.DynamicMenuItem(title=line, action=lambda: "") for line in unfit)
        if items:
            # What becomes of a change left unchecked, beside the button that makes the rest.
            rows.append(dialogs.DynamicMenuItem(title=session.sentence(words["apply"]), action=apply, hint=words["hint"]))
        others = proposal.get("alternatives") or []
        if others:
            # Other options, each asking again with its own pins; none, no heading.
            rows.append(dialogs.DynamicMenuItem(title=words["others"], action=lambda: ""))
            rows.extend(dialogs.DynamicMenuItem(title=other["sentence"], action=lambda other=other: take(other))
                        for other in others)
        rows.append(dialogs.DynamicMenuItem(title=session.sentence(words["cancel"]), action=cancel))
        return rows

    def commands() -> list:
        made = []
        if items_of(state["proposal"]):
            made.append(dialogs.MenuCommand(name="apply", label=session.sentence(words["apply"]),
                                            action=lambda menu: apply(menu)))
        made.append(dialogs.MenuCommand(name="cancel", label=session.sentence(words["cancel"]),
                                        action=lambda menu: cancel(menu)))
        return made

    opening = True
    while not state["done"]:
        proposal = state["proposal"]
        dialogs.dynamic_menu(
            build(),
            title=f"{session.sentence(words['title'])}. {proposal.get('announcement', '')}",
            exit_condition=moved,
            refresh_interval=REFRESH,
            # Said as it opens; a proposal shown after an apply was said by the apply.
            announce_title=opening,
            app_menu=commands(),
            app_menu_title=f"{session.sentence(words['title'])} menu",
        )
        opening = False
        if not moved(clear=True):
            break
    return state["said"]


actions.FORMS[("day", "replan")] = lambda session, action, row: proposal_screen(session, "replan")
actions.FORMS[("day", "plan_day")] = lambda session, action, row: proposal_screen(session, "plan_day", action["target"])
