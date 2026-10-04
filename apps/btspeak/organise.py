"""Projects, labels and saved filters: opened, and also made, renamed, reordered and
removed (§3.4, §6.2).

A name is written after its sigil the way the filter and quick-add languages read it: quoted
when it has a space in it.
"""

from __future__ import annotations

from BTSpeak import dialogs

from rows import Tree, describe
from session import Session, ask, choose, confirm, live_menu
import tasks


def sigil(mark: str, name: str) -> str:
    return f'{mark}"{name}"' if " " in name else f"{mark}{name}"


# ---------------------------------------------------------------------------------------
# Projects
# ---------------------------------------------------------------------------------------


def projects(session: Session) -> str:
    """The project tree, foldable, with weights and what is archived."""
    state = {"heading": "Projects", "names": []}

    def build():
        listing = session.call("project.list")
        rows = listing.get("rows", [])
        state["heading"] = f"Projects, {listing.get('announcement', '')}"
        state["names"] = [row["title"] for row in rows]
        add = dialogs.DynamicMenuItem(title="Add a project", action=lambda: add_project(session))
        return [add] + Tree(rows).items(
            lambda row: project_actions(session, row, state["names"]),
            on_delete=lambda row: delete_project(session, row["title"]),
        )

    live_menu(session, build, lambda: state["heading"])
    return ""


def add_project(session: Session, parent: str | None = None) -> str:
    name = ask(f"New project under {parent}" if parent else "New project")
    if name is None:
        return ""
    return session.write("project.add", name=name, **({"parent": parent} if parent else {}))


def project_actions(session: Session, row: dict, names: list[str]) -> str:
    name = row["title"]
    archived = "archived" in row.get("state", [])
    actions = {
        "open": "Show its tasks",
        "add": "Add a task to it",
        "sub": "Add a project under it",
        "rename": "Rename",
        "under": "Move it under another project",
        "up": "Move up",
        "down": "Move down",
        "weight": "Weight",
        "archive": "Unarchive" if archived else "Archive",
        "rm": "Delete",
    }
    choice = choose(actions, describe(row))
    if choice == "open":
        return tasks.task_list(session, sigil("#", name), name, prefix=sigil("#", name) + " ")
    if choice == "add":
        return tasks.add_task(session, prefix=sigil("#", name) + " ")
    if choice == "sub":
        return add_project(session, parent=name)
    if choice == "rename":
        to = ask(f"Rename {name} to", name)
        return session.write("project.rename", name=name, to=to) if to and to != name else ""
    if choice == "under":
        options = {"": "The top level"}
        options.update({other: other for other in names if other != name})
        parent = choose(options, f"Move {name} under")
        if parent is None:
            return ""
        return session.write("project.move", name=name, **({"parent": parent} if parent else {}))
    if choice in ("up", "down"):
        return session.write("project.order", name=name, direction=choice)
    if choice == "weight":
        text = ask(
            f"Weight of {name}: how much this whole area matters now, roughly 0.5 to 2, "
            "or inherit to take the parent's again",
            "1.0",
        )
        if text is None:
            return ""
        if text.lower() == "inherit":
            return session.write("project.weight", name=name, value="inherit")
        try:
            return session.write("project.weight", name=name, value=float(text))
        except ValueError:
            return "A weight is a number, such as 1.5, or inherit"
    if choice == "archive":
        return session.write("project.archive", name=name)
    if choice == "rm":
        return delete_project(session, name)
    return ""


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
    """Labels: a first-class axis, with their own list (§16.1)."""
    state = {"heading": "Labels", "names": []}

    def build():
        listing = session.call("label.list")
        rows = listing.get("rows", [])
        state["heading"] = f"Labels, {listing.get('announcement', '')}"
        state["names"] = [row["title"] for row in rows]
        items = [dialogs.DynamicMenuItem(title="Add a label", action=lambda: add_label(session))]
        for row in rows:
            items.append(
                dialogs.DynamicMenuItem(
                    title=describe(row),
                    action=(lambda row=row: label_actions(session, row, state["names"])),
                    delete=(lambda row=row: delete_label(session, row["title"])),
                )
            )
        return items

    live_menu(session, build, lambda: state["heading"])
    return ""


def add_label(session: Session) -> str:
    name = ask("New label")
    return session.write("label.add", name=name.lstrip("@")) if name else ""


