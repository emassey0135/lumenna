"""Stands in for the device's `BTSpeak` package when the tests run anywhere else.

The app imports `BTSpeak.dialogs` at module load, and that package exists only on a BTSpeak
or BTBraille device. Off the device this installs a `dialogs` that plays a **script**: each
dialog the app opens takes the next step and answers as a person would — choose the row that
says this, type that, say yes. A step of the wrong kind fails the test at once, naming what
the app asked for instead, so a test reads as the conversation a person would have.

On the device the real package is found first and this does nothing.
"""

from __future__ import annotations

import contextlib
import inspect
import sys
import time
import types
from typing import NamedTuple


class Choice(NamedTuple):
    key: object
    label: str


class MenuCommand:
    """One entry of a context menu or an application main menu, as the device has it."""

    def __init__(self, name, label, action, key="", dependency=None, hidden=False, shortcut="",
                 on_delete_key=False):
        self.name = name
        self.label = label
        self.action = action
        self.key = key
        self.dependency = dependency
        self.on_delete_key = on_delete_key

    def get_label(self, dialog) -> str:
        label = self.label(dialog) if callable(self.label) else self.label
        return f"{label}, {self.key}" if self.key else label

    def applies(self, dialog) -> bool:
        return self.dependency is None or self.dependency(dialog)


class DynamicMenuItem:
    """Records what a `DynamicMenuItem` was built with, with the device's defaults."""

    def __init__(self, title="", shortcut=None, action=None, delete=None, left=None, right=None,
                 hotkeys=None, dependency=None, idle=None, idle_timeout=4.0, hint=None,
                 navigation_title=None, braille_title=None):
        self.title = title
        self.shortcut = shortcut
        self.action = action
        self.delete = delete
        self.left = left
        self.right = right
        self.hotkeys = hotkeys or {}
        self.dependency = dependency
        self.hint = hint

    def get_title(self) -> str:
        return self.title() if callable(self.title) else self.title


class InputField:
    def __init__(self, key, prompt, field_type="text", required=False, default_text="",
                 validate=None, format_hint="", choices=None, **_ignored):
        self.key = key
        self.prompt = prompt
        self.field_type = field_type
        self.required = required
        self.default_text = default_text
        self.validate = validate
        self.format_hint = format_hint
        self.choices = choices or []


class ScriptError(AssertionError):
    pass


class Script:
    """What the person does, step by step, and everything the app said back."""

    def __init__(self, steps):
        self.steps = list(steps)
        self.said: list[str] = []
        self.titles: list[str] = []
        self.menus: list[dict] = []
        self.shown: list = []
        self.offered: list[list[str]] = []
        self.prompts: list[str] = []
        self.forms: list[list] = []

    def take(self, kind: str, asked: str):
        if not self.steps:
            raise ScriptError(f"the script ran out; the app opened a {kind}: {asked}")
        step = self.steps.pop(0)
        if step[0] != kind:
            raise ScriptError(f"expected the app to open a {step[0]} for {step[1:]}, but it opened a {kind}: {asked}")
        return step[1:]

    def finished(self) -> bool:
        return not self.steps


script: Script | None = None


def _call(fn, menu=None, item=None):
    """Calls a menu function the way the device does, passing `menu` and `item` if asked."""
    names = inspect.signature(fn).parameters
    kwargs = {}
    if "menu" in names:
        kwargs["menu"] = menu
    if "item" in names:
        kwargs["item"] = item
    return fn(**kwargs)


class _Menu:
    """What a menu's commands are handed as `menu`: the rows and where the cursor is."""

    def __init__(self, menu):
        self.closed = False
        self.menu = menu
        self.selection = 0

    def close(self):
        self.closed = True


def dynamic_menu(menu, title=None, exit_condition=None, refresh_interval=0, default=0,
                 context_menu=(), app_menu=(), app_menu_title="", global_keys=None,
                 empty_message=None, **_ignored):
    """A menu, played from the script:

    - `("menu", text)` presses Enter on the visible row whose title contains `text`;
      `("menu", text, "delete")` presses the delete keys on it;
    - `("context", text, label)` opens that row's context menu (M-Chord with Dot 7) and
      chooses the entry whose label contains `label` — failing if it is not offered there;
    - `("app", label)` opens the main menu (M-Chord) and chooses from it;
    - `("key", text, letter)` presses a letter on that row;
    - `("back",)` leaves; `("wait",)` stays until the exit condition fires, as the real menu
      would on its refresh.
    """
    script.titles.append(title() if callable(title) else (title or ""))
    selection = default
    handle = _Menu(menu)
    script.menus.append({"context": context_menu, "app": app_menu, "keys": dict(global_keys or {}), "empty": empty_message})
    while True:
        if exit_condition and exit_condition():
            return Choice(selection, "")
        if not script.steps:
            raise ScriptError(f"the script ran out in the menu {title!r}")
        kind = script.steps[0][0]
        if kind == "back":
            script.steps.pop(0)
            return None
        if kind == "wait":
            script.steps.pop(0)
            deadline = time.monotonic() + 60
            while not (exit_condition and exit_condition()):
                if time.monotonic() > deadline:
                    raise ScriptError(f"the menu {title!r} never moved on")
                time.sleep(0.05)
            return Choice(selection, "")
        visible = [i for i, it in enumerate(menu) if it.dependency is None or it.dependency()]

        def row(text):
            found = [i for i in visible if text in menu[i].get_title()]
            if not found:
                rows = [menu[i].get_title() for i in visible]
                raise ScriptError(f"no row says {text!r} in {title!r}: {rows}")
            handle.selection = found[0]
            return menu[found[0]]

        def command(commands, label, where):
            offered = [c for c in commands if c.applies(handle)]
            for c in offered:
                if label in c.get_label(handle):
                    return c
            raise ScriptError(f"no {label!r} in the {where} of {title!r}: {[c.get_label(handle) for c in offered]}")

        if kind == "context":
            _, text, label = script.steps.pop(0)
            item = row(text)
            fn, = (command(context_menu, label, "context menu").action,)
        elif kind == "app":
            _, label = script.steps.pop(0)
            item = menu[handle.selection] if menu else None
            fn = command(app_menu, label, "main menu").action
        elif kind == "key":
            _, text, letter = script.steps.pop(0)
            item = row(text)
            if letter not in (global_keys or {}):
                raise ScriptError(f"{letter!r} does nothing in {title!r}: {sorted(global_keys or {})}")
            fn = global_keys[letter]
        else:
            text, *verb = script.take("menu", title)
            item = row(text)
            fn = getattr(item, verb[0] if verb else "action")
            if fn is None:
                raise ScriptError(f"{text!r} has no {verb or 'action'}")
        selection = handle.selection
        said = _call(fn, handle, item)
        if isinstance(said, str) and said:
            script.said.append(said)
        if handle.closed:
            return Choice(selection, item.get_title())


