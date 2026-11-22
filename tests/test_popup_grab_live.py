"""A popup's grab: a click away from it, and the keyboard it takes (private sway and private KWin).

`PopupWindow(grab=True)` gives the popup the pointer and keyboard: a click outside it ends it with `closed`,
reason `"dismissed"`. It needs real pointer and key events, which the private compositors take without any
device of the real machine: KWin through its own `org_kde_kwin_fake_input`, sway through the wlroots virtual
pointer and keyboard protocols (`live_compositor.input_device()`; no uinput, nothing reaches the real session).

The app is a bar (a 300x40 layer surface, top-left, overlay) whose click opens a popup under it. The test
clicks the bar, waits for the popup and then clicks away or presses Escape. A held popup (`first_present`
`'on_ready'`) is not mapped, so it holds no grab: a click while it is held does nothing, and after `ready()`
a click away dismisses it.
"""

import json
import queue
import subprocess
import sys
import textwrap
import threading
import time

import live_compositor
import pytest
import tess_re

pytestmark = [
    pytest.mark.skipif(
        not sys.platform.startswith("linux") or not tess_re.supports("layer_shell"),
        reason="needs a compositor with the layer-shell protocol",
    ),
    pytest.mark.usefixtures("compositor"),
]

CHILD = textwrap.dedent(
    """
    import json, sys
    from tess_re import App, LayerWindow, PopupWindow

    def say(kind, **kw):
        print(json.dumps({"kind": kind, **kw}), flush=True)

    MODE = %(mode)s
    app = App()
    bar = LayerWindow("popup-grab-bar", layer="overlay", anchor=("top", "left"), size=(300, 40), keyboard="none")
    bar.root.set(fill=(30, 30, 200, 255))
    button = bar.create("box", width=300, height=40, position="absolute", x=0, y=0, fill=(30, 30, 200, 255))
    bar.root.add_child(button)
    popup = PopupWindow(bar, size=(120, 60), anchor_rect=(100, 0, 60, 40), anchor="bottom_left",
                        gravity="bottom_right", grab=True, first_present=MODE)
    popup.root.set(fill=(30, 200, 30, 255))
    keys = popup.create("box", width=120, height=60, position="absolute", x=0, y=0, focusable=True,
                        fill=(30, 200, 30, 255))
    popup.root.add_child(keys)
    keys.on("key_down", lambda e: say("popup key", key=e.key))
    popup.on("closed", lambda e: say("popup closed", reason=e.reason))
    popup.on("configure", lambda e: (say("popup configured"), keys.focus()))
    bar.on("closed", lambda e: say("bar closed", reason=e.reason))
    opened = []

    def open_popup(event):
        if not opened:
            opened.append(1)
            say("click")
            app.add_window(popup)
            if MODE == "on_ready":
                bar.after(%(ready_after)d, lambda: (say("ready"), popup.ready()))

    button.on("click", open_popup)
    app.add_window(bar)
    say("started")
    handle = app.thread_handle()
    bar.after(%(lifetime)d, handle.stop)
    app.run()
    """
)

GREEN = (30, 200, 30)
CLICK_AWAY = (900, 500)  # nothing is there: the desktop
BAR = (20, 20)
POPUP = (110, 60)  # inside the popup: anchored at (100, 40) of the bar, 120x60


