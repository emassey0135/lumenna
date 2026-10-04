"""Settings, devices and sync, and getting data in and out (§3.12, §7, §9).

Laid out as the phone lays it out: planning settings that sync, this device's backups, the
devices it syncs with, and export and import. Each setting is offered the way its value is
shaped — a choice where there are only a few, a line of text where core reads a phrase — and
core validates every one of them, so nothing here does.
"""

from __future__ import annotations

import datetime
import queue
from pathlib import Path

from BTSpeak import dialogs

from client import LumennaError
from session import REFRESH, Session, ask, choose, confirm, live_menu, spoken


WEEKDAYS = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]

#: Each setting's name in words, and the values to choose from when there are only a few.
SETTINGS = {
    "cascade-complete-subtasks": (
        "Completing a task completes its subtasks", {"true": "Yes", "false": "No"},
    ),
    "day-start": ("Day starts", None),
    "day-end": ("Day ends", None),
    "all-day-reminder-hour": ("All-day reminders at", None),
    "verbosity": ("Announcements", {"full": "Full sentences", "terse": "Terse"}),
    "week-start": ("Week starts on", {day: day.capitalize() for day in WEEKDAYS}),
    "backup-every": (
        "Automatic backups",
        {"12h": "Every 12 hours", "1d": "Every day", "7d": "Every week", "off": "Off"},
    ),
    "backup-keep": ("Backups kept", None),
    "backup-dir": ("Backups go to", None),
}

PLANNING = ["cascade-complete-subtasks", "day-start", "day-end", "all-day-reminder-hour", "verbosity", "week-start"]
BACKUPS = ["backup-every", "backup-keep", "backup-dir"]


def settings(session: Session) -> str:
    """A short list of pages, as on the phone."""
    items = [
        dialogs.DynamicMenuItem(title="Planning", action=lambda: setting_page(session, "Planning", PLANNING)),
        dialogs.DynamicMenuItem(title="Devices and sync", action=lambda: devices(session)),
        dialogs.DynamicMenuItem(title="Backups, on this device only", action=lambda: backups(session)),
        dialogs.DynamicMenuItem(title="Export and import", action=lambda: export_import(session)),
    ]
    dialogs.dynamic_menu(items, title="Settings")
    return ""


def current_settings(session: Session) -> dict:
    return {s["key"]: s["value"] for s in session.call("config.get").get("settings", [])}


def setting_items(session: Session, keys: list[str]) -> list:
    values = current_settings(session)
    items = []
    for key in keys:
        if key not in values:
            continue
        name, options = SETTINGS.get(key, (key, None))
        shown = options.get(values[key], values[key]) if options else values[key]
        items.append(
            dialogs.DynamicMenuItem(
                title=f"{name}, {shown}",
                action=(lambda key=key: change_setting(session, key, values[key])),
            )
        )
    return items


def setting_page(session: Session, title: str, keys: list[str]) -> str:
    live_menu(session, lambda: setting_items(session, keys), title)
    return ""


def change_setting(session: Session, key: str, value: str) -> str:
    name, options = SETTINGS.get(key, (key, None))
    if options:
        chosen = choose(options, name, default=value)
    else:
        chosen = dialogs.request_input(name, default_text=value)
    if chosen is None or chosen == value:
        return ""
    return session.write("config.set", key=key, value=chosen)


# ---------------------------------------------------------------------------------------
# Devices, sync and pairing (§7)
# ---------------------------------------------------------------------------------------


def ago(stamp: str | None) -> str:
    """`5 minutes ago`, as the command line says it."""
    if not stamp:
        return ""
    try:
        then = datetime.datetime.fromisoformat(stamp.replace("Z", "+00:00"))
    except ValueError:
        return stamp
    seconds = max(0, int((datetime.datetime.now(datetime.timezone.utc) - then).total_seconds()))
    for size, unit in ((86400, "day"), (3600, "hour"), (60, "minute")):
        if seconds >= size:
            n = seconds // size
            return f"{n} {unit}{'s' if n != 1 else ''} ago"
    return "just now"


