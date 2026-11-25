"""Settings, devices and sync, and getting data in and out.

Laid out in the core's pages, as every app lays them out (`SettingsView.pages`): what each holds,
its caption, its buttons and its letter are the core's. Each setting is offered the way its value is
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
import forms
from actions import L
import core
from core import device_name, refused
import options
from rows import Row, items
from session import REFRESH, Command, Session, choose, command_title, confirm, live_menu, screen, sentences
from tasks import undo_commands


#: How long a sync round may take before it is given up on: a device that cannot be reached
#: takes a while to give up on.
SYNC_FOR = 180

#: How long the Devices page waits for the network to say how syncing is going, in seconds.
STATUS_FOR = 5


def read_back_title() -> str:
    on = options.get(options.READ_BACK, False)
    words = core.words().read_back
    return words.title_on if on else words.title_off


def toggle_read_back() -> str:
    on = not options.get(options.READ_BACK, False)
    options.put(options.READ_BACK, on)
    words = core.words().read_back
    return words.turned_on if on else words.turned_off


@contextlib.contextmanager
def opened(session: Session):
    """The app's settings, open while a page is shown: opened here unless they are open already,
    and put away here only if opened here."""
    if session.view().settings is not None:
        yield
        return
    session.send(L.ShellEventSettings(L.SettingsEventOpen()))
    try:
        yield
    finally:
        if session.view().settings is not None:
            session.send(L.ShellEventSettings(L.SettingsEventClose()))


def pages(session: Session) -> list:
    """The core's settings pages, in order, once the app has opened settings."""
    view = session.view().settings
    return list(view.pages) if view is not None else []


def page_of(session: Session, page_id: str):
    return next((page for page in pages(session) if page.id == page_id), None)


def settings(session: Session) -> str:
    """The core's settings pages, as every app groups them, each with its letter here and its
    description as its hint, then this app's own preference. Which pages this device lists is
    the core's (`SettingsView::pages`): every one is listed. The app may have opened settings
    itself (Settings on Home's menu, `Key("settings")`); either way they are put away on leaving."""
    session.settings_open = True
    try:
        with screen("lumenna-settings"), opened(session):
            menu = [
                dialogs.DynamicMenuItem(
                    title=page.title, shortcut=L.page_letter(page.id) or "", hint=page.description or None,
                    action=lambda page_id=page.id: open_page(session, page_id),
                )
                for page in pages(session)
            ]
            menu.append(dialogs.DynamicMenuItem(title=lambda *_: read_back_title(), shortcut=L.command_letter("read-back"), action=toggle_read_back))
            dialogs.dynamic_menu(menu, title=command_title("settings"))
            if session.view().settings is not None:
                session.send(L.ShellEventSettings(L.SettingsEventClose()))
    finally:
        session.settings_open = False
    return ""


def open_page(session: Session, page_id: str) -> str:
    """One of the core's pages: Devices and sync (the page the core marks `devices`) is a screen
    of its own, its devices a list; the rest are the app's description of the page
    (`App::settings_page_on`), a row per control."""
    page = page_of(session, page_id)
    if page is not None and page.devices:
        return devices(session)
    return setting_page(session, page_id)


def page_from_menu(session: Session, command: str) -> str:
    """The settings page a command of the menu bar opens (`settings_page_of`: Devices, Export and
    import), settings open while it is shown and put away after if opened for it."""
    page_id = L.settings_page_of(command)
    if page_id is None:
        return ""
    with opened(session):
        return open_page(session, page_id)


def page_dialog(session: Session, page_id: str):
    """Settings page `page_id` as the app describes it, its controls in tab order; None while
    settings are not open, or for a page this device does not list."""
    session.view()
    return session.app.settings_page_on(0, page_id)


def control_of(session: Session, page_id: str, key: str):
    dialog = page_dialog(session, page_id)
    return next((control for control in (dialog.controls if dialog is not None else []) if control.key == key), None)


