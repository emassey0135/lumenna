"""Settings, devices and sync, and getting data in and out.

Laid out as the phone lays it out: planning settings that sync, this device's backups, the
devices it syncs with, and export and import. Each setting is offered the way its value is
shaped — a choice where there are only a few, a line of text where core reads a phrase — and
core validates every one of them, so nothing here does.
"""

from __future__ import annotations

import datetime
import queue
from pathlib import Path

from BTSpeak import clipboard, dialogs

import actions
import options
from client import LumennaError
from session import REFRESH, Command, Session, ask, choose, confirm, live_menu, row_item, screen, spoken
from tasks import undo_commands


#: What this app shows of the device settings: `clock` is for clients with no clock of their
#: own, and this device has its Time Format.
NOT_HERE = {"clock"}


def planning(setting: dict) -> bool:
    """Whether a setting is on the Planning page: those that sync to every device."""
    return bool(setting.get("syncs"))


def on_this_device(setting: dict) -> bool:
    """Whether a setting is on the Backups page: this device's own."""
    return not setting.get("syncs") and setting["key"] not in NOT_HERE


def read_back_title() -> str:
    on = options.get(options.READ_BACK, False)
    return f"Read a task back before adding it, {'on' if on else 'off'}"


def toggle_read_back() -> str:
    on = not options.get(options.READ_BACK, False)
    options.put(options.READ_BACK, on)
    return "A task is read back before it is added" if on else "A task is added as soon as it is entered"


def settings(session: Session) -> str:
    """A short list of pages, as on the phone, and this app's own preference."""
    items = [
        dialogs.DynamicMenuItem(title="Planning", shortcut="p", action=lambda: setting_page(session, "Planning", planning)),
        dialogs.DynamicMenuItem(title="Devices and sync", shortcut="d", action=lambda: devices(session)),
        dialogs.DynamicMenuItem(title="Backups, on this device only", shortcut="b", action=lambda: backups(session)),
        dialogs.DynamicMenuItem(title="Export and import", shortcut="e", action=lambda: export_import(session)),
        dialogs.DynamicMenuItem(title=lambda *_: read_back_title(), shortcut="r", action=toggle_read_back),
    ]
    with screen("lumenna-settings"):
        dialogs.dynamic_menu(items, title="Settings")
    return ""


def current_settings(session: Session) -> dict:
    return {s["key"]: s["value"] for s in session.call("config.get").get("settings", [])}


def setting_items(session: Session, which) -> list:
    """The settings `which` keeps, one row each, said as the core names them."""
    items = []
    for setting in session.call("config.get").get("settings", []):
        if not which(setting):
            continue
        named = {o["id"]: o["title"] for o in setting.get("options", [])}
        items.append(
            row_item(
                setting,
                f"{setting['title']}, {named.get(setting['value'], setting['value'])}",
                action=(lambda setting=setting: change_setting(session, setting)),
            )
        )
    return items


def setting_page(session: Session, title: str, which) -> str:
    """Settings, one per row: Enter changes one."""
    with screen("lumenna-settings"):
        live_menu(
            session, lambda: setting_items(session, which), title,
            app=undo_commands(session), app_title=f"{title} menu",
        )
    return ""


#: What a line of text takes, for a setting typed rather than chosen, by its kind.
TAKES = {"time": "a time such as 9am or 14:30", "number": "a whole number"}


def change_setting(session: Session, setting: dict) -> str:
    """A choice where the setting has options; else a line of text, which the core reads and
    checks. The device has a date dialog but no time one, so a time is typed as said."""
    value = setting["value"]
    options = {o["id"]: o["title"] for o in setting.get("options", [])}
    said = ". ".join(part for part in (setting["title"], setting.get("hint", "")) if part)
    if options:
        chosen = choose(options, said, default=value)
    else:
        takes = TAKES.get(setting.get("kind", ""))
        chosen = dialogs.request_input(f"{said}, {takes}" if takes else said, default_text=value)
    if chosen is None or chosen == value:
        return ""
    return session.write("config.set", key=setting["key"], value=chosen)


# ---------------------------------------------------------------------------------------
# Devices, sync and pairing
# ---------------------------------------------------------------------------------------


def device_line(device: dict) -> str:
    """One device as a sentence: what it is, and how syncing with it last went — the status
    the core words for every app. Words rather than a symbol, which
    a screen reader may not say."""
    return ", ".join([device["name"], device["platform"], *device.get("status", [])])


def devices(session: Session) -> str:
    """How syncing is going, device by device, and pairing another."""
    state = {"heading": "Devices and sync"}

    def build():
        status = session.call("sync.status")
        state["heading"] = spoken(status)
        if status.get("devices") and not status.get("running"):
            # The daemon is what syncs in the background here, and setting it up
            # asks for an administrator, which nothing in a menu can do.
            state["heading"] += (
                ". To keep in sync in the background, run lum daemon install once from a shell"
            )
        return [row_item(device, device_line(device), navigation_title=device["name"]) for device in status.get("devices", [])]

    with screen("lumenna-settings"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda device: actions.run_kind(session, device, "rename"),
            context=lambda rows: actions.commands(session, rows),
            app=[
                Command("Sync now", lambda _: sync_now(session), key="s"),
                Command("Pair a device", lambda _: pair(session), key="p"),
            ],
            empty="Not paired with any other device yet. Press p to pair one.",
            app_title="Devices menu",
        )
    return ""