def request_choice(choices, prompt="", default=None, **_ignored):
    """`("choose", text)` picks the choice whose label contains `text`; `("choose", None)`
    cancels."""
    options = dict(choices) if isinstance(choices, dict) else {c: c for c in choices}
    script.offered.append(list(options.values()))
    script.prompts.append(prompt)
    (text,) = script.take("choose", f"{prompt}: {list(options.values())}")
    if text is None:
        return None
    for key, label in options.items():
        if text in label:
            return Choice(key, label)
    raise ScriptError(f"no choice says {text!r} for {prompt!r}: {list(options.values())}")


def request_input(prompt, default_text="", **_ignored):
    """`("input", text)` types `text`; None cancels; `...` accepts what was offered."""
    script.prompts.append(prompt)
    (text,) = script.take("input", prompt)
    return default_text if text is Ellipsis else text


def request_form(fields, **_ignored):
    """`("form", {key: value})` fills those fields and leaves the rest as offered; None
    cancels the form."""
    script.forms.append(list(fields))
    (answers,) = script.take("form", [f.key for f in fields])
    if answers is None:
        return None
    result = {}
    for f in fields:
        if f.field_type == "choice":
            keys = list(f.choices) if isinstance(f.choices, dict) else f.choices
            result[f.key] = f.default_text if f.default_text in keys else keys[0]
        else:
            result[f.key] = f.default_text
    unknown = set(answers) - set(result)
    if unknown:
        raise ScriptError(f"the form has no {unknown}; it has {list(result)}")
    result.update(answers)
    return result


def request_confirmation(question, default=True, **_ignored):
    (answer,) = script.take("confirm", question)
    return answer


def show_message(message, wait=True, **_ignored):
    script.said.append(message if isinstance(message, str) else " ".join(message))


def view_lines(lines, wrap=False, line=1):
    script.shown.append(list(lines) if not isinstance(lines, str) else [lines])


def request_file(start_dir, extensions=None, prompt="", **_ignored):
    (path,) = script.take("file", prompt)
    return path


def request_directory(start_dir, prompt="", **_ignored):
    (path,) = script.take("directory", prompt)
    return path


@contextlib.contextmanager
def activity(message=None, stdscr=None):
    yield


#: The device's settings, as `BTSpeak.settings.getValue` reads them; a test may change one.
device_settings = {"time-format": "24-hour"}

#: The device's clipboard, as one string.
_clipboard = [""]


def install() -> None:
    try:
        import BTSpeak  # noqa: F401
        return
    except ImportError:
        pass
    host = types.ModuleType("BTSpeak.host")
    host.push_app_context = lambda *a, **k: None
    host.pop_app_context = lambda *a, **k: None
    host.say = lambda *a, **k: None
    dialogs = types.ModuleType("BTSpeak.dialogs")
    for name, value in list(globals().items()):
        if name in {"Choice", "DynamicMenuItem", "MenuCommand", "InputField", "dynamic_menu", "request_choice",
                    "request_input", "request_form", "request_confirmation", "show_message",
                    "view_lines", "request_file", "request_directory", "activity"}:
            setattr(dialogs, name, value)
    clipboard = types.ModuleType("BTSpeak.clipboard")
    clipboard.copy = lambda text, isbraille, append=False: _clipboard.__setitem__(0, text) or "copied"
    clipboard.paste = lambda isbraille, size, multiline=True: ("pasted", _clipboard[0][:size])
    settings = types.ModuleType("BTSpeak.settings")
    settings.getValue = lambda setting: device_settings.get(setting, "")
    package = types.ModuleType("BTSpeak")
    package.dialogs = dialogs
    package.host = host
    package.clipboard = clipboard
    package.settings = settings
    sys.modules["BTSpeak"] = package
    sys.modules["BTSpeak.dialogs"] = dialogs
    sys.modules["BTSpeak.host"] = host
    sys.modules["BTSpeak.clipboard"] = clipboard
    sys.modules["BTSpeak.settings"] = settings


def play(steps) -> Script:
    """Starts a script; the dialogs play it from now on."""
    global script
    script = Script(steps)
    return script
