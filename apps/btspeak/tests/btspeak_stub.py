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
        self.choices = choices or []


class ScriptError(AssertionError):
    pass


class Script:
    """What the person does, step by step, and everything the app said back."""

    def __init__(self, steps):
        self.steps = list(steps)
        self.said: list[str] = []
        self.titles: list[str] = []
        self.shown: list = []

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
    def __init__(self):
        self.closed = False

    def close(self):
        self.closed = True


def dynamic_menu(menu, title=None, exit_condition=None, refresh_interval=0, default=0, **_ignored):
    """A menu: each `("menu", text)` step runs the visible row whose title contains `text`;
    `("menu", text, "delete")` presses delete on it; `("back",)` leaves; `("wait",)` stays until
    the menu's exit condition fires, as the real one would on its refresh."""
    script.titles.append(title() if callable(title) else (title or ""))
    selection = default
    handle = _Menu()
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
        text, *verb = script.take("menu", title)
        visible = [i for i, it in enumerate(menu) if it.dependency is None or it.dependency()]
        found = [i for i in visible if text in menu[i].get_title()]
        if not found:
            rows = [menu[i].get_title() for i in visible]
            raise ScriptError(f"no row says {text!r} in {title!r}: {rows}")
        selection = found[0]
        item = menu[selection]
        fn = getattr(item, verb[0] if verb else "action")
        if fn is None:
            raise ScriptError(f"{text!r} has no {verb or 'action'}")
        said = _call(fn, handle, item)
        if isinstance(said, str) and said:
            script.said.append(said)
        if handle.closed:
            return Choice(selection, item.get_title())


def request_choice(choices, prompt="", default=None, **_ignored):
    """`("choose", text)` picks the choice whose label contains `text`; `("choose", None)`
    cancels."""
    options = dict(choices) if isinstance(choices, dict) else {c: c for c in choices}
    (text,) = script.take("choose", f"{prompt}: {list(options.values())}")
    if text is None:
        return None
    for key, label in options.items():
        if text in label:
            return Choice(key, label)
    raise ScriptError(f"no choice says {text!r} for {prompt!r}: {list(options.values())}")


def request_input(prompt, default_text="", **_ignored):
    """`("input", text)` types `text`; None cancels; `...` accepts what was offered."""
    (text,) = script.take("input", prompt)
    return default_text if text is Ellipsis else text


def request_form(fields, **_ignored):
    """`("form", {key: value})` fills those fields and leaves the rest as offered; None
    cancels the form."""
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


def install() -> None:
    try:
        import BTSpeak  # noqa: F401
        return
    except ImportError:
        pass
    dialogs = types.ModuleType("BTSpeak.dialogs")
    for name, value in list(globals().items()):
        if name in {"Choice", "DynamicMenuItem", "InputField", "dynamic_menu", "request_choice",
                    "request_input", "request_form", "request_confirmation", "show_message",
                    "view_lines", "request_file", "request_directory", "activity"}:
            setattr(dialogs, name, value)
    package = types.ModuleType("BTSpeak")
    package.dialogs = dialogs
    sys.modules["BTSpeak"] = package
    sys.modules["BTSpeak.dialogs"] = dialogs


def play(steps) -> Script:
    """Starts a script; the dialogs play it from now on."""
    global script
    script = Script(steps)
    return script
