"""Projects, labels and saved filters: opened, and also made, renamed, reordered and
removed — each by the row's own actions, as the app offers them in its places.

Each list is the part of the app's sidebar under its heading, held in the sidebar's hand: a
change made from a row lands back on it, or on the row now in its place.
"""

from __future__ import annotations

import actions
from actions import L
from rows import Row, items
from session import Command, Session, empty_then, live_menu, screen
import tasks


def group_rows(session: Session, group) -> list[Row]:
    """The places under `group`'s heading in the app's sidebar, without those folded away under
    a collapsed one, each said with its level where that changes against the row shown before."""
    view = session.view()
    # The rows the app has collapsed: what is deeper under each is left out.
    folded = set(view.sidebar_folded)
    rows: list[Row] = []
    under = False
    previous = 0
    hidden_below = None
    for entry in view.sidebar:
        if entry.depth == 0:
            under = isinstance(entry.kind, L.SidebarKindGroup) and entry.kind.field_0 == group
            hidden_below = None
            continue
        if not under:
            continue
        if hidden_below is not None:
            if entry.depth > hidden_below:
                continue
            hidden_below = None
        if entry.key in folded:
            hidden_below = entry.depth
        depth = entry.depth - 1
        kinds = {action.kind for action in entry.actions}
        expanded = True if actions.K.COLLAPSE in kinds else False if actions.K.EXPAND in kinds else None
        level = f"level {depth + 1}" if depth != previous else None
        previous = depth
        row = Row(entry.key, entry.text, entry.actions, entry, level=level, expanded=expanded, title=entry.title)
        row.side = L.Side.SIDEBAR
        rows.append(row)
    return rows


def place_of(row: Row):
    """The place a sidebar row is."""
    kind = row.record.kind
    return kind.field_0 if isinstance(kind, L.SidebarKindPlace) else None


def add_in(session: Session, place) -> str:
    """Adds a task in `place`, starting the line as the app says a task added there does."""
    session.show(place)
    return tasks.add_task(session, session.view().quick_add)


def group_screen(session: Session, group, title: str, show: str, add: str, empty: str = "") -> str:
    """One heading's places: Enter shows one's tasks; its context menu shows them, adds a task
    there, and has the row's own actions; the main menu adds one, as the heading does."""
    state = {"heading": title}

    def build():
        rows = group_rows(session, group)
        state["heading"] = title
        return items(rows, fold=lambda row, open_: actions.fold(session, row, open_))

    def tasks_of(row) -> str:
        place = place_of(row)
        if place is None:
            return ""
        inside = [
            Command(actions.named(action), lambda _, action=action: actions.act(session, action, row), key="p")
            for action in actions.of_kind(row, actions.K.NEW_INSIDE)
        ]
        return tasks.task_list(session, place, row.title, more=inside)

    with screen("lumenna-organise"):
        live_menu(
            session, build, lambda: state["heading"],
            main=tasks_of,
            context=lambda rows: actions.commands(session, rows, before=[
                Command(show, tasks_of),
                Command(add, lambda row: add_in(session, place_of(row)), key="t"),
            ]),
            app=[*actions.heading_command(session, group), *tasks.undo_commands(session)],
            empty=empty_then(lambda: L.group_empty(group), empty),
            app_title=f"{title} menu",
            focus=lambda: session.view().sidebar_held.focus,
        )
    return ""


def projects(session: Session) -> str:
    """The project tree, foldable, with weights and what is archived."""
    return group_screen(session, L.SidebarGroup.PROJECTS, "Projects", "Show its tasks", "Add a task to it")


def labels(session: Session) -> str:
    """Labels: a first-class axis, with their own list."""
    return group_screen(session, L.SidebarGroup.LABELS, "Labels", "Show the tasks wearing it", "Add a task wearing it",
                        "Press a to add one.")


def saved_filters(session: Session) -> str:
    """Saved filters, and the lists they open. A filter is stored as text and evaluated when it
    is used, so one saved as "today" still means today next month."""
    return group_screen(session, L.SidebarGroup.FILTERS, "Filters", "Show its tasks", "Add a task to it",
                        "Press a to add one, or slash to filter now.")
