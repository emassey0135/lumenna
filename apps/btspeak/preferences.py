"""Settings, devices and sync, and getting data in and out.

Laid out as the phone lays it out: planning settings that sync, this device's backups, the
devices it syncs with, and export and import. Each setting is offered the way its value is
shaped — a choice where there are only a few, a line of text where the core reads a phrase —
and the core checks every one of them, so nothing here does. The settings, the devices and
pairing are the app's (`ViewModel::settings`, `devices`), opened as their pages open and put
away as they close.
"""

from __future__ import annotations

import contextlib
import time
from pathlib import Path

from BTSpeak import clipboard, dialogs

import actions
from actions import L
from core import refused
import options
import reminder_lists
from rows import Row, items
from session import REFRESH, Command, Session, choose, confirm, live_menu, screen
from tasks import undo_commands


#: What this app shows of the device settings: `clock` is for clients with no clock of their
#: own, and this device has its Time Format.
NOT_HERE = {"clock"}

#: How long a sync round may take before it is given up on: a device that cannot be reached
#: takes a while to give up on.
SYNC_FOR = 180

#: How long the Devices page waits for the network to say how syncing is going, in seconds.
STATUS_FOR = 5


def planning(setting) -> bool:
    """Whether a setting is on the Planning page: those that sync to every device."""
    return setting.syncs


def on_this_device(setting) -> bool:
    """Whether a setting is on the Backups page: this device's own."""
    return not setting.syncs and setting.key not in NOT_HERE and setting.key not in reminder_lists.KEYS


def read_back_title() -> str:
    on = options.get(options.READ_BACK, False)
    return f"Read a task back before adding it, {'on' if on else 'off'}"


def toggle_read_back() -> str:
    on = not options.get(options.READ_BACK, False)
    options.put(options.READ_BACK, on)
    return "A task is read back before it is added" if on else "A task is added as soon as it is entered"


@contextlib.contextmanager
def opened(session: Session):
    """The app's settings, open while a page is shown: opened here unless a page opened them."""
    if session.view().settings is not None:
        yield
        return
    session.send(L.ShellEventSettings(L.SettingsEventOpen()))
    try:
        yield
    finally:
        session.send(L.ShellEventSettings(L.SettingsEventClose()))


def settings(session: Session) -> str:
    """A short list of pages, as on the phone, and this app's own preference."""
    menu = [
        dialogs.DynamicMenuItem(title="Planning", shortcut="p", action=lambda: setting_page(session, "Planning", planning)),
        dialogs.DynamicMenuItem(title="Devices and sync", shortcut="d", action=lambda: devices(session)),
        dialogs.DynamicMenuItem(title="Reminders, on this device", shortcut="m", action=lambda: reminder_lists.settings_page(session)),
        dialogs.DynamicMenuItem(title="Backups, on this device only", shortcut="b", action=lambda: backups(session)),
        dialogs.DynamicMenuItem(title="Export and import", shortcut="e", action=lambda: export_import(session)),
        dialogs.DynamicMenuItem(title=lambda *_: read_back_title(), shortcut="r", action=toggle_read_back),
    ]
    with screen("lumenna-settings"), opened(session):
        dialogs.dynamic_menu(menu, title="Settings")
    return ""


def shown_settings(session: Session) -> list:
    """Every setting as the app shows it, once the host has said."""
    view = session.view().settings
    return view.settings if view is not None else []


def setting_items(session: Session, which) -> list:
    """The settings `which` keeps, one row each, said as the core names them."""
    made = []
    for shown in shown_settings(session):
        setting = shown.setting
        if not which(setting):
            continue
        named = {option.id: option.title for option in setting.options}
        made.append(dialogs.DynamicMenuItem(
            title=f"{setting.title}, {named.get(setting.value, setting.value)}",
            action=lambda key=setting.key: change_setting(session, key),
        ))
    return made


def setting_page(session: Session, title: str, which) -> str:
    """Settings, one per row: Enter changes one."""
    with screen("lumenna-settings"), opened(session):
        live_menu(
            session, lambda: setting_items(session, which), title,
            app=undo_commands(session), app_title=f"{title} menu",
        )
    return ""


