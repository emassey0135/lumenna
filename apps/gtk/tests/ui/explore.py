"""Presses keys and reports, after each, what has focus, what was announced and what Orca said.

    dbus-run-session -- ./headless.sh python3 explore.py Down Down F6 space

`type:text` types text. Seeded with a small store: two projects, a task with subtasks.
"""
import sys
from harness import Session

SEED = [
    ("project", "add", "Work"),
    ("project", "add", "Reports", "--parent", "Work"),
    ("task", "add", "Buy milk"),
    ("task", "add", "Write the report tomorrow p1 #Work"),
    ("task", "add", "Outline #Work"),
    ("task", "add", "Find sources #Work"),
    ("label", "add", "calls"),
    ("filter", "add", "Urgent", "p1"),
]

session = Session(SEED)
try:
    # The subtasks, by title, under the report.
    rows = session.lum("task", "list", "--json")
    import json
    ids = {row["title"]: row["id"] for row in json.loads(rows)["rows"]}
    session.lum("task", "move", ids["Outline"], "--parent", ids["Write the report"])
    session.lum("task", "move", ids["Find sources"], "--parent", ids["Write the report"])
    session.press("space", wait=1.2)  # nothing focused yet; let the one-second refresh land
    print("start:", session.focus(), session.speech())
    session.said()
    for key in sys.argv[1:]:
        if key.startswith("type:"):
            session.type(key[5:])
        else:
            session.press(key)
        print(f"{key}: {session.focus()}  said={session.said()}  speech={session.speech()}")
finally:
    print(open(session.directory + "/app.log").read()[-3000:])
    session.close()
