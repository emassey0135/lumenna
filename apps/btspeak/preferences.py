"""Settings, devices and sync, and getting data in and out.

Laid out in the core's pages, as every app lays them out (`SettingsView.pages`): what each holds,
its caption and its buttons are the core's, its letter this app's. Each setting is offered the way its value is
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
from core import device_name, refused
import options
from rows import Row, items
from session import REFRESH, Command, Session, choose, confirm, live_menu, screen, sentences
from tasks import undo_commands


#: This app's letter for each of the core's settings pages (`SettingsPage.id`), as it has
#: always had them.
PAGE_KEYS = {"general": "g", "planning": "p", "reminders": "m", "devices": "d", "backups": "b", "export": "e"}

#: This app's letter for each of a page's buttons (`SettingsButton.id`), on the page's main menu.
BUTTON_KEYS = {"sync-now": "s", "pair": "p", "back-up": "b", "restore-backup": "r", "import": "i"}

#: How long a sync round may take before it is given up on: a device that cannot be reached
#: takes a while to give up on.
SYNC_FOR = 180

#: How long the Devices page waits for the network to say how syncing is going, in seconds.
STATUS_FOR = 5


def read_back_title() -> str:
    on = options.get(options.READ_BACK, False)
    words = L.read_back_words(True)
    return words.title_on if on else words.title_off


def toggle_read_back() -> str:
    on = not options.get(options.READ_BACK, False)
    options.put(options.READ_BACK, on)
    words = L.read_back_words(True)
    return words.turned_on if on else words.turned_off


def setting_title(setting) -> str:
    """A setting's row: its name and its value, as the core names them."""
    named = {option.id: option.title for option in setting.options}
    return f"{setting.title}, {named.get(setting.value, setting.value)}"


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


def pages(session: Session) -> list:
    """The core's settings pages, in order, once the app has opened settings."""
    view = session.view().settings
    return list(view.pages) if view is not None else []


def page_of(session: Session, page_id: str):
    return next((page for page in pages(session) if page.id == page_id), None)


def settings(session: Session) -> str:
    """The core's settings pages, as every app groups them, each with its letter here and its
    description as its hint, then this app's own preference. A page with nothing on it here —
    no setting the app shows, no button, no screen of its own — is left out."""
    with screen("lumenna-settings"), opened(session):
        shown = {setting.setting.key for setting in shown_settings(session)}
        menu = [
            dialogs.DynamicMenuItem(
                title=page.title, shortcut=PAGE_KEYS.get(page.id, ""), hint=page.description or None,
                action=lambda page_id=page.id: open_page(session, page_id),
            )
            for page in pages(session)
            if page.buttons or page.id == "export" or any(key in shown for key in page.keys)
        ]
        menu.append(dialogs.DynamicMenuItem(title=lambda *_: read_back_title(), shortcut="r", action=toggle_read_back))
        dialogs.dynamic_menu(menu, title=command_title("settings"))
    return ""


def command_title(command: str) -> str:
    """A command of the shared table's name, as the core's menus give it: "Settings"."""
    return next((item.title for group in L.menus(True) for item in group.items if item.id == command), command)


def open_page(session: Session, page_id: str) -> str:
    """One of the core's pages: Devices and sync, and Export and import, are screens of their
    own; the rest list their settings."""
    if page_id == "devices":
        return devices(session)
    if page_id == "export":
        return export_import(session)
    return setting_page(session, page_id)


def button_commands(session: Session, page) -> list[Command]:
    """A page's buttons, the core's, as its main menu's commands, each with this app's letter."""
    runs = {"sync-now": sync_now, "pair": pair, "back-up": back_up, "restore-backup": restore, "import": import_file}
    return [
        Command(button.title, lambda _, run=runs[button.id]: run(session), key=BUTTON_KEYS.get(button.id, ""))
        for button in (page.buttons if page is not None else [])
        if button.id in runs
    ]


def store_items(session: Session) -> list:
    """What the app offers to do to the store as a whole (Erase everything deleted), a row
    each beside backups and export, under the core's name, its caption the row's hint. No shortcut: what it does cannot be
    undone, so it is reached by moving to it, and the app asks first."""
    view = session.view().settings
    return [
        dialogs.DynamicMenuItem(
            title=actions.named(offered),
            action=lambda offered=offered: actions.act(session, offered, on=L.ActingOnSettings()),
            # The core's caption for them, said and brailled as the device says a row's hint.
            hint=view.offered_caption or None,
        )
        for offered in (view.offered if view is not None else [])
    ]