def check_words(session: Session, control) -> dict:
    """What a setting that is on or off calls each state, `true` and `false`: the setting's own
    options ("On", "Off"), which its check does not carry."""
    key = control.sends.field_0 if isinstance(control.sends, L.ControlSendsSetting) else ""
    view = session.view().settings
    shown = next((shown for shown in (view.settings if view is not None else []) if shown.setting.key == key), None)
    named = {option.id: option.title for option in (shown.setting.options if shown is not None else [])}
    if {"true", "false"} <= set(named):
        return {"true": named["true"], "false": named["false"]}
    yes_no = {choice.id: choice.title for choice in core.words().yes_no}
    return {"true": yes_no.get("yes", "true"), "false": yes_no.get("no", "false")}


def control_title(session: Session, control) -> str:
    """A setting's row: its name and what it holds, as the core names them."""
    value = check_words(session, control)[forms.shown_value(control)] if control.kind == forms.T.CHECK else control.value
    return f"{control.label}, {value}" if value else control.label


def setting_items(session: Session, page_id: str) -> list:
    """The page's controls, in its order, one row each: a setting said as the core names it, a
    button with its letter, a line to read (the store's actions' caption) as the hint of the rows
    it is about. Enter changes a setting or presses a button."""
    dialog = page_dialog(session, page_id)
    made = []
    for control in (dialog.controls if dialog is not None else []):
        if control.kind == forms.T.TEXT:
            if control.key == "offered-caption":
                # The caption of the store's own actions, said after each as the device says a hint.
                for item in made:
                    if getattr(item, "store_action", False):
                        item.hint = control.value
            continue
        if is_folder(control):
            # A folder setting, chosen with the device's folder dialog.
            title, action = control_title(session, control), (lambda key=control.key: change_setting(session, page_id, key))
        elif control.kind == forms.T.BUTTON:
            title, action = control.label, (lambda key=control.key: press(session, page_id, key))
        else:
            title, action = control_title(session, control), (lambda key=control.key: change_setting(session, page_id, key))
        item = dialogs.DynamicMenuItem(title=title, action=action, shortcut=control.letter or None)
        item.store_action = control.key.startswith("action:")
        made.append(item)
    return made


def is_folder(control) -> bool:
    """Whether `control` is a folder setting, which the app leaves to this app to choose."""
    return control.kind == forms.T.BUTTON and isinstance(control.sends, L.ControlSendsShell) and control.key.startswith("setting:")


def page_title(page) -> str:
    """A page's heading: its title, then what it says under its settings."""
    return sentences(page.title, page.caption) if page is not None else ""


def button_commands(session: Session, page_id: str) -> list[Command]:
    """A page's buttons with letters, the core's, as its main menu's commands too."""
    dialog = page_dialog(session, page_id)
    return [
        Command(control.label, lambda _, key=control.key: press(session, page_id, key), key=control.letter)
        for control in (dialog.controls if dialog is not None else [])
        if control.kind == forms.T.BUTTON and control.letter
    ]


def setting_page(session: Session, page_id: str) -> str:
    """One of the core's pages, one row per control: Enter changes a setting or presses a
    button; its lettered buttons are on its main menu too."""
    with screen("lumenna-settings"), opened(session):
        page = page_of(session, page_id)
        if page is None:
            return ""
        live_menu(
            session, lambda: setting_items(session, page_id), lambda: page_title(page_of(session, page_id)),
            app=lambda: [*button_commands(session, page_id), *undo_commands(session)], app_title=L.menu_of(page.title),
        )
    return ""


def press(session: Session, page_id: str, key: str) -> str:
    """A page's button pressed: what the app says it sends, sent, and whatever that asks
    answered; or what it leaves to this app (`ControlSends::Shell`): a file to pick, a backup to
    take, a device to pair. Sync now waits for its answer; an export asks where to save."""
    control = control_of(session, page_id, key)
    if control is None:
        return ""
    if isinstance(control.sends, L.ControlSendsShell):
        runs = {"pair": pair, "back-up": back_up, "restore-backup": restore, "import": import_file}
        run = runs.get(control.sends.field_0)
        return run(session) if run is not None else ""
    if key == "sync-now":
        return sync_now(session)
    if key.startswith("export:"):
        return export(session, control)
    return actions.joined(forms.send(session, control), actions.follow(session))


