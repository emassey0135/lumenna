"""Projects, labels and saved filters: opened, and also made, renamed, reordered and
removed.

A name is written after its sigil the way the filter and quick-add languages read it: quoted
when it has a space in it.
"""

from __future__ import annotations

import math

from BTSpeak import dialogs

from rows import Tree, describe
from session import Command, Session, ask, choose, confirm, live_menu, row_item, screen
import tasks


def sigil(mark: str, name: str) -> str:
    return f'{mark}"{name}"' if " " in name else f"{mark}{name}"


# ---------------------------------------------------------------------------------------
# Projects
# ---------------------------------------------------------------------------------------


def parse_weight(text: str) -> float | str | None:
    """A project's weight as the core reads it (`parse_weight`): a number above zero, or
    "inherit" in any case. None for anything else, which must not quietly become either."""
    text = text.strip()
    if text.lower() == "inherit":
        return "inherit"
    try:
        value = float(text)
    except ValueError:
        return None
    return value if math.isfinite(value) and value > 0 else None


def projects(session: Session) -> str:
    """The project tree, foldable, with weights and what is archived. Enter shows a project's
    tasks; the rest is on its context menu."""
    state = {"heading": "Projects", "names": []}

    def build():
        listing = session.call("project.list")
        rows = listing.get("rows", [])
        state["heading"] = f"Projects, {listing.get('announcement', '')}"
        state["names"] = [row["title"] for row in rows]
        return Tree(rows).items()

    def rename(row):
        name = row["title"]
        to = ask(f"Rename {name} to", name)
        return session.write("project.rename", name=name, to=to) if to and to != name else ""

    def move_under(row):
        name = row["title"]
        options = {"": "The top level"}
        options.update({other: other for other in state["names"] if other != name})
        parent = choose(options, f"Move {name} under")
        if parent is None:
            return ""
        return session.write("project.move", name=name, **({"parent": parent} if parent else {}))

    def weigh(row):
        name = row["title"]
        text = ask(
            f"Weight of {name}: how much this whole area matters now, roughly 0.5 to 2, "
            "or inherit to take the parent's again",
            "1.0",
        )
        if text is None:
            return ""
        weight = parse_weight(text)
        if weight is None:
            return f"'{text.strip()}' is not a weight; use a number above zero, such as 1.5, or inherit"
        return session.write("project.weight", name=name, value=weight)

    def tasks_of(row):
        name = row["title"]
        return tasks.task_list(session, sigil("#", name), name, prefix=sigil("#", name) + " ")

    with screen("lumenna-organise"):
        live_menu(
            session, build, lambda: state["heading"],
            main=tasks_of,
            context=[
                Command("Show its tasks", tasks_of),
                Command("Add a task to it", lambda row: tasks.add_task(session, prefix=sigil("#", row["title"]) + " "), key="t"),
                Command("Add a project inside it", lambda row: add_project(session, parent=row["title"]), key="n"),
                Command("Rename", rename, key="r"),
                Command("Move it under another project", move_under, key="m"),
                Command("Move up", lambda row: session.write("project.order", name=row["title"], direction="up"), key=","),
                Command("Move down", lambda row: session.write("project.order", name=row["title"], direction="down"), key="."),
                Command("Weight", weigh, key="w"),
                Command(
                    lambda row: "Unarchive" if "archived" in row.get("state", []) else "Archive",
                    lambda row: session.write("project.archive", name=row["title"]),
                ),
                Command("Delete", lambda row: delete_project(session, row["title"]), deletes=True),
            ],
            app=[Command("Add a project", lambda _: add_project(session), key="a"), *tasks.undo_commands(session)],
            app_title="Projects menu",
        )
    return ""


def add_project(session: Session, parent: str | None = None) -> str:
    name = ask(f"New project under {parent}" if parent else "New project")
    if name is None:
        return ""
    return session.write("project.add", name=name, **({"parent": parent} if parent else {}))


def delete_project(session: Session, name: str) -> str:
    choice = choose(
        {"trash": "Delete it and put its tasks in the trash", "keep": "Delete it and keep its tasks in the Inbox"},
        f"Delete {name}?",
    )
    if choice is None:
        return ""
    return session.write("project.rm", name=name, keep_tasks=choice == "keep")


# ---------------------------------------------------------------------------------------
# Labels
# ---------------------------------------------------------------------------------------


