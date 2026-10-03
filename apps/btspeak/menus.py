"""The app: menus, forms and the flows between them.

Everything reachable here goes through `lum rpc`'s typed surface (§12). Nothing parses prose,
nothing computes a due date, nothing decides what completing a task does to its subtasks —
that is all core's, and a UI that reimplemented any of it would be the business-logic leak
principle 2 forbids.

Two device conventions this follows rather than invents:

- **Left and right fold a branch.** They are already bound to left-arrow/Dot7 and
  right-arrow/Dot8, so a tree needs no key handling of its own (§16.11).
- **Nothing is announced twice.** A menu action's return value is spoken by the menu itself,
  so an action that has something to say returns it rather than opening a dialog.
"""

from __future__ import annotations

from BTSpeak import dialogs

from client import Disconnected, LumennaError
from rows import Tree, describe


#: The JSON shapes this app was written against (§15). The server reports its own; if they
#: disagree, one of the two is guessing, and §15 says a client that does not know the number
#: should refuse rather than guess.
CONTRACT = 1

#: How often a menu wakes to notice that the store changed underneath it.
#:
#: §16.11 warns that live refresh costs battery, and prefers pushed updates. This is the
#: cheap half of both: the push has already arrived and set a flag, and this only decides how
#: long a stale list can sit on screen. One second is what the dialog library itself uses
#: whenever any item has an idle or hint handler, so it is the device's own normal.
REFRESH = 1


class Session:
    """A client, plus the bookkeeping the menus share."""

    def __init__(self, client) -> None:
        self.client = client
        self.dirty = False

    def call(self, method: str, **params):
        """Calls, and turns a refusal into something worth hearing.

        Every message the server sends is written to be spoken — §6.3 says the text is the
        only channel, there being no squiggle to point at — so it is passed through rather
        than wrapped in something of ours.
        """
        return self.client.call(method, **params)

    def wrote(self) -> None:
        """Records that this app changed something.

        Our own writes do not come back as `lumenna/changed`: `data_version` does not move
        for the connection that wrote, which is the point of that pragma. So a list refreshes
        after our edits because we say so, and after anyone else's because the server does.
        """
        self.dirty = True

    def restless(self) -> bool:
        """Whether anything has changed under whatever is on screen."""
        return self.dirty or self.client.changed.is_set()

    def settle(self) -> bool:
        """Whether to rebuild, clearing the reason."""
        changed = self.dirty or self.client.take_changed()
        self.dirty = False
        return changed


def run(client) -> int:
    """Checks the server is one we understand, then hands over to the main menu."""
    try:
        server = client.call("initialize")
    except Disconnected as error:
        dialogs.show_message(f"Could not start Lumenna. {error.message}")
        return 1

    if server.get("contract") != CONTRACT:
        dialogs.show_message(
            f"This app speaks version {CONTRACT} of Lumenna's data format and the installed "
            f"`lum` speaks version {server.get('contract')}. Update whichever is older; "
            "guessing at the difference would be worse than stopping."
        )
        return 1

    session = Session(client)
    try:
        main_menu(session)
    except Disconnected as error:
        dialogs.show_message(error.message)
        return 1
    return 0


# ---------------------------------------------------------------------------------------
# The top
# ---------------------------------------------------------------------------------------


def main_menu(session: Session) -> None:
    """Everything else, one level down."""
    item = dialogs.DynamicMenuItem
    dialogs.dynamic_menu(
        [
            item(title="Today", shortcut="t", action=lambda: day_plan(session)),
            item(title="Tasks", shortcut="k", action=lambda: task_list(session)),
            item(title="Add a task", shortcut="a", action=lambda: add_task(session)),
            item(title="Filters", shortcut="f", action=lambda: saved_filters(session)),
            item(title="Projects", shortcut="p", action=lambda: projects(session)),
            item(title="Labels", shortcut="l", action=lambda: labels(session)),
            item(title="Blocks", shortcut="b", action=lambda: blocks(session)),
            item(title="Settings", shortcut="s", action=lambda: settings(session)),
        ],
        title="Lumenna",
    )


# ---------------------------------------------------------------------------------------
# Tasks
# ---------------------------------------------------------------------------------------