def change_setting(session: Session, page_id: str, key: str) -> str:
    """A setting changed as its control is shaped: a choice from its options, a yes or no, a
    folder from the device's folder dialog, else a line of text, which the core reads and
    checks: one it refuses is asked again, its sentence first and the line as typed, as a
    refused field stays in its dialog. The device has a date dialog but no time one, so a time
    is typed as said."""
    control = control_of(session, page_id, key)
    if control is None:
        return ""
    asked = actions.prompt(control.label, control.hint)
    if control.kind == forms.T.CHOICE:
        chosen = choose({option.id: option.title for option in control.options}, asked, default=forms.selected_id(control))
        if chosen is None or chosen == forms.selected_id(control):
            return ""
        return forms.send(session, control, chosen, L.ControlMoveSet())
    if control.kind == forms.T.CHECK:
        now = forms.shown_value(control)
        chosen = choose(check_words(session, control), asked, default=now)
        if chosen is None or chosen == now:
            return ""
        return forms.send(session, control, chosen, L.ControlMoveSet())
    if is_folder(control):
        start = control.value if control.value and Path(control.value).is_dir() else str(Path.home())
        folder = dialogs.request_directory(start, prompt=control.label)
        if folder is None:
            return ""
        # The folder is the setting's value, set as any setting's is.
        events = L.control_event(L.ControlSendsSetting(control.sends.field_0), folder, L.ControlMoveSet())
        return session.send(*events)
    wrong, typed = "", control.value
    while True:
        chosen = dialogs.request_input(actions.prompt(wrong, asked), default_text=typed)
        if chosen is None or (not wrong and chosen == control.value):
            return ""
        made = forms.send(session, control, chosen, L.ControlMoveSet())
        problem = next(
            (shown.value for shown in (page_dialog(session, page_id).controls if page_dialog(session, page_id) else [])
             if shown.key == f"problem:{key.removeprefix('setting:')}"),
            "",
        )
        if problem:
            wrong, typed = problem, chosen
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


def devices_page(session: Session):
    """The core's Devices and sync page (the one it marks `devices`), whether or not settings
    are open."""
    return next((page for page in pages(session) if page.devices), None) or next(
        page for page in L.settings_pages(False, True) if page.devices
    )


def devices(session: Session) -> str:
    """How syncing is going, device by device, and pairing another."""
    page = devices_page(session)
    state = {"heading": page.title}
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
        state["heading"] = view.status if view is not None and view.status else page.title
        return items(rows)

    with screen("lumenna-settings"):
        live_menu(
            session, build, lambda: state["heading"],
            # Enter renames a device, as Enter does on every platform (`Key("open")`).
            main=lambda row: actions.press(session, "open", row),
            context=lambda rows: actions.commands(session, rows),
            app=lambda: button_commands(session, page.id),
            # The status heading already says nothing is paired; the empty list adds only the key.
            empty=L.press_to_pair(L.button_letter("pair")),
            app_title=L.menu_of(page.title),
            focus=lambda: session.view().devices_held.focus,
        )
    return ""


def sync_now(session: Session) -> str:
    """One round with every paired device, through whatever on this device holds the sync
    endpoint (the background service, once it is on), as the app runs Sync now
    (`Key("sync-now")`); the app says how it went once it has, waited for here."""
    session.send(L.ShellEventKey("sync-now"))
    with dialogs.activity(core.words().syncing):
        return session.wait_said(SYNC_FOR)


def pairing_words(session: Session):
    """Pairing's sentences and buttons, the core's: on this network the device finds the
    other by itself, through Avahi."""
    if session.view().devices is None:
        # Pairing's words come with the Devices page, wherever pairing is asked for.
        session.send(L.ShellEventDevices(L.DevicesEventShow()))
    return session.view().devices.words