def labels(session: Session) -> str:
    """Labels: a first-class axis, with their own list. Enter shows the tasks wearing
    one; the rest is on its context menu."""
    state = {"heading": "Labels", "names": []}

    def build():
        listing = session.call("label.list")
        rows = listing.get("rows", [])
        state["heading"] = f"Labels, {listing.get('announcement', '')}"
        state["names"] = [row["title"] for row in rows]
        return [row_item(row, describe(row), navigation_title=row["title"]) for row in rows]

    def rename(row):
        name = row["title"]
        to = ask(f"Rename {name} to", name)
        return session.write("label.rename", name=name, to=to) if to and to != name else ""

    def merge(row):
        # For when a typo made a near-duplicate: this one's tasks move to the other.
        name = row["title"]
        into = choose({other: other for other in state["names"] if other != name}, f"Merge {name} into")
        return session.write("label.merge", **{"from": name, "into": into}) if into else ""

    def colour(row):
        name = row["title"]
        chosen = ask(f"Colour for {name}: a colour name such as red or teal, or none. The name always shows too")
        return session.write("label.colour", name=name, colour=chosen) if chosen else ""

    def tasks_of(row):
        name = row["title"]
        return tasks.task_list(session, sigil("@", name), name, prefix=sigil("@", name) + " ")

    with screen("lumenna-organise"):
        live_menu(
            session, build, lambda: state["heading"],
            main=tasks_of,
            context=[
                Command("Show the tasks wearing it", tasks_of),
                Command("Add a task wearing it", lambda row: tasks.add_task(session, prefix=sigil("@", row["title"]) + " "), key="t"),
                Command("Rename", rename, key="r"),
                Command("Merge it into another label", merge, key="m"),
                Command("Colour", colour, key="c"),
                Command("Move up", lambda row: session.write("label.order", name=row["title"], direction="up"), key=","),
                Command("Move down", lambda row: session.write("label.order", name=row["title"], direction="down"), key="."),
                Command("Delete", lambda row: delete_label(session, row["title"]), deletes=True),
            ],
            app=[Command("Add a label", lambda _: add_label(session), key="a"), *tasks.undo_commands(session)],
            empty="No labels yet. Press a to add one.",
            app_title="Labels menu",
        )
    return ""


def add_label(session: Session) -> str:
    name = ask("New label")
    return session.write("label.add", name=name.lstrip("@")) if name else ""


def delete_label(session: Session, name: str) -> str:
    if not confirm(f"Delete {name}? Tasks wearing it stay; they just stop showing it."):
        return ""
    return session.write("label.rm", name=name)


# ---------------------------------------------------------------------------------------
# Saved filters
# ---------------------------------------------------------------------------------------


def saved_filters(session: Session) -> str:
    """Saved filters, and the lists they open: Enter opens one.

    A filter is stored as text and evaluated when it is used, so one saved as "today" still
    means today next month.
    """
    state = {"heading": "Filters"}

    def build():
        listing = session.call("filter.list")
        state["heading"] = f"Filters, {listing.get('announcement', '')}"
        return [
            row_item({"title": saved["name"], **saved}, f"{saved['name']}, {saved['query']}", navigation_title=saved["name"])
            for saved in listing.get("filters", [])
        ]

    def rename(row):
        name = row["name"]
        to = ask(f"Rename {name} to", name)
        return session.write("filter.edit", name=name, rename=to) if to and to != name else ""

    def requery(row):
        query = dialogs.request_input(f"Query for {row['name']}", default_text=row["query"])
        if not query or query == row["query"]:
            return ""
        return session.write("filter.edit", name=row["name"], query=query)

    with screen("lumenna-organise"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: tasks.task_list(session, row["query"], row["name"]),
            context=[
                Command("Rename", rename, key="r"),
                Command("Change the query", requery, key="q"),
                Command("Move up", lambda row: session.write("filter.order", name=row["name"], direction="up"), key=","),
                Command("Move down", lambda row: session.write("filter.order", name=row["name"], direction="down"), key="."),
                Command("Delete", lambda row: delete_filter(session, row["name"]), deletes=True),
            ],
            app=[
                Command("Add a filter", lambda _: add_filter(session), key="a"),
                Command("Search or filter now", lambda _: tasks.query_tasks(session), key="/"),
                *tasks.undo_commands(session),
            ],
            empty="No saved filters yet. Press a to add one, or slash to filter now.",
            app_title="Filters menu",
        )
    return ""


def add_filter(session: Session) -> str:
    name = ask("Name for the new filter")
    if name is None:
        return ""
    query = tasks.assisted_input(session, f"Query for {name}", "filter", history_key="lumenna-filter")
    if not query:
        return ""
    return session.write("filter.add", name=name, query=query)


def delete_filter(session: Session, name: str) -> str:
    if not confirm(f"Delete the filter {name}? The tasks it shows are not touched."):
        return ""
    return session.write("filter.rm", name=name)
