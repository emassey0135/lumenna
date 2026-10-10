"""Drives the GTK app as a screen reader user does, and checks what a screen reader is told.

Keys go in through the headless compositor's own RemoteDesktop interface, so they arrive as
real key presses; what the app exposes is read back through AT-SPI, as Orca reads it. With
ORCA=1, Orca itself runs as well, and `speech()` returns what it said — the end-to-end check.

Run inside `headless.sh` (see `run`), never on a desktop: the keys would go to whatever
window has focus there.
"""

import os
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, Gio, GLib  # noqa: E402

ROOT = Path(__file__).resolve().parents[4]
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug"
APP = Path(os.environ.get("LUMENNA_GTK", TARGET / "lumenna-gtk"))
LUM = Path(os.environ.get("LUM", TARGET / "lum"))

KEYSYMS = {
    "Down": 0xFF54, "Up": 0xFF52, "Left": 0xFF51, "Right": 0xFF53, "Tab": 0xFF09,
    "Return": 0xFF0D, "space": 0x20, "Escape": 0xFF1B, "Home": 0xFF50, "End": 0xFF57,
    "Delete": 0xFFFF, "BackSpace": 0xFF08, "Menu": 0xFF67, "Page_Up": 0xFF55,
    "Page_Down": 0xFF56, "Shift": 0xFFE1, "Control": 0xFFE3, "Alt": 0xFFE9, "comma": 0x2C,
    **{f"F{n}": 0xFFBE + n - 1 for n in range(1, 13)},
}

STATES = [
    ("expanded", Atspi.StateType.EXPANDED),
    ("collapsed", Atspi.StateType.COLLAPSED),
    ("checked", Atspi.StateType.CHECKED),
    ("selected", Atspi.StateType.SELECTED),
]


class Keyboard:
    """Key presses through the compositor's RemoteDesktop session."""

    def __init__(self):
        if not os.environ.get("LUMENNA_HEADLESS"):
            raise RuntimeError("run under headless.sh: these keys would go to the desktop")
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION)
        reply = self._call("/org/gnome/Mutter/RemoteDesktop", "org.gnome.Mutter.RemoteDesktop",
                           "CreateSession", None, "(o)")
        self.session = reply.unpack()[0]
        self._call(self.session, "org.gnome.Mutter.RemoteDesktop.Session", "Start")

    def _call(self, path, interface, method, args=None, reply=None):
        return self.bus.call_sync("org.gnome.Mutter.RemoteDesktop", path, interface, method, args,
                                  GLib.VariantType(reply) if reply else None, 0, -1, None)

    def _key(self, keysym, down):
        self._call(self.session, "org.gnome.Mutter.RemoteDesktop.Session", "NotifyKeyboardKeysym",
                   GLib.Variant("(ub)", (keysym, down)))

    def press(self, combo):
        """A key with its modifiers: `Down`, `Control+k`, `Shift+F10`."""
        parts = combo.split("+") if combo != "+" else ["+"]
        syms = [KEYSYMS.get(part, ord(part) if len(part) == 1 else None) for part in parts]
        if None in syms:
            raise ValueError(f"unknown key in {combo!r}")
        for sym in syms:
            self._key(sym, True)
        for sym in reversed(syms):
            self._key(sym, False)

    def type(self, text):
        for char in text:
            if char.isupper():
                self.press(f"Shift+{char}")
            else:
                self._key(ord(char), True)
                self._key(ord(char), False)


def pump(seconds):
    """Lets AT-SPI events in for a while."""
    end = time.monotonic() + seconds
    context = GLib.MainContext.default()
    while time.monotonic() < end:
        while context.pending():
            context.iteration(False)
        time.sleep(0.02)


def describe(accessible):
    """What a screen reader has to go on: role, name, level, position and states."""
    if accessible is None:
        return "nothing"
    states = accessible.get_state_set()
    attributes = accessible.get_attributes() or {}
    parts = [f"[{accessible.get_role_name()}]", repr(accessible.get_name())]
    if "level" in attributes:
        parts.append(f"level {attributes['level']}")
    if "posinset" in attributes:
        parts.append(f"{attributes['posinset']} of {attributes.get('setsize', '?')}")
    parts += [name for name, state in STATES if states.contains(state)]
    # GTK reports a collapsed row as expandable and not expanded, which Orca says as "collapsed".
    if states.contains(Atspi.StateType.EXPANDABLE) and not states.contains(Atspi.StateType.EXPANDED) \
            and "collapsed" not in parts:
        parts.append("collapsed")
    return " ".join(parts)