def task_list(session: Session, query: str = "", title: str = "Tasks") -> str:
    """A filtered list of tasks, as a foldable tree.

    Rebuilt whenever the store moves — by us or by another process — and reopened where the
    cursor was, because losing your place in a list is the sort of thing that makes a UI
    unusable without sight.
    """
    selection = 0
    while True:
        try:
            result = session.call("task.list", query=query) if query else session.call("task.list")
        except LumennaError as error:
            return error.message

        heading = title
        spoken = result.get("announcement", "")
        readback = (result.get("query") or {}).get("description")
        if readback:
            # §6.2: a mis-parsed filter shows wrong results silently, and wrong results are
            # invisible. So the query is read back before its results are.
            heading = f"{title}: {readback}"
        for unresolved in (result.get("query") or {}).get("unresolved", []):
            suggestion = unresolved.get("suggestion")
            hint = f", did you mean {suggestion}?" if suggestion else ""
            dialogs.show_message(
                f"No {unresolved['kind']} called {unresolved['name']}{hint}", wait=False
            )

        rows = result.get("rows", [])
        if not rows:
            return spoken

        tree = Tree(rows)
        choice = dialogs.dynamic_menu(
            tree.items(lambda row: task_actions(session, row)),
            title=f"{heading}, {spoken}",
            exit_condition=session.restless,
            refresh_interval=REFRESH,
            default=min(selection, len(rows) - 1),
        )
        if not session.settle():
            # The menu closed because the user left it, not because anything moved.
            return ""
        selection = choice.key if choice else 0


def task_actions(session: Session, row: dict) -> str:
    """What can be done to one task."""
    identifier = row["id"]
    actions = {
        "done": "Complete",
        "edit": "Edit",
        "show": "Details",
        "move": "Move to a project",
        "assign": "Put in a block",
        "rm": "Delete",
    }
    choice = dialogs.request_choice(actions, describe(row))
    if choice is None:
        return ""

    try:
        if choice.key == "done":
            return session_write(session, "task.done", id=identifier)
        if choice.key == "rm":
            return session_write(session, "task.rm", id=identifier)
        if choice.key == "show":
            return task_details(session, identifier)
        if choice.key == "edit":
            return edit_task(session, identifier)
        if choice.key == "move":
            return move_task(session, identifier)
        if choice.key == "assign":
            return assign_task(session, identifier)
    except LumennaError as error:
        return error.message
    return ""


def session_write(session: Session, method: str, **params) -> str:
    """Calls something that changes the store, and says what it did."""
    try:
        result = session.call(method, **params)
    except LumennaError as error:
        return error.message
    session.wrote()
    return result.get("announcement", "")


def task_details(session: Session, identifier: str) -> str:
    """Everything about one task, as lines to pan through.

    A detail view is where the near-universal states are worth having, so this shows the full
    set rather than the notable ones a list line carries (§13).
    """
    task = session.call("task.show", id=identifier)
    lines = [task["title"]]
    if task.get("project"):
        lines.append(f"project: {task['project']}")
    if task.get("priority", 4) != 4:
        lines.append(f"priority: {task['priority']}")
    if task.get("due"):
        due = task["due"]
        if task.get("due_time"):
            due = f"{due} at {task['due_time']}"
        if task.get("recurrence"):
            due = f"{due}, repeating"
        lines.append(f"due: {due}")
    if task.get("estimate_mins"):
        lines.append(f"estimate: {task['estimate_mins']} minutes")
    if task.get("labels"):
        lines.append("labels: " + ", ".join(task["labels"]))
    for dependency in task.get("depends", []):
        lines.append(f"waits for: {dependency['title']}")
    if task.get("state"):
        lines.append("state: " + ", ".join(task["state"]))
    if task.get("notes"):
        lines.append(f"notes: {task['notes']}")
    dialogs.view_lines(lines, wrap=True)
    return ""