class Child:
    """The app in a child process; its stdout lines (JSON) arrive on a queue."""

    def __init__(self, mode="'immediate'", ready_after=1500, lifetime=25000):
        code = CHILD % {"mode": mode, "ready_after": ready_after, "lifetime": lifetime}
        self.process = subprocess.Popen([sys.executable, "-c", code], stdout=subprocess.PIPE, text=True)
        self.events = queue.Queue()
        self.seen = []
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.process.stdout:
            self.events.put(json.loads(line))

    def wait_for(self, kind, timeout=15):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                event = self.events.get(timeout=0.1)
            except queue.Empty:
                continue
            self.seen.append(event)
            if event["kind"] == kind:
                return event
        raise AssertionError(f"no {kind!r} within {timeout} s; saw {self.seen}")

    def kinds(self):
        while True:
            try:
                self.seen.append(self.events.get_nowait())
            except queue.Empty:
                return [event["kind"] for event in self.seen]

    def stop(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


@pytest.fixture
def pointer():
    try:
        device = live_compositor.input_device()
    except RuntimeError as error:
        pytest.skip(str(error))
    with device:
        yield device


@pytest.fixture
def child():
    made = []

    def start(**kwargs):
        made.append(Child(**kwargs))
        return made[-1]

    yield start
    for each in made:
        each.stop()


def open_the_popup(app, pointer):
    app.wait_for("started")
    time.sleep(1.0)  # the bar is mapped and has its first frame
    pointer.click(*BAR)
    app.wait_for("click")


def test_a_click_away_dismisses_a_grabbing_popup(child, pointer):
    app = child()
    open_the_popup(app, pointer)
    app.wait_for("popup configured")
    time.sleep(0.8)
    assert live_compositor.pixel(*POPUP) == GREEN, "the popup is on screen"
    pointer.click(*CLICK_AWAY)
    assert app.wait_for("popup closed")["reason"] == "dismissed"
    time.sleep(0.5)
    assert live_compositor.pixel(*POPUP) != GREEN, "and gone after the click away"
    assert "bar closed" not in app.kinds()


def popup_gets_key(app, pointer, key, timeout=3):
    app.wait_for("popup configured")
    time.sleep(0.8)
    pointer.key(key)
    try:
        return app.wait_for("popup key", timeout=timeout)["key"]
    except AssertionError:
        return None


# KWin gives the keyboard to a popup that grabbed it, whatever its parent asked for. A private sway
# keeps the keyboard where it was (the parent bar, which here asked for none): the popup never hears the key.
# This is the compositors' difference, pinned here (docs/contributing.md, "What differs in KWin").
KEY_REACHES_THE_POPUP = {"kwin": True, "sway": False}


def test_a_grabbing_popup_takes_the_keyboard_where_the_compositor_gives_it(child, pointer, compositor):
    app = child()
    open_the_popup(app, pointer)
    got = popup_gets_key(app, pointer, "a")
    assert got == ("a" if KEY_REACHES_THE_POPUP[compositor] else None), (compositor, app.seen)


def test_escape_is_a_key_like_another_for_a_grabbing_popup(child, pointer, compositor):
    """Neither compositor ends the popup on Escape by itself: the key is the popup's (KWin) or nobody's (sway)."""
    app = child()
    open_the_popup(app, pointer)
    got = popup_gets_key(app, pointer, "Escape")
    assert got == ("escape" if KEY_REACHES_THE_POPUP[compositor] else None), (compositor, app.seen)
    time.sleep(1.0)
    assert "popup closed" not in app.kinds(), app.seen


def test_a_held_popup_is_shown_by_ready_and_a_click_away_then_dismisses_it(child, pointer):
    app = child(mode="'on_ready'", ready_after=4000)
    open_the_popup(app, pointer)
    app.wait_for("popup configured")
    time.sleep(0.5)
    assert live_compositor.pixel(*POPUP) != GREEN, "a held popup is not shown"
    app.wait_for("ready", timeout=15)
    time.sleep(1.0)
    assert live_compositor.pixel(*POPUP) == GREEN, "ready() mapped it"
    assert "popup closed" not in app.kinds(), app.seen
    pointer.click(*CLICK_AWAY)
    assert app.wait_for("popup closed")["reason"] == "dismissed"


# A held popup is not mapped, but its xdg_popup.grab was requested with the role (a grab cannot be asked for
# after the first commit). KWin keeps the grab inert until the popup is mapped; sway has it live: a click
# away while the popup is held ends it before it was ever shown.
CLICK_AWAY_DISMISSES_A_HELD_POPUP = {"kwin": False, "sway": True}


def test_a_click_away_while_the_popup_is_held(child, pointer, compositor):
    app = child(mode="'on_ready'", ready_after=6000)
    open_the_popup(app, pointer)
    app.wait_for("popup configured")
    time.sleep(0.5)
    pointer.click(*CLICK_AWAY)
    time.sleep(1.5)
    dismissed = "popup closed" in app.kinds()
    assert dismissed == CLICK_AWAY_DISMISSES_A_HELD_POPUP[compositor], (compositor, app.seen)
    if not dismissed:
        app.wait_for("ready", timeout=15)
        time.sleep(1.0)
        assert live_compositor.pixel(*POPUP) == GREEN, "the held popup was shown by ready()"
