"""TRE-REQ-024: knowing that the keyboard left a layer surface, and that the user clicked elsewhere.

Two signals a panel that holds the keyboard exclusively (a launcher) wants, and what tre gives for each:

* **The keyboard leaves**: the window's `active` event. A `LayerWindow` gets it from the compositor's keyboard
  enter and leave, so it is `False` when another surface takes the keyboard and `True` when it returns.
* **A press lands outside**: no new event. A transparent 1x1 `PopupWindow(grab=True)` hung off the panel is
  dismissed by a press on another application's surface, and its `closed` event (reason `"dismissed"`) is the
  signal. The panel keeps its keyboard. Wayland gives a client no way to see a press on another client's
  surface without taking it, so the press is the popup's, not the application's below; and a grab is one-shot.

Both run on a private sway: they need `wtype` (a virtual keyboard, so the seat has one) and, for the click,
`pywayland` (`tests/wlr_pointer.py`, a virtual pointer; never `/dev/uinput`). They skip without them.
"""

import json
import os
import shutil
import subprocess
import sys
import textwrap
import time

import pytest

sys.path.insert(0, os.path.dirname(__file__))

KEYBOARD_CHILD = """
import json, os, threading, time
from tess_re import App, LayerWindow

def say(**kw):
    print(json.dumps(kw), flush=True)

app = App()
first = LayerWindow("focus-first", anchor=("top", "left", "right"), size=(0, 30), keyboard="exclusive")
second = LayerWindow("focus-second", anchor=("bottom", "left", "right"), size=(0, 30), keyboard="exclusive")
first.on("active", lambda e: say(who="first", active=e.active))
second.on("active", lambda e: say(who="second", active=e.active))
app.add_window(first)
handle = app.thread_handle()

def go():
    time.sleep(2.5)
    say(step="open second")
    handle.call_soon(lambda: app.add_window(second))
    time.sleep(2.5)
    say(step="close second")
    handle.call_soon(second.close)
    time.sleep(2.5)
    say(step="done")
    os._exit(0)

threading.Thread(target=go, daemon=True).start()
app.run()
"""

CLICK_CHILD = """
import json, os, threading, time
from tess_re import App, LayerWindow, PopupWindow

def say(**kw):
    print(json.dumps(kw), flush=True)

app = App()
launcher = LayerWindow("focus-launcher", anchor=("bottom",), size=(300, 120), keyboard="exclusive")
launcher.root.set(fill=(30, 60, 120, 255), padding=8)
field = launcher.create("text_input", width=280, height=30, text="", focusable=True)
launcher.root.add_child(field)
field.on("change", lambda e: say(kind="typed", text=field.get("text")))
launcher.on("active", lambda e: say(kind="active", active=e.active))
# The click catcher: transparent, 1x1, grabbing. Its `closed` is the click.
catcher = PopupWindow(launcher, size=(1, 1), anchor_rect=(0, 0, 1, 1), grab=True)
catcher.root.set(fill=(0, 0, 0, 0))
catcher.on("closed", lambda e: say(kind="click_away", reason=e.reason))
app.add_window(launcher)
handle = app.thread_handle()

def go():
    time.sleep(2.5)
    handle.call_soon(field.focus)
    handle.call_soon(lambda: app.add_window(catcher))
    time.sleep(0.5)
    say(kind="ready")
    time.sleep(14)
    os._exit(0)

threading.Thread(target=go, daemon=True).start()
app.run()
"""

OTHER_APP = """
import os, threading, time
from tess_re import App, Window
app = App()
window = Window(width=400, height=300, title="another-application")
window.root.set(fill=(200, 40, 40, 255))
app.add_window(window)
threading.Thread(target=lambda: (time.sleep(25), os._exit(0)), daemon=True).start()
app.run()
"""


def need_sway():
    if not os.environ.get("WAYLAND_DISPLAY") or not os.environ.get("SWAYSOCK"):
        pytest.skip("needs a private sway (tools/sway_live.sh)")
    if not shutil.which("wtype"):
        pytest.skip("needs wtype: a virtual keyboard, so the private compositor's seat has one to focus")


def events_until(process, kind, timeout):
    """JSON lines the child printed, up to and including the first with `step` or `kind` equal to `kind`."""
    got, end = [], time.time() + timeout
    while time.time() < end:
        line = process.stdout.readline()
        if not line:
            break
        if line.startswith("{"):
            event = json.loads(line)
            got.append(event)
            if kind in (event.get("step"), event.get("kind")):
                break
    return got


def test_the_keyboard_leaving_a_layer_window_is_its_active_event():
    need_sway()
    keyboard = subprocess.Popen(["wtype", "a", "-s", "30000", "b"])  # holds the seat's keyboard
    child = subprocess.Popen(
        [sys.executable, "-c", textwrap.dedent(KEYBOARD_CHILD)], stdout=subprocess.PIPE, text=True
    )
    try:
        seen = events_until(child, "done", 40)
    finally:
        child.kill()
        keyboard.kill()
    active = [(e["who"], e["active"]) for e in seen if "who" in e]
    # It has the keyboard; a second exclusive surface takes it; closing that one gives it back.
    assert active == [("first", True), ("first", False), ("second", True), ("first", True)], seen


def test_a_click_on_another_application_dismisses_the_catcher_and_the_panel_keeps_the_keyboard():
    need_sway()
    import wlr_pointer

    if not wlr_pointer.available():
        pytest.skip("needs pywayland for a virtual pointer (tests/wlr_pointer.py)")
    pointer = wlr_pointer.Pointer(os.environ)
    other = subprocess.Popen([sys.executable, "-c", textwrap.dedent(OTHER_APP)])
    time.sleep(2.0)  # another client's window is on the output
    # One keystroke now, and one a few seconds after the click, from a virtual keyboard that stays.
    keyboard = subprocess.Popen(["wtype", "-s", "6000", "a", "-s", "6000", "z"], stdout=subprocess.DEVNULL)
    child = subprocess.Popen(
        [sys.executable, "-c", textwrap.dedent(CLICK_CHILD)], stdout=subprocess.PIPE, text=True
    )
    try:
        events = events_until(child, "ready", 30)
        assert any(e.get("kind") == "ready" for e in events), events
        pointer.move(100, 100, 1280, 720)  # on the other application's window, outside the panel
        pointer.click()
        events += events_until(child, "click_away", 15)
        away = [e for e in events if e.get("kind") == "click_away"]
        assert away and away[0]["reason"] == "dismissed", events
        # The panel still has the keyboard: a key typed after the click reaches its field.
        events += events_until(child, "typed", 15)
        assert any(e.get("kind") == "typed" for e in events), events
        assert not any(e.get("kind") == "active" and e["active"] is False for e in events), (
            "the click did not take the keyboard from the panel",
            events,
        )
    finally:
        child.kill()
        other.kill()
        keyboard.kill()
        pointer.close()