#: What a line of text takes, for a setting typed rather than chosen, by its kind.
TAKES = {L.SettingKind.TIME: "a time such as 9am or 14:30", L.SettingKind.NUMBER: "a whole number"}


def setting_of(session: Session, key: str):
    return next((shown for shown in shown_settings(session) if shown.setting.key == key), None)


def change_setting(session: Session, key: str) -> str:
    """A choice where the setting has options; else a line of text, which the core reads and
    checks. The device has a date dialog but no time one, so a time is typed as said."""
    shown = setting_of(session, key)
    if shown is None:
        return ""
    setting = shown.setting
    value = setting.value
    offered = {option.id: option.title for option in setting.options}
    said = ". ".join(part for part in (setting.title, setting.hint) if part)
    if offered:
        chosen = choose(offered, said, default=value)
        if chosen is None or chosen == value:
            return ""
        return session.send(L.ShellEventSettings(L.SettingsEventSet(key, chosen)))
    takes = TAKES.get(setting.kind)
    asked = f"{said}, {takes}" if takes else said
    # Saved when entered. One the core refuses is asked again, its sentence first and the
    # line as typed, as a refused field stays in its dialog.
    wrong, typed = "", value
    while True:
        chosen = dialogs.request_input(f"{wrong}. {asked}" if wrong else asked, default_text=typed)
        if chosen is None or chosen == value:
            return ""
        made = session.send(L.ShellEventSettings(L.SettingsEventSet(key, chosen)))
        shown = setting_of(session, key)
        if shown is not None and shown.problem:
            wrong, typed = shown.problem, chosen
            continue
        return made


# ---------------------------------------------------------------------------------------
# Devices, sync and pairing
# ---------------------------------------------------------------------------------------


def device_rows(session: Session) -> list[Row]:
    """The paired devices, each said as the core lines it: what it is, and how syncing with
    it last went. Words rather than a symbol, which a screen reader may not say."""
    view, lines = session.view().devices, session.lines().devices
    listed = view.list.devices if view is not None and view.list is not None else []
    made = []
    for index, device in enumerate(listed):
        line = lines[index].text if index < len(lines) else device.name
        row = Row(device.node_id, line, device.actions, device, title=device.name)
        row.side = L.Side.DEVICES
        made.append(row)
    return made


def devices(session: Session) -> str:
    """How syncing is going, device by device, and pairing another."""
    state = {"heading": "Devices and sync"}
    session.send(L.ShellEventDevices(L.DevicesEventShow()))
    # How syncing is going comes from the network, which answers later than the store: waited
    # for a moment, so the heading the page opens with is not replaced as it is read.
    deadline = time.monotonic() + STATUS_FOR
    while time.monotonic() < deadline:
        view = session.view().devices
        if view is not None and view.status:
            break
        time.sleep(0.05)

    def build():
        view = session.view().devices
        rows = device_rows(session)
        state["heading"] = view.status if view is not None and view.status else "Devices and sync"
        return items(rows)

    with screen("lumenna-settings"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: actions.run_kind(session, row, actions.K.RENAME),
            context=lambda rows: actions.commands(session, rows),
            app=[
                Command("Sync now", lambda _: sync_now(session), key="s"),
                Command("Pair a device", lambda _: pair(session), key="p"),
            ],
            # The status heading already says nothing is paired; the empty list adds only the key.
            empty="Press p to pair one.",
            app_title="Devices menu",
            focus=lambda: session.view().devices_held.focus,
        )
    return ""


def sync_now(session: Session) -> str:
    """One round with every paired device, through whatever on this device holds the sync
    endpoint (the daemon, as it should be here); the app says how it went once it has."""
    session.send(L.ShellEventDevices(L.DevicesEventSyncNow()))
    with dialogs.activity("Syncing"):
        return session.wait_said(SYNC_FOR)


def pairing_words(session: Session):
    """Pairing's sentences and buttons, the core's: on this network the device finds the
    other by itself, through Avahi."""
    if session.view().devices is None:
        # Pairing's words come with the Devices page, wherever pairing is asked for.
        session.send(L.ShellEventDevices(L.DevicesEventShow()))
    return session.view().devices.words