def ask_code(session: Session) -> str | None:
    """The other device's code, as typed or pasted into the field, spaces between its groups and
    all: the app takes it as typed. Nothing is taken from the clipboard unasked, as in every other
    client. None if cancelled; empty if none was given."""
    words = pairing_words(session)
    text = dialogs.request_input(words.their_code, default_text="")
    if text is None:
        return None
    return text.strip()


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
    how = choose({"wait": words.wait, "code": words.join}, words.heading)
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
        # Where pairing stands, the core's one line: what went wrong, else waiting, opening,
        # connecting.
        pairing = pairing_view(session)
        return pairing.status if pairing is not None else ""

    def code_line() -> str:
        # The code to give the other device, a row of its own under the status while waiting.
        pairing = pairing_view(session)
        return (pairing.mine_line or pairing.mine or "") if pairing is not None else ""

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
            # The question about the words, the core's one line: its title, then the words.
            matched = dialogs.request_confirmation(pairing.question or "", default=False)
            send(L.PairEventConfirm(bool(matched)))
            continue
        if not pairing.running and not pairing.problem:
            # Nothing under way and nothing went wrong: the screen was let go of.
            send(L.PairEventCancel())
            return ""
        session.settle()
        menu = [
            dialogs.DynamicMenuItem(title=lambda *_: status(), action=read_code),
            # Enter on the code shows it alone, to read a character at a time.
            *([dialogs.DynamicMenuItem(title=lambda *_: code_line(), action=read_code)] if pairing.mine else []),
            *([dialogs.DynamicMenuItem(title=words.copy_code, action=copy_code)] if pairing.mine else []),
            *([dialogs.DynamicMenuItem(title=words.join, action=code_instead)] if pairing.waiting and pairing.running else []),
            dialogs.DynamicMenuItem(title=core.words().cancel, action=cancel),
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
    view = session.view().settings
    shown = next((shown for shown in (view.settings if view is not None else []) if shown.setting.key == key), None)
    return shown.setting.value if shown is not None else ""


def restore(session: Session) -> str:
    """Merges a backup in. Nothing in the store is lost; a task deleted since stays deleted."""
    start = setting_value(session, "backup-dir") or str(Path.home())
    if not Path(start).is_dir():
        start = str(Path.home())
    chosen = dialogs.request_file(start, ["lumbak"], prompt=core.words().files.restore)
    if chosen is None:
        return ""
    return import_path(session, Path(chosen))


def export(session: Session, control) -> str:
    """Writes the current state to a file in a folder of the person's choosing, named as the app
    names it: one of the Export page's exports (`control`), as the app describes it."""
    view = session.view().settings
    picked = next((made for made in (view.exports if view is not None else []) if made.label == control.label), None)
    if picked is None:
        return core.words().nothing_to_choose
    folder = dialogs.request_directory(str(Path.home()), prompt=core.words().files.save_in)
    if folder is None:
        return ""
    path = Path(folder) / picked.name
    if path.exists() and not confirm(L.already_there(picked.name)):
        return ""
    session.files.clear()
    said = forms.send(session, control)
    if not session.files:
        return said
    made = session.files.pop()
    try:
        path.write_bytes(bytes(made.bytes))
    except OSError as error:
        return L.could_not_save(made.name, str(error.strerror or error))
    return said or L.saved(str(path))


def import_file(session: Session) -> str:
    """Reads a JSON export or a backup; the core tells them apart."""
    chosen = dialogs.request_file(str(Path.home()), ["json", "lumbak"], prompt=core.words().files.import_)
    if chosen is None:
        return ""
    return import_path(session, Path(chosen))


def import_path(session: Session, path: Path) -> str:
    """Reads `path` into the store: the app tells an export from a backup by its name."""
    try:
        contents = path.read_bytes()
    except OSError as error:
        return L.could_not_read(path.name, str(error.strerror or error))
    with opened(session):
        return session.send(L.ShellEventSettings(L.SettingsEventImport(path.name, contents)))