def edit_task(session: Session, identifier: str) -> str:
    """One form over the fields `task.edit` takes.

    Every field is sent as text and parsed by core: `due` goes through the same date grammar
    the quick-add line uses, so "next friday" means here exactly what it means there. The
    dialog library has its own `request_date`, with its own flexible parsing — using it would
    put a second date parser in the product, which §6.2 warns against by name.
    """
    task = session.call("task.show", id=identifier)
    due = task.get("due") or ""
    if task.get("due_time"):
        due = f"{due} {task['due_time']}"

    answers = dialogs.request_form(
        [
            dialogs.InputField(key="title", prompt="Title", default_text=task["title"]),
            dialogs.InputField(
                key="due",
                prompt="Due",
                default_text=due,
                format_hint="a date phrase such as tomorrow, next friday, or none",
            ),
            dialogs.InputField(
                key="priority",
                prompt="Priority 1 to 4",
                default_text=str(task.get("priority", 4)),
                validate=lambda text: text.strip() in {"1", "2", "3", "4"},
                format_hint="1 to 4, where 1 is highest",
            ),
            dialogs.InputField(
                key="estimate",
                prompt="Estimate",
                default_text=(
                    f"{task['estimate_mins']}m" if task.get("estimate_mins") else ""
                ),
                format_hint="a duration such as 45m or 1h30m, or none",
            ),
            dialogs.InputField(
                key="notes", prompt="Notes", field_type="multiline",
                default_text=task.get("notes", ""),
            ),
        ]
    )
    if answers is None:
        return ""

    changes = {"id": identifier}
    if answers["title"] != task["title"]:
        changes["title"] = answers["title"]
    if answers["due"].strip() != due.strip():
        changes["due"] = answers["due"].strip() or "none"
    if answers["priority"].strip() != str(task.get("priority", 4)):
        changes["priority"] = int(answers["priority"])
    if answers["estimate"].strip() != (
        f"{task['estimate_mins']}m" if task.get("estimate_mins") else ""
    ):
        changes["estimate"] = answers["estimate"].strip() or "none"
    if answers["notes"] != task.get("notes", ""):
        changes["notes"] = answers["notes"]

    if len(changes) == 1:
        return "nothing changed"
    return session_write(session, "task.edit", **changes)


def move_task(session: Session, identifier: str) -> str:
    """Puts a task in another project."""
    names = [row["title"] for row in session.call("project.list").get("rows", [])]
    if not names:
        return "there are no projects"
    choice = dialogs.request_choice(names, "Move to")
    if choice is None:
        return ""
    return session_write(session, "task.move", id=identifier, project=choice.label)


def add_task(session: Session) -> str:
    """Capture, in the quick-add grammar (§6.1).

    The line is previewed before it is written, because an unknown project is an error and an
    unknown label is a new label — and being told which of those just happened *after* the
    fact is not the same thing (§3.4).
    """
    text = assisted_input(session, "Task", "quick-add", history_key="lumenna-add")
    if not text:
        return ""

    try:
        preview = session.call("preview", text=text)
    except LumennaError as error:
        return error.message

    if preview.get("has_errors"):
        problems = "; ".join(
            diagnostic["message"]
            for diagnostic in preview.get("diagnostics", [])
            if diagnostic["severity"] == "error"
        )
        dialogs.show_message(problems)
        return ""

    written = session_write(session, "task.add", text=text)
    notices = [
        diagnostic["message"]
        for diagnostic in preview.get("diagnostics", [])
        if diagnostic["severity"] == "notice"
    ]
    return ", ".join([written, *notices]) if notices else written


def assisted_input(
    session: Session, prompt: str, syntax: str, history_key: str | None = None
) -> str | None:
    """Text entry, with a completion pass afterwards.

    `InputDialog` has no hook for completing as you type — Tab is form navigation — so this
    offers candidates once the line is entered instead, whenever the last word looks like a
    name that was started and not finished. It is the honest version of §6.3's completion on
    this toolkit: worth having for `#wo` when you cannot remember whether the project is Work
    or Workshop, and not pretending to be inline.

    Making it inline means subclassing `InputDialog` to bind a key to `complete`, which is
    the same shape of change §16.11 describes for the menu's speech and braille, and worth
    doing once the rest is in daily use.
    """
    text = dialogs.request_input(prompt, history_key=history_key)
    if not text:
        return text

    try:
        found = session.call("complete", text=text, cursor=len(text), syntax=syntax)
    except LumennaError:
        return text

    start, end = found.get("start", 0), found.get("end", 0)
    partial = text[start:end]
    candidates = found.get("candidates", [])
    if not partial or not candidates:
        return text
    if any(candidate["text"].lower() == partial.lower() for candidate in candidates):
        return text

    choice = dialogs.request_choice(
        {candidate["text"]: candidate["label"] for candidate in candidates},
        found.get("announcement", "Completions"),
    )
    if choice is None:
        return text
    return text[:start] + choice.key + text[end:]