def sync_now(session: Session) -> str:
    """One round with every paired device — through the daemon when it runs, as it should
    on this device."""
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


def pairing_words(session: Session, name: str = "this device") -> dict:
    """Pairing's sentences and buttons, the core's (`form.pairing_words`): on this network
    the device finds the other by itself, through Avahi."""
    return session.words("form.pairing_words", this_device=name, local=True)


def ask_code(session: Session, own: str = "") -> str | None:
    """The other device's code: typed, or, left empty, the clipboard's, where a code sent from
    the other device usually arrives. This device's own code, copied while it waits, is never
    the other's. None if cancelled; empty if there is no code to be had."""
    words = pairing_words(session)
    text = dialogs.request_input(f"{words['their_code']}. {words['empty_means']}", default_text="")
    if text is None:
        return None
    text = text.strip()
    if not text:
        _, pasted = clipboard.paste(False, 200, multiline=False)
        text = pasted.strip()
        if text == own:
            text = ""
    return text.replace(" ", "")


def pair(session: Session) -> str:
    """Pairs with another of the person's devices, comparing three words on both.

    The pairing runs in the server and talks through notifications: the code to give the
    other device while this one waits, then the words. This shows each as it arrives and
    answers with `pair.confirm`, so the menu stays responsive the whole time.
    """
    words = pairing_words(session)
    how = choose(
        {"wait": session.sentence(words["wait"]), "code": session.sentence(words["join"])},
        f"{session.sentence(words['title'])}. {words['intro']}",
    )
    if how is None:
        return ""
    params = {}
    if how == "code":
        code = ask_code(session)
        if code is None:
            return ""
        if not code:
            return words["need_code"]
        params["code"] = code
    return run_pairing(session, params)


def run_pairing(session: Session, params: dict) -> str:
    """One pairing, waiting to be found or joining by `params["code"]`. While waiting, a
    code can be typed instead: the wait is given up and the code joined once it has ended."""
    client = session.client
    while not client.pairing.empty():
        client.pairing.get_nowait()
    pending = client.begin("pair", **params)
    words = pairing_words(session)
    shown = {
        "status": words["connecting"] if params else words["opening"],
        "code": "",
        "cancel": False,
        "instead": None,
    }

    def read_code() -> str:
        if shown["code"]:
            dialogs.view_lines([shown["code"]])
        return ""

    def cancel(menu) -> None:
        shown["cancel"] = True
        menu.close()

    def code_instead(menu) -> None:
        code = ask_code(session, own=shown["code"])
        if code:
            shown["instead"] = code
            menu.close()
        elif code is not None:
            dialogs.show_message(words["need_code"])

    while True:
        items = [
            dialogs.DynamicMenuItem(title=lambda: shown["status"], action=read_code),
            *([] if params else [dialogs.DynamicMenuItem(title=session.sentence(words["join"]), action=code_instead)]),
            dialogs.DynamicMenuItem(title="Cancel", action=cancel),
        ]
        choice = dialogs.dynamic_menu(
            items,
            title=session.sentence(words["title"]),
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
            # The person left, chose to cancel, or typed the other device's code instead.
            if choice is None or shown["cancel"] or shown["instead"]:
                try:
                    client.call("pair.cancel")
                except LumennaError:
                    pass  # It ended on its own in the meantime.
                if shown["instead"]:
                    try:
                        pending.result(timeout=60)
                    except LumennaError:
                        pass  # The wait, given up.
                    dialogs.show_message(words["switching"], wait=False)
                    return run_pairing(session, {"code": shown["instead"]})
                break
            continue
        if event.get("code"):
            shown["code"] = event["code"]
            # Copied, as every app does: the Blazie clipboard is the desktop's too.
            clipboard.copy(event["code"], False)
            named = pairing_words(session, event.get("name") or "this device")
            # Enter on the line shows the code alone, to read a character at a time.
            shown["status"] = f"{named['waiting']} {named['copied']} {named['my_code']}: {event['code']}"
        elif event.get("words"):
            matched = dialogs.request_confirmation(
                f"{session.sentence(words['match_title'])} {words['match_message']} {', '.join(event['words'])}",
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
# Backups, export and import
# ---------------------------------------------------------------------------------------


def backups(session: Session) -> str:
    """This device's backups: how often, how many, where — and one now, or one merged in."""

    with screen("lumenna-settings"):
        live_menu(
            session,
            lambda: setting_items(session, on_this_device),
            "Backups. A backup holds your whole history, including every task you deleted, so the "
            "store can be rebuilt from it. It stays on this device",
            app=[
                Command("Back up now", lambda _: session.write("backup"), key="b"),
                Command("Restore from a backup", lambda _: restore(session), key="r"),
            ],
            app_title="Backups menu",
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