class Orca:
    """Orca, on the private bus, logging what it says. The launcher is copied with its check
    for another running Orca narrowed to its own name, so the desktop's Orca is never touched."""

    def __init__(self, directory):
        self.log = Path(directory) / "orca.log"
        launcher = Path(directory) / "orca-headless"
        source = Path(shutil.which("orca"))
        launcher.write_text(source.read_text().replace("-x orca", "-x orca-headless"))
        launcher.chmod(0o755)
        environment = dict(os.environ, XDG_DATA_HOME=str(directory), XDG_CONFIG_HOME=str(directory))
        self.process = subprocess.Popen([str(launcher), f"--debug-file={self.log}"], env=environment,
                                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.seen = 0
        for _ in range(100):
            if self.log.exists() and "Screen reader on" in self.log.read_text(errors="replace"):
                break
            time.sleep(0.1)

    def speech(self):
        """What Orca has said since last asked, one utterance per item."""
        lines = self.log.read_text(errors="replace").splitlines()
        said = [match.group(1) for line in lines[self.seen:]
                if (match := re.search(r"SPEECH OUTPUT: '(.*?)'(?: \{|$)", line))]
        self.seen = len(lines)
        return said

    def stop(self):
        self.process.terminate()
        self.process.wait(timeout=10)


class Session:
    """One run of the app on a store of its own."""

    def __init__(self, seed=(), orca=None, environment=None):
        self.directory = tempfile.mkdtemp(prefix="lumenna-ui-")
        self.environment = dict(
            os.environ,
            LUMENNA_PROFILE=str(Path(self.directory) / "profile"),
            LUMENNA_BACKUP_DIR=str(Path(self.directory) / "backups"),
            **(environment or {}),
        )
        for command in seed:
            self.lum(*command)
        self.announcements = []
        self.listener = Atspi.EventListener.new(self._announced)
        self.listener.register("object:announcement")
        orca = os.environ.get("ORCA") == "1" if orca is None else orca
        self.orca = Orca(self.directory) if orca else None
        self.keyboard = Keyboard()
        # Never the person's global shortcuts, as the Windows tests do.
        self.process = subprocess.Popen([str(APP), "--no-shortcuts"], env=self.environment,
                                        stdout=subprocess.DEVNULL,
                                        stderr=open(Path(self.directory) / "app.log", "w"))
        self.app = None
        for _ in range(100):
            self.app = self._find_app()
            if self.app and self.focused():
                break
            pump(0.1)
        # The first key a new RemoteDesktop session sends can be lost; a lone Shift costs nothing.
        self.keyboard.press("Shift")
        pump(0.5)
        if self.orca:
            self.orca.speech()

    def _announced(self, event):
        if isinstance(event.any_data, str):
            self.announcements.append(event.any_data)

    def _find_app(self):
        desktop = Atspi.get_desktop(0)
        for index in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(index)
            if app and app.get_process_id() == self.process.pid:
                return app
        return None

    def lum(self, *args):
        """Runs `lum` on this session's store, as another process writing to it."""
        result = subprocess.run([str(LUM), *args], env=self.environment, capture_output=True, text=True)
        if result.returncode != 0:
            raise RuntimeError(f"lum {' '.join(args)}: {result.stderr.strip()}")
        return result.stdout

    def press(self, *keys, wait=0.35):
        """Presses each key in turn, letting the app answer after each."""
        for key in keys:
            self.keyboard.press(key)
            pump(wait)

    def type(self, text, wait=0.35):
        self.keyboard.type(text)
        pump(wait)

    def focused(self):
        """What has focus, as Orca would find it: in the active window, since each window keeps
        a focused widget of its own."""
        app = self.app or self._find_app()
        if not app:
            return None
        focused = lambda a: a.get_state_set().contains(Atspi.StateType.FOCUSED)
        for index in range(app.get_child_count()):
            window = app.get_child_at_index(index)
            if window and window.get_state_set().contains(Atspi.StateType.ACTIVE):
                return _find(window, focused)
        return _find(app, focused)

    def active_window(self):
        """The name of the window in front."""
        for index in range(self.app.get_child_count()):
            window = self.app.get_child_at_index(index)
            if window and window.get_state_set().contains(Atspi.StateType.ACTIVE):
                return window.get_name()
        return None

    def wait_for_window(self, name, seconds=10):
        """Waits for a window to come to the front: a new one takes the headless compositor
        well over half a second to show, and keys sent before then go to the one behind."""
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if self.active_window() == name and self.focused() is not None:
                pump(0.2)
                return
            pump(0.1)
        raise AssertionError(f"{name!r} never came to the front; {self.active_window()!r} is")

    def wait_for_alert(self, seconds=10):
        """Waits for an alert to come to the front: a window named by its heading, which is
        whatever the core said, so it is known by not being the main window."""
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            name = self.active_window()
            if name is not None and not name.endswith("Lumenna") and self.focused() is not None:
                pump(0.2)
                return name
            pump(0.1)
        raise AssertionError(f"no alert came to the front; {self.active_window()!r} is")

    def tab_to(self, name, most=8):
        """Tabs until a button named `name` has focus. An alert lays its answers out side by
        side or stacked, as they fit, and Tab follows the layout."""
        for _ in range(most):
            if self.focus() == f"[button] {name!r}":
                return
            self.press("Tab")
        raise AssertionError(f"Tab never reached {name!r}; {self.focus()} has focus")

    def focus(self):
        """What has focus, described."""
        return describe(self.focused())

    def find(self, role, name=None):
        """The first accessible with this role, and name if given, anywhere in the app."""
        return _find(self.app, lambda a: a.get_role_name() == role and (name is None or a.get_name() == name))

    def find_all(self, role):
        found = []
        _walk(self.app, lambda a: found.append(a) if a.get_role_name() == role else None)
        return found

    def said(self):
        """What has been announced since last asked."""
        said, self.announcements = self.announcements, []
        return said

    def speech(self):
        return self.orca.speech() if self.orca else []

    def close(self):
        self.listener.deregister("object:announcement")
        self.process.terminate()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
        if self.orca:
            self.orca.stop()
        shutil.rmtree(self.directory, ignore_errors=True)


def _walk(accessible, visit, depth=0):
    if accessible is None or depth > 60:
        return
    visit(accessible)
    for index in range(accessible.get_child_count()):
        _walk(accessible.get_child_at_index(index), visit, depth + 1)


def _find(accessible, matches, depth=0):
    if accessible is None or depth > 60:
        return None
    try:
        if matches(accessible):
            return accessible
        for index in range(accessible.get_child_count()):
            found = _find(accessible.get_child_at_index(index), matches, depth + 1)
            if found:
                return found
    except GLib.Error:
        return None
    return None