# ---------------------------------------------------------------------------------------
# The day
# ---------------------------------------------------------------------------------------


def day_plan(session: Session, date: str = "") -> str:
    """A day's blocks and what is assigned to them.

    §16.11: the timeline has no visual metaphor here, and blocks as a time-ordered list lose
    nothing. Assignments sit under their block as a second level, so the same folding works.
    """
    selection = 0
    while True:
        try:
            plan = session.call("plan", date=date) if date else session.call("plan")
        except LumennaError as error:
            return error.message

        rows = []
        for block in plan.get("blocks", []):
            rows.append(
                {
                    "id": block["id"],
                    "role": "block",
                    "depth": 0,
                    "title": block["title"],
                    "value": f"{block['start']} to {block['end']}",
                    "state": [],
                }
            )
            for assignment in block.get("assignments", []):
                detail = [assignment["status"]]
                if assignment["minutes"]:
                    detail.append(f"{assignment['minutes']} minutes logged")
                if assignment["capped"]:
                    detail.append("timer looks orphaned")
                rows.append(
                    {
                        "id": assignment["id"],
                        "role": "assignment",
                        "depth": 1,
                        "title": assignment["title"],
                        "state": detail,
                    }
                )

        if not rows:
            return f"{plan.get('date', '')}, nothing planned"

        tree = Tree(rows)
        choice = dialogs.dynamic_menu(
            tree.items(lambda row: plan_actions(session, row)),
            title=plan.get("announcement", "Today"),
            exit_condition=session.restless,
            refresh_interval=REFRESH,
            default=min(selection, len(rows) - 1),
        )
        if not session.settle():
            return ""
        selection = choice.key if choice else 0


def plan_actions(session: Session, row: dict) -> str:
    """Timers, for an assignment; nothing yet for a block."""
    if row["role"] != "assignment":
        return describe(row)
    choice = dialogs.request_choice(
        {"start": "Start the timer", "stop": "Stop the timer", "unassign": "Take it out"},
        row["title"],
    )
    if choice is None:
        return ""
    return session_write(session, choice.key, assignment=row["id"])


def assign_task(session: Session, identifier: str) -> str:
    """Puts a task in one of today's blocks."""
    plan = session.call("plan")
    blocks = plan.get("blocks", [])
    if not blocks:
        return "there are no blocks today"
    choice = dialogs.request_choice(
        {block["series"]: f"{block['title']}, {block['start']}" for block in blocks},
        "Put it in",
    )
    if choice is None:
        return ""
    return session_write(session, "assign", task=identifier, block=choice.key)


def blocks(session: Session) -> str:
    """The block series, and a way to make one."""
    while True:
        listing = session.call("block.list")
        rows = listing.get("rows", [])
        items = [
            dialogs.DynamicMenuItem(title="Add a block", shortcut="a", action=lambda: add_block(session))
        ]
        for row in rows:
            items.append(
                dialogs.DynamicMenuItem(
                    title=describe(row),
                    action=(lambda row=row: session_write(session, "block.rm", id=row["id"].split("@")[0])),
                    hint="Enter deletes this block",
                )
            )
        dialogs.dynamic_menu(
            items,
            title=f"Blocks, {listing.get('announcement', '')}",
            exit_condition=session.restless,
            refresh_interval=REFRESH,
        )
        if not session.settle():
            return ""


def add_block(session: Session) -> str:
    """One block, one-off or repeating."""
    answers = dialogs.request_form(
        [
            dialogs.InputField(key="title", prompt="Name", required=True),
            dialogs.InputField(
                key="at", prompt="Starts at", default_text="9am",
                format_hint="a time such as 9am or 14:30",
            ),
            dialogs.InputField(
                key="minutes", prompt="Minutes", default_text="60",
                validate=lambda text: text.strip().isdigit(),
                format_hint="a whole number of minutes",
            ),
            dialogs.InputField(key="date", prompt="Starting on", default_text="today"),
            dialogs.InputField(
                key="repeat", prompt="Repeating",
                format_hint="a repetition such as every weekday, or blank for a one-off",
            ),
        ]
    )
    if answers is None:
        return ""
    params = {
        "title": answers["title"],
        "at": answers["at"],
        "minutes": int(answers["minutes"]),
        "date": answers["date"],
    }
    if answers["repeat"].strip():
        params["repeat"] = answers["repeat"].strip()
    return session_write(session, "block.add", **params)