def ask_code(session: Session) -> str | None:
    """The other device's code, as typed or pasted into the field. Nothing is taken from the
    clipboard unasked, as in every other client. None if cancelled; empty if none was given."""
    words = pairing_words(session)
    text = dialogs.request_input(words.their_code, default_text="")
    if text is None:
        return None
    return text.strip().replace(" ", "")


def pairing_view(session: Session):
    view = session.view().devices
    return view.pairing if view is not None else None


def pair(session: Session) -> str:
    """Pairs with another of the person's devices, comparing three words on both.

    The app runs the pairing and shows each step as it comes: the code to give the other device
    while this one waits, then the words, answered here. The menu stays responsive the whole
    time.
    """
    words = pairing_words(session)
    how = choose({"wait": words.wait, "code": words.join}, f"{words.title}. {words.intro_sentence}")
    if how is None:
        return ""
    code = None
    if how == "code":
        code = ask_code(session)
        if code is None:
            return ""
        if not code:
            return words.need_code
    if session.view().devices is None:
        # Pairing is the Devices page's, wherever it is asked for.
        session.send(L.ShellEventDevices(L.DevicesEventShow()))
    session.send(L.ShellEventDevices(L.DevicesEventPair(L.PairEventOpen("this device"))))
    if code:
        session.send(
            L.ShellEventDevices(L.DevicesEventPair(L.PairEventCode(code))),
            L.ShellEventDevices(L.DevicesEventPair(L.PairEventJoin())),
        )
    else:
        session.send(L.ShellEventDevices(L.DevicesEventPair(L.PairEventWait())))
    return pairing_screen(session)


def pairing_screen(session: Session) -> str:
    """The pairing the app has under way, until it ends: what is said at its end."""
    words = pairing_words(session)

    def send(event) -> str:
        return session.send(L.ShellEventDevices(L.DevicesEventPair(event)))

    def status() -> str:
        pairing = pairing_view(session)
        if pairing is None:
            return ""
        if pairing.problem:
            return pairing.problem
        if pairing.mine:
            # Enter on the line shows the code alone, to read a character at a time.
            return f"{words.waiting} {words.my_code}: {pairing.mine}"
        return words.opening if pairing.waiting else words.connecting

    def read_code() -> str:
        pairing = pairing_view(session)
        if pairing is not None and pairing.mine:
            dialogs.view_lines([pairing.mine])
        return ""

    def copy_code() -> str:
        # Only when asked: the Blazie clipboard is the desktop's too, and the person's own.
        pairing = pairing_view(session)
        if pairing is not None and pairing.mine:
            clipboard.copy(pairing.mine, False)
            dialogs.show_message(words.copied)
        return ""

    def code_instead(menu) -> None:
        code = ask_code(session)
        if code:
            send(L.PairEventCode(code))
            send(L.PairEventJoin())
            menu.close()
        elif code is not None:
            dialogs.show_message(words.need_code)

    left = {"now": False}

    def cancel(menu) -> None:
        left["now"] = True
        menu.close()

    said = ""
    while True:
        pairing = pairing_view(session)
        if pairing is None:
            # Paired: what the app said of it is what is said after the screen.
            return said or session.wait_said(0)
        if pairing.words:
            matched = dialogs.request_confirmation(
                f"{words.match_title}. {words.match_message} {', '.join(pairing.words)}", default=False,
            )
            send(L.PairEventConfirm(bool(matched)))
            continue
        if not pairing.running and not pairing.problem:
            # Nothing under way and nothing went wrong: the screen was let go of.
            send(L.PairEventCancel())
            return ""
        session.settle()
        menu = [
            dialogs.DynamicMenuItem(title=lambda *_: status(), action=read_code),
            *([dialogs.DynamicMenuItem(title=words.copy_code, action=copy_code)] if pairing.mine else []),
            *([dialogs.DynamicMenuItem(title=words.join, action=code_instead)] if pairing.waiting and pairing.running else []),
            dialogs.DynamicMenuItem(title=session.sentence("Cancel"), action=cancel),
        ]
        choice = dialogs.dynamic_menu(menu, title=words.title, exit_condition=session.restless, refresh_interval=REFRESH)
        if session.restless():
            continue
        if left["now"] or choice is None:
            # Given up: the pairing under way first, then, once it has stopped, the screen.
            if pairing_view(session) is not None and pairing_view(session).running:
                send(L.PairEventCancel())
                deadline = time.monotonic() + 60
                while pairing_view(session) is not None and pairing_view(session).running and time.monotonic() < deadline:
                    session.wait_said(1)
            if pairing_view(session) is not None:
                send(L.PairEventCancel())
            session.settle()
            return ""
        if pairing_view(session) is not None and not pairing_view(session).running:
            # It ended in a problem, said on the status line; Cancel or leaving puts it away.
            continue