def shown_settings(session: Session) -> list:
    """Every setting as the app shows it, once the host has said."""
    view = session.view().settings
    return view.settings if view is not None else []


def setting_items(session: Session, page_id: str) -> list:
    """The settings on the core's page `page_id`, in its order, one row each, said as the core
    names them."""
    page = page_of(session, page_id)
    made = []
    for key in (page.keys if page is not None else []):
        shown = setting_of(session, key)
        if shown is None:
            continue
        made.append(dialogs.DynamicMenuItem(
            title=setting_title(shown.setting),
            action=lambda key=key: change_setting(session, key),
        ))
    return made


def page_title(page) -> str:
    """A page's heading: its title, then what it says under its settings."""
    return sentences(page.title, page.caption) if page is not None else ""


def setting_page(session: Session, page_id: str) -> str:
    """One of the core's pages of settings, one per row: Enter changes one; its buttons are on
    its main menu."""
    with screen("lumenna-settings"), opened(session):
        page = page_of(session, page_id)
        if page is None:
            return ""
        live_menu(
            session, lambda: setting_items(session, page_id), lambda: page_title(page_of(session, page_id)),
            app=[*button_commands(session, page), *undo_commands(session)], app_title=L.menu_of(page.title),
        )
    return ""


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
    if offered:
        chosen = choose(offered, actions.prompt(setting.title, setting.hint), default=value)
        if chosen is None or chosen == value:
            return ""
        return session.send(L.ShellEventSettings(L.SettingsEventSet(key, chosen)))
    words = session.view().settings.words
    # What a line takes is the setting's own hint; where it has none, the core's for its shape.
    hint = setting.hint or {L.SettingKind.TIME: words.time_hint, L.SettingKind.NUMBER: words.number_hint}.get(setting.kind, "")
    asked = actions.prompt(setting.title, hint)
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
        row = Row(device.node_id, line, device.offered, device, title=device.name)
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
            main=lambda row: actions.press(session, "rename", row),
            context=lambda rows: actions.commands(session, rows),
            app=[
                Command("Sync now", lambda _: sync_now(session), key="s"),
                Command("Pair a device", lambda _: pair(session), key="p"),
            ],
            # The status heading already says nothing is paired; the empty list adds only the key.
            empty=L.press_to_pair("p"),
            app_title=L.menu_of("Devices"),
            focus=lambda: session.view().devices_held.focus,
        )
    return ""


def sync_now(session: Session) -> str:
    """One round with every paired device, through whatever on this device holds the sync
    endpoint (the background service, once it is on); the app says how it went once it has."""
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
    session.send(L.ShellEventDevices(L.DevicesEventPair(L.PairEventOpen(device_name()))))
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
            return sentences(words.waiting, pairing.mine_line or pairing.mine)
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
            matched = dialogs.request_confirmation(sentences(words.match_title, pairing.matching or ""), default=False)
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
    chosen = dialogs.request_file(start, ["lumbak"], prompt=L.file_words(True).restore)
    if chosen is None:
        return ""
    return import_path(session, Path(chosen))


def export_import(session: Session) -> str:
    with opened(session):
        return _export_import(session)


def _export_import(session: Session) -> str:
    """The core's Export and import page: Export (this app's row, choosing among the core's
    exports), the page's buttons, then what the app offers to do to the store as a whole."""
    page = page_of(session, "export")
    runs = {"import": import_file}
    menu = [
        dialogs.DynamicMenuItem(title=L.file_words(True).export, action=lambda: export(session)),
        *[
            dialogs.DynamicMenuItem(title=button.title, action=lambda run=runs[button.id]: run(session))
            for button in (page.buttons if page is not None else []) if button.id in runs
        ],
        *store_items(session),
    ]
    dialogs.dynamic_menu(menu, title=page_title(page))
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
        return L.nothing_to_choose()
    index = choose({i: choice.label for i, choice in enumerate(offered)}, "Export as")
    if index is None:
        return ""
    picked = offered[index]
    folder = dialogs.request_directory(str(Path.home()), prompt=L.file_words(True).save_in)
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
    chosen = dialogs.request_file(str(Path.home()), ["json", "lumbak"], prompt=L.file_words(True).import_)
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