def label_actions(session: Session, row: dict, names: list[str]) -> str:
    name = row["title"]
    actions = {
        "open": "Show the tasks wearing it",
        "add": "Add a task wearing it",
        "rename": "Rename",
        "merge": "Merge it into another label",
        "colour": "Colour",
        "up": "Move up",
        "down": "Move down",
        "rm": "Delete",
    }
    choice = choose(actions, describe(row))
    if choice == "open":
        return tasks.task_list(session, sigil("@", name), name, prefix=sigil("@", name) + " ")
    if choice == "add":
        return tasks.add_task(session, prefix=sigil("@", name) + " ")
    if choice == "rename":
        to = ask(f"Rename {name} to", name)
        return session.write("label.rename", name=name, to=to) if to and to != name else ""
    if choice == "merge":
        # For when a typo made a near-duplicate: this one's tasks move to the other.
        into = choose({other: other for other in names if other != name}, f"Merge {name} into")
        return session.write("label.merge", **{"from": name, "into": into}) if into else ""
    if choice == "colour":
        colour = ask(
            f"Colour for {name}: a colour name such as red or teal, or none. The name always "
            "shows too",
        )
        return session.write("label.colour", name=name, colour=colour) if colour else ""
    if choice in ("up", "down"):
        return session.write("label.order", name=name, direction=choice)
    if choice == "rm":
        return delete_label(session, name)
    return ""


def delete_label(session: Session, name: str) -> str:
    if not confirm(f"Delete {name}? Tasks wearing it stay; they just stop showing it."):
        return ""
    return session.write("label.rm", name=name)


# ---------------------------------------------------------------------------------------
# Saved filters
# ---------------------------------------------------------------------------------------


def saved_filters(session: Session) -> str:
    """Saved filters, and the lists they open.

    A filter is stored as text and evaluated when it is used, so one saved as "today" still
    means today next month (§6.2).
    """
    state = {"heading": "Filters"}

    def build():
        listing = session.call("filter.list")
        state["heading"] = f"Filters, {listing.get('announcement', '')}"
        items = [
            dialogs.DynamicMenuItem(title="A one-off query", action=lambda: ad_hoc_query(session)),
            dialogs.DynamicMenuItem(title="Add a filter", action=lambda: add_filter(session)),
        ]
        for saved in listing.get("filters", []):
            items.append(
                dialogs.DynamicMenuItem(
                    title=f"{saved['name']}, {saved['query']}",
                    action=(lambda saved=saved: filter_actions(session, saved)),
                    delete=(lambda saved=saved: delete_filter(session, saved["name"])),
                )
            )
        return items

    live_menu(session, build, lambda: state["heading"])
    return ""


def ad_hoc_query(session: Session) -> str:
    """A filter typed now rather than saved; `search: words` looks through titles and notes."""
    query = tasks.assisted_input(session, "Filter", "filter", history_key="lumenna-filter")
    if not query:
        return ""
    return tasks.task_list(session, query, "Query")


def add_filter(session: Session) -> str:
    name = ask("Name for the new filter")
    if name is None:
        return ""
    query = tasks.assisted_input(session, f"Query for {name}", "filter", history_key="lumenna-filter")
    if not query:
        return ""
    return session.write("filter.add", name=name, query=query)


def filter_actions(session: Session, saved: dict) -> str:
    name = saved["name"]
    actions = {
        "open": "Show its tasks",
        "rename": "Rename",
        "query": "Change the query",
        "up": "Move up",
        "down": "Move down",
        "rm": "Delete",
    }
    choice = choose(actions, f"{name}, {saved['query']}")
    if choice == "open":
        return tasks.task_list(session, saved["query"], name)
    if choice == "rename":
        to = ask(f"Rename {name} to", name)
        return session.write("filter.edit", name=name, rename=to) if to and to != name else ""
    if choice == "query":
        query = dialogs.request_input(f"Query for {name}", default_text=saved["query"])
        if not query or query == saved["query"]:
            return ""
        return session.write("filter.edit", name=name, query=query)
    if choice in ("up", "down"):
        return session.write("filter.order", name=name, direction=choice)
    if choice == "rm":
        return delete_filter(session, name)
    return ""


def delete_filter(session: Session, name: str) -> str:
    if not confirm(f"Delete the filter {name}? The tasks it shows are not touched."):
        return ""
    return session.write("filter.rm", name=name)
