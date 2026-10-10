"""Projects, labels and saved filters: opened, and also made, renamed, reordered and
removed — each by the row's own actions, as the core offers them.

A name is written the way the filter and quick-add languages read it, as the core writes it
(`form.project_reference`, `form.label_reference`).
"""

from __future__ import annotations

import actions
from rows import Tree, describe
from session import Command, Session, ask, empty_then, live_menu, row_item, screen
import tasks


def project_reference(session: Session, name: str) -> str:
    """A project as the filter and quick-add languages name it: `#Work`, `#"Home Office"`."""
    return session.call("form.project_reference", name=name).get("value", "")


def label_reference(session: Session, name: str) -> str:
    """A label as the filter and quick-add languages name it: `@calls`, `@"deep work"`."""
    return session.call("form.label_reference", name=name).get("value", "")


# ---------------------------------------------------------------------------------------
# Projects
# ---------------------------------------------------------------------------------------


def project_tasks(session: Session, name: str, row=None) -> str:
    """A project's tasks; a task added there lands there, and its main menu can add a
    project inside it, when the project offers that."""
    inside = [
        Command(actions.named(action), lambda _, action=action: actions.act(session, action, row), key="p")
        for action in actions.of_kind(row, "new_inside")
    ]
    reference = project_reference(session, name)
    return tasks.task_list(session, reference, name, prefix=reference + " ", more=inside)


def projects(session: Session) -> str:
    """The project tree, foldable, with weights and what is archived. Enter shows a project's
    tasks; the rest is on its context menu."""
    state = {"heading": "Projects"}

    def build():
        listing = session.call("project.list")
        state["heading"] = f"Projects, {listing.get('announcement', '')}"
        return Tree(listing.get("rows", [])).items()

    def tasks_of(row):
        return project_tasks(session, row["title"], row)

    with screen("lumenna-organise"):
        live_menu(
            session, build, lambda: state["heading"],
            main=tasks_of,
            context=lambda rows: actions.commands(session, rows, before=[
                Command("Show its tasks", tasks_of),
                Command("Add a task to it", lambda row: tasks.add_task(session, prefix=project_reference(session, row["title"]) + " "), key="t"),
            ]),
            app=[
                *actions.heading_command(session, "Projects"),
                *tasks.undo_commands(session),
            ],
            app_title="Projects menu",
        )
    return ""


# ---------------------------------------------------------------------------------------
# Labels
# ---------------------------------------------------------------------------------------


def label_tasks(session: Session, name: str) -> str:
    reference = label_reference(session, name)
    return tasks.task_list(session, reference, name, prefix=reference + " ")


def labels(session: Session) -> str:
    """Labels: a first-class axis, with their own list. Enter shows the tasks wearing
    one; the rest is on its context menu."""
    state = {"heading": "Labels"}

    def build():
        listing = session.call("label.list")
        state["heading"] = f"Labels, {listing.get('announcement', '')}"
        state["empty"] = listing.get("empty", "")
        return [row_item(row, describe(row), navigation_title=row["title"]) for row in listing.get("rows", [])]

    def tasks_of(row):
        return label_tasks(session, row["title"])

    with screen("lumenna-organise"):
        live_menu(
            session, build, lambda: state["heading"],
            main=tasks_of,
            context=lambda rows: actions.commands(session, rows, before=[
                Command("Show the tasks wearing it", tasks_of),
                Command("Add a task wearing it", lambda row: tasks.add_task(session, prefix=label_reference(session, row["title"]) + " "), key="t"),
            ]),
            app=[
                *actions.heading_command(session, "Labels"),
                *tasks.undo_commands(session),
            ],
            empty=empty_then(state, "Press a to add one."),
            app_title="Labels menu",
        )
    return ""


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
        state["empty"] = listing.get("empty", "")
        return [
            row_item({"title": saved["name"], **saved}, f"{saved['name']}, {saved['query']}", navigation_title=saved["name"])
            for saved in listing.get("filters", [])
        ]

    with screen("lumenna-organise"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: tasks.task_list(session, row["query"], row["name"]),
            context=lambda rows: actions.commands(session, rows),
            app=[
                *actions.heading_command(session, "Filters"),
                Command("Search or filter now", lambda _: tasks.query_tasks(session), key="/"),
                *tasks.undo_commands(session),
            ],
            empty=empty_then(state, "Press a to add one, or slash to filter now."),
            app_title="Filters menu",
        )
    return ""


def add_filter(session: Session, action=None, row=None) -> str:
    """The new saved filter's form: a name, then its query, completed as it is typed."""
    naming, querying = session.words("form.new_filter")
    name = ask(actions.prompt(actions.text_prompt(naming), naming["label"]))
    if name is None:
        return ""
    query = tasks.assisted_input(
        session, actions.prompt(actions.text_prompt(querying), querying["label"]), "filter", history_key="lumenna-filter",
    )
    if not query:
        return ""
    return session.write("filter.add", name=name, query=query)


actions.FORMS[("filter", "new")] = add_filter