def device_line(device: dict) -> str:
    """One device as a sentence: what it is, and how syncing with it last went. Words rather
    than a symbol, because §9 is explicit that a glyph communicates nothing."""
    line = f"{device['name']}, {device['platform']}"
    if device.get("this_device"):
        return f"{line}, this device"
    if device.get("last_error") and device.get("last_attempt"):
        line += f", last attempt {ago(device['last_attempt'])} failed: {device['last_error']}"
        if device.get("last_success"):
            line += f"; last synced {ago(device['last_success'])}"
    elif device.get("last_success"):
        line += f", last synced {ago(device['last_success'])}"
    else:
        line += ", not synced yet"
    return line


def devices(session: Session) -> str:
    """How syncing is going, device by device, and pairing another."""
    state = {"heading": "Devices and sync"}

    def build():
        status = session.call("sync.status")
        state["heading"] = spoken(status)
        if status.get("devices") and not status.get("running"):
            # The daemon is what syncs in the background here (§16.11), and setting it up
            # asks for an administrator, which nothing in a menu can do.
            state["heading"] += (
                ". To keep in sync in the background, run lum daemon install once from a shell"
            )
        items = [
            dialogs.DynamicMenuItem(title="Sync now", action=lambda: sync_now(session)),
            dialogs.DynamicMenuItem(title="Pair a device", action=lambda: pair(session)),
        ]
        for device in status.get("devices", []):
            items.append(
                dialogs.DynamicMenuItem(
                    title=device_line(device),
                    action=(lambda device=device: device_actions(session, device)),
                )
            )
        return items

    live_menu(session, build, lambda: state["heading"])
    return ""


def sync_now(session: Session) -> str:
    """One round with every paired device — through the daemon when it runs, as it should
    on this device (§16.11)."""
    try:
        # A device that cannot be reached takes a while to give up on, longer than the
        # client's usual wait.
        with dialogs.activity("Syncing"):
            report = session.client.begin("sync").result(timeout=180)
    except LumennaError as error:
        return error.message
    session.wrote()
    lines = [spoken(report)]
    for peer in report.get("peers", []):
        if peer.get("error"):
            lines.append(f"{peer['name']}: not synced, {peer['error']}")
        elif peer.get("changed"):
            lines.append(f"{peer['name']}: brought in {', '.join(peer['changed'])}")
        else:
            lines.append(f"{peer['name']}: synced, nothing new from it")
    return ". ".join(lines)


def device_actions(session: Session, device: dict) -> str:
    actions = {"rename": "Rename"}
    if not device.get("this_device"):
        actions["unpair"] = "Stop syncing with it"
    choice = choose(actions, device["name"])
    if choice == "rename":
        name = ask(f"New name for {device['name']}", device["name"])
        if not name or name == device["name"]:
            return ""
        return session.write("device.rename", device=device["node_id"], name=name)
    if choice == "unpair":
        if not confirm(
            f"Stop syncing with {device['name']}? It keeps what it already has: this is for a "
            "device you replaced, not one that was stolen."
        ):
            return ""
        return session.write("device.unpair", device=device["node_id"])
    return ""


