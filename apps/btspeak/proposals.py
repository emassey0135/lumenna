"""Replan from now and Plan the day: the app proposes, and the person checks what to take.

The app's proposal screen (`ViewModel::proposal`), drawn as a menu of the device's dialogs: the
title is the whole proposal in one sentence, which the menu says as it opens, since for someone
who cannot glance at a timeline the sentence is the check. Each change is a row, checked as the
app has it; Enter turns it off or on. The apply row makes what is checked, its hint saying what
becomes of a change left unchecked, and Cancel leaves making nothing. Every word is the core's,
whether a change is checked too (`ChangeView.state`); only the main menu's name ("… menu") is
this app's, as on every screen here.

A change taken alone can leave the day crowded, so applying shows the app's next proposal,
worked out around what was left unchecked, in place of the last; the screen closes once nothing
more is proposed. A proposal the day has moved past is refused: the app's sentence is said and
shown over a new proposal. Other options each propose again, in place.
"""

from __future__ import annotations

from BTSpeak import dialogs

from actions import L
from session import REFRESH, Session


#: For tests: a function of nothing giving the moment proposals are made against, a local date
#: and time (`2030-01-15T08:00`). None is now.
NOW = None


def pin(session: Session) -> None:
    """Tells the app the moment proposals are made against, when a test has fixed it."""
    if NOW:
        session.send(L.ShellEventPlanFrom(NOW()))


def proposal_screen(session: Session) -> str:
    """The proposal the app has open, checked and made; what to say after."""
    said = {"after": ""}
    shown = {"count": None}

    def send(event) -> str:
        return session.send(L.ShellEventProposal(event))

    def toggle(index: int, checked: bool) -> str:
        send(L.ProposalEventCheck(index, not checked))
        # Said as a check box says itself, by the core's name for a change and its state.
        proposal = session.view().proposal
        if proposal is None or index >= len(proposal.changes):
            return ""
        return f"{proposal.words.item}, {proposal.changes[index].state}"

    def apply(menu=None) -> str:
        made = send(L.ProposalEventApply())
        if session.view().proposal is None:
            # Nothing more is proposed: the screen closes, and what was made is said after it.
            said["after"] = made
            if menu is not None:
                menu.close()
            return ""
        return made

    def cancel(menu) -> None:
        send(L.ProposalEventClose())
        menu.close()

    def take(index: int) -> str:
        return send(L.ProposalEventTake(index))

    def checked(index: int) -> bool:
        proposal = session.view().proposal
        return proposal is not None and index < len(proposal.changes) and proposal.changes[index].checked

    def line(index: int) -> str:
        proposal = session.view().proposal
        if proposal is None or index >= len(proposal.changes):
            return ""
        change = proposal.changes[index]
        return f"{change.sentence}, {change.state}"

    def which():
        """What tells one proposal shown from the next: a new proposal, or a refusal."""
        proposal = session.view().proposal
        return None if proposal is None else (proposal.shown, proposal.refusal)

    def build(proposal) -> list:
        words = proposal.words
        rows = []
        if proposal.refusal:
            rows.append(dialogs.DynamicMenuItem(title=proposal.refusal, action=lambda: proposal.refusal))
        for index, change in enumerate(proposal.changes):
            rows.append(dialogs.DynamicMenuItem(
                # Read again as it is checked, so the menu stays where it is.
                title=lambda *_, index=index: line(index),
                action=lambda index=index: toggle(index, checked(index)),
                # Why it is offered is its description: the device says a row's hint after it.
                hint=change.reason or None,
            ))
        if not proposal.changes:
            rows.append(dialogs.DynamicMenuItem(title=words.nothing, action=lambda: words.nothing))
        if proposal.unfit:
            rows.append(dialogs.DynamicMenuItem(title=words.unfit, action=lambda: ""))
            rows.extend(dialogs.DynamicMenuItem(title=line, action=lambda: "") for line in proposal.unfit)
        if proposal.changes:
            # What becomes of a change left unchecked, beside the button that makes the rest.
            rows.append(dialogs.DynamicMenuItem(title=words.apply, action=apply, hint=words.hint))
        if proposal.others:
            # Other options, each proposing again with its own pins; none, no heading.
            rows.append(dialogs.DynamicMenuItem(title=words.others, action=lambda: ""))
            rows.extend(dialogs.DynamicMenuItem(title=other, action=lambda index=index: take(index))
                        for index, other in enumerate(proposal.others))
        rows.append(dialogs.DynamicMenuItem(title=words.cancel, action=cancel))
        return rows

    def commands(proposal) -> list:
        made = []
        if proposal.changes:
            made.append(dialogs.MenuCommand(name="apply", label=proposal.words.apply, action=lambda menu: apply(menu)))
        made.append(dialogs.MenuCommand(name="cancel", label=proposal.words.cancel, action=lambda menu: cancel(menu)))
        return made

    while True:
        proposal = session.view().proposal
        if proposal is None:
            break
        session.settle()
        drawn = which()
        # Said as it opens, and for each new proposal; one shown after an apply was said by it.
        opening = shown["count"] is None
        shown["count"] = proposal.shown
        choice = dialogs.dynamic_menu(
            build(proposal),
            title=f"{proposal.words.title}. {proposal.announcement}",
            # Only a new proposal redraws it: checking a change rewords its row in place.
            exit_condition=lambda: which() != drawn,
            refresh_interval=REFRESH,
            announce_title=opening,
            app_menu=commands(proposal),
            app_menu_title=L.menu_of(proposal.words.title),
        )
        if session.view().proposal is None:
            break
        if choice is None and which() == drawn:
            # The person left: the proposal is put away, making nothing.
            send(L.ProposalEventClose())
            break
    return said["after"]