# ---------------------------------------------------------------------------------------
# Organisation
# ---------------------------------------------------------------------------------------


def saved_filters(session: Session) -> str:
    """Saved filters, and the lists they open.

    A filter is stored as text and evaluated when it is used, so one saved as "today" still
    means today next month (§6.2).
    """
    listing = session.call("filter.list")
    items = [
        dialogs.DynamicMenuItem(
            title="A one-off query", shortcut="q", action=lambda: ad_hoc_query(session)
        )
    ]
    for saved in listing.get("filters", []):
        items.append(
            dialogs.DynamicMenuItem(
                title=f"{saved['name']}, {saved['query']}",
                action=(
                    lambda saved=saved: task_list(session, saved["query"], saved["name"])
                ),
                hotkeys={
                    "d": (
                        lambda saved=saved: session_write(
                            session, "filter.rm", name=saved["name"]
                        )
                    )
                },
                hint="d deletes this filter",
            )
        )
    dialogs.dynamic_menu(items, title=f"Filters, {listing.get('announcement', '')}")
    return ""


def ad_hoc_query(session: Session) -> str:
    """A filter typed now rather than saved."""
    query = assisted_input(session, "Filter", "filter", history_key="lumenna-filter")
    if not query:
        return ""
    return task_list(session, query, "Query")


def projects(session: Session) -> str:
    """Projects, and the tasks in one."""
    while True:
        listing = session.call("project.list")
        items = [
            dialogs.DynamicMenuItem(
                title="Add a project", shortcut="a", action=lambda: add_named(session, "project")
            )
        ]
        for row in listing.get("rows", []):
            items.append(
                dialogs.DynamicMenuItem(
                    title=describe(row),
                    action=(
                        lambda row=row: task_list(
                            session, f"#{row['title']}", row["title"]
                        )
                    ),
                )
            )
        dialogs.dynamic_menu(
            items,
            title=f"Projects, {listing.get('announcement', '')}",
            exit_condition=session.restless,
            refresh_interval=REFRESH,
        )
        if not session.settle():
            return ""


def labels(session: Session) -> str:
    """Labels, and the tasks wearing one."""
    while True:
        listing = session.call("label.list")
        items = [
            dialogs.DynamicMenuItem(
                title="Add a label", shortcut="a", action=lambda: add_named(session, "label")
            )
        ]
        for row in listing.get("rows", []):
            items.append(
                dialogs.DynamicMenuItem(
                    title=describe(row),
                    action=(
                        lambda row=row: task_list(
                            session, f"@{row['title']}", row["title"]
                        )
                    ),
                )
            )
        dialogs.dynamic_menu(
            items,
            title=f"Labels, {listing.get('announcement', '')}",
            exit_condition=session.restless,
            refresh_interval=REFRESH,
        )
        if not session.settle():
            return ""


def add_named(session: Session, kind: str) -> str:
    """A project or a label, which differ only in the noun."""
    name = dialogs.request_input(f"New {kind}")
    if not name:
        return ""
    return session_write(session, f"{kind}.add", name=name)


def settings(session: Session) -> str:
    """The settings core keeps, edited one at a time."""
    while True:
        current = session.call("config.get").get("settings", [])
        items = [
            dialogs.DynamicMenuItem(
                title=(lambda setting=setting: f"{setting['key']}, {setting['value']}"),
                action=(lambda setting=setting: change_setting(session, setting)),
            )
            for setting in current
        ]
        dialogs.dynamic_menu(items, title="Settings")
        if not session.settle():
            return ""


def change_setting(session: Session, setting: dict) -> str:
    """One setting, as text — core validates it, so this does not."""
    value = dialogs.request_input(setting["key"], default_text=setting["value"])
    if value is None or value == setting["value"]:
        return ""
    return session_write(session, "config.set", key=setting["key"], value=value)