def pair(session: Session) -> str:
    """Pairs with another of the person's devices, comparing three words on both (§7).

    The pairing runs in the server and talks through notifications: the code to give the
    other device while this one waits, then the words. This shows each as it arrives and
    answers with `pair.confirm`, so the menu stays responsive the whole time.
    """
    how = choose(
        {
            "wait": "Wait for the other device: on this network it finds this one, or give it a code",
            "code": "Type the code the other device shows",
        },
        "Pair a device",
    )
    if how is None:
        return ""
    params = {}
    if how == "code":
        code = ask("The other device's pairing code")
        if code is None:
            return ""
        params["code"] = code.replace(" ", "")

    client = session.client
    while not client.pairing.empty():
        client.pairing.get_nowait()
    pending = client.begin("pair", **params)
    shown = {
        "status": "Connecting to the other device" if params else "Starting",
        "code": "",
        "cancel": False,
    }

    def read_code() -> str:
        if shown["code"]:
            dialogs.view_lines([shown["code"]])
        return ""

    def cancel(menu) -> None:
        shown["cancel"] = True
        menu.close()

    while True:
        items = [
            dialogs.DynamicMenuItem(title=lambda: shown["status"], action=read_code),
            dialogs.DynamicMenuItem(title="Cancel the pairing", action=cancel),
        ]
        choice = dialogs.dynamic_menu(
            items,
            title="Pairing",
            exit_condition=lambda: pending.done() or not client.pairing.empty(),
            refresh_interval=REFRESH,
        )
        if pending.done():
            break
        try:
            event = client.pairing.get_nowait()
        except queue.Empty:
            event = None
        if event is None:
            # The person left, or chose to cancel.
            if choice is None or shown["cancel"]:
                try:
                    client.call("pair.cancel")
                except LumennaError:
                    pass  # It ended on its own in the meantime.
                break
            continue
        if event.get("code"):
            shown["code"] = event["code"]
            shown["status"] = (
                f"Waiting to pair, as {event.get('name', 'this device')}. On the other device, "
                "pair too while on this network, or type this code there. Enter reads the code "
                f"a character at a time: {event['code']}"
            )
        elif event.get("words"):
            words = ", ".join(event["words"])
            matched = dialogs.request_confirmation(
                f"The words are: {words}. Do the same three words show on the other device?",
                default=False,
            )
            client.call("pair.confirm", match=matched)
            shown["status"] = "Finishing" if matched else "Refusing"

    try:
        paired = pending.result(timeout=60)
    except LumennaError as error:
        return error.message
    session.wrote()
    return spoken(paired)


# ---------------------------------------------------------------------------------------
# Backups, export and import (§9)
# ---------------------------------------------------------------------------------------


def backups(session: Session) -> str:
    """This device's backups: how often, how many, where — and one now, or one merged in."""

    def build():
        return [
            dialogs.DynamicMenuItem(title="Back up now", action=lambda: session.write("backup")),
            dialogs.DynamicMenuItem(title="Restore from a backup", action=lambda: restore(session)),
        ] + setting_items(session, BACKUPS)

    live_menu(
        session,
        build,
        "Backups. A backup holds your whole history, including every task you deleted, so the "
        "store can be rebuilt from it. It stays on this device",
    )
    return ""


def restore(session: Session) -> str:
    """Merges a backup in. Nothing in the store is lost; a task deleted since stays deleted."""
    start = current_settings(session).get("backup-dir") or str(Path.home())
    if not Path(start).is_dir():
        start = str(Path.home())
    chosen = dialogs.request_file(start, ["lumbak"], prompt="Restore which backup?")
    if chosen is None:
        return ""
    return session.write("restore", file=str(chosen))


FORMATS = {
    "json": ("JSON, complete, can be imported", "json"),
    "markdown": ("Markdown checklist", "md"),
    "org": ("Org outline", "org"),
    "ics": ("Calendar file of your blocks", "ics"),
}


def export_import(session: Session) -> str:
    items = [
        dialogs.DynamicMenuItem(title="Export", action=lambda: export(session)),
        dialogs.DynamicMenuItem(title="Import an export, or restore a backup", action=lambda: import_file(session)),
    ]
    dialogs.dynamic_menu(
        items,
        title="Export and import. An export is what you have now, with nothing from the trash. "
        "Importing adds what this device lacks and removes nothing",
    )
    return ""


def export(session: Session) -> str:
    """Writes the current state to a file in a folder of the person's choosing."""
    format = choose({key: label for key, (label, _) in FORMATS.items()}, "Export as")
    if format is None:
        return ""
    folder = dialogs.request_directory(str(Path.home()), prompt="Save it in which folder?")
    if folder is None:
        return ""
    name = f"Lumenna {datetime.date.today().isoformat()}.{FORMATS[format][1]}"
    path = Path(folder) / name
    force = False
    if path.exists():
        if not confirm(f"{name} is already there. Replace it?"):
            return ""
        force = True
    return session.write("export", format=format, output=str(path), force=force)


def import_file(session: Session) -> str:
    """Reads a JSON export or a backup; core tells them apart."""
    chosen = dialogs.request_file(str(Path.home()), ["json", "lumbak"], prompt="Import which file?")
    if chosen is None:
        return ""
    return session.write("import", file=str(chosen))