# ---------------------------------------------------------------------------------------
# Backups, export and import
# ---------------------------------------------------------------------------------------


def backups(session: Session) -> str:
    """This device's backups: how often, how many, where — and one now, or one merged in."""

    with screen("lumenna-settings"), opened(session):
        live_menu(
            session,
            lambda: setting_items(session, on_this_device),
            "Backups. A backup holds your whole history, including every task you deleted, so the "
            "store can be rebuilt from it. It stays on this device",
            app=[
                Command("Back up now", lambda _: back_up(session), key="b"),
                Command("Restore from a backup", lambda _: restore(session), key="r"),
            ],
            app_title="Backups menu",
        )
    return ""


def back_up(session: Session) -> str:
    """A backup now, beside the automatic ones."""
    try:
        return session.device.backup().announcement
    except Exception as error:  # noqa: BLE001 - the core's refusal
        return refused(error)


def setting_value(session: Session, key: str) -> str:
    shown = setting_of(session, key)
    return shown.setting.value if shown is not None else ""


def restore(session: Session) -> str:
    """Merges a backup in. Nothing in the store is lost; a task deleted since stays deleted."""
    start = setting_value(session, "backup-dir") or str(Path.home())
    if not Path(start).is_dir():
        start = str(Path.home())
    chosen = dialogs.request_file(start, ["lumbak"], prompt="Restore which backup?")
    if chosen is None:
        return ""
    return import_path(session, Path(chosen))


def export_import(session: Session) -> str:
    with opened(session):
        return _export_import(session)


def _export_import(session: Session) -> str:
    menu = [
        dialogs.DynamicMenuItem(title="Export", action=lambda: export(session)),
        dialogs.DynamicMenuItem(title="Import an export, or restore a backup", action=lambda: import_file(session)),
    ]
    dialogs.dynamic_menu(
        menu,
        title="Export and import. An export is what you have now, with nothing from the trash. "
        "Importing adds what this device lacks and removes nothing",
    )
    return ""


def export(session: Session) -> str:
    """Writes the current state to a file in a folder of the person's choosing, named as the
    app names it."""
    with opened(session):
        return _export(session)


def _export(session: Session) -> str:
    view = session.view().settings
    offered = view.exports if view is not None else []
    if not offered:
        return ""
    index = choose({i: choice.label for i, choice in enumerate(offered)}, "Export as")
    if index is None:
        return ""
    picked = offered[index]
    folder = dialogs.request_directory(str(Path.home()), prompt="Save it in which folder?")
    if folder is None:
        return ""
    path = Path(folder) / picked.name
    if path.exists() and not confirm(f"{picked.name} is already there. Replace it?"):
        return ""
    session.files.clear()
    said = session.send(L.ShellEventSettings(L.SettingsEventExport(picked.format)))
    if not session.files:
        return said
    made = session.files.pop()
    try:
        path.write_bytes(bytes(made.bytes))
    except OSError as error:
        return f"{made.name} could not be saved: {error.strerror or error}"
    return said or f"Saved {path}"


def import_file(session: Session) -> str:
    """Reads a JSON export or a backup; the core tells them apart."""
    chosen = dialogs.request_file(str(Path.home()), ["json", "lumbak"], prompt="Import which file?")
    if chosen is None:
        return ""
    return import_path(session, Path(chosen))


def import_path(session: Session, path: Path) -> str:
    """Reads `path` into the store: the app tells an export from a backup by its name."""
    try:
        contents = path.read_bytes()
    except OSError as error:
        return f"{path.name} could not be read: {error.strerror or error}"
    with opened(session):
        return session.send(L.ShellEventSettings(L.SettingsEventImport(path.name, contents)))
