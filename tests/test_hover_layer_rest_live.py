"""A button's `hover_layer` fades out and leaves nothing behind, on a real compositor (private sway and KWin).

A button on a layer surface with `hover_layer` and a short `hover_layer_ms` (the spec's 15 ms is shorter than a
frame) is entered, pressed, released and left quickly; once it is at rest the surface's pixels must be the ones
from before the pointer came. The compositor's frame pacing decides which frames are drawn, so this is a live test:
headless runs tick on a clock of their own.
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
]

CHILD = textwrap.dedent(
    """
    import json
    from tess_re import App, LayerWindow

    def say(kind, **kw):
        print(json.dumps({"kind": kind, **kw}), flush=True)

    app = App()
    bar = LayerWindow("hover-rest-bar", layer="overlay", anchor=("top", "left"), size=(300, 60), keyboard="none")
    bar.root.set(fill=(40, 40, 60, 255))
    button = bar.create("box", width=200, height=40, position="absolute", x=40, y=10, fill=(200, 200, 210, 255),
                        corner_radius=6, focusable=True, hover_layer=(255, 255, 255, 40),
                        pressed_layer=(255, 255, 255, 70), hover_layer_ms=%(ms)s)
    label = bar.create("text", text="Button", font_size=16, fill=(20, 20, 30, 255), position="absolute", x=10, y=10)
    button.add_child(label)
    bar.root.add_child(button)
    bar.on("configure", lambda e: say("configured"))
    button.on("click", lambda e: say("click"))
    app.add_window(bar)
    say("started")
    handle = app.thread_handle()
    bar.after(60000, handle.stop)
    app.run()
    """
)

BAR_AT = (0, 0)  # the surface is anchored top-left of the output
OUTSIDE = (800, 500)  # nothing there but the desktop


class Child:
    def __init__(self, ms):
        self.process = subprocess.Popen([sys.executable, "-c", CHILD % {"ms": ms}], stdout=subprocess.PIPE, text=True)
        self.events = queue.Queue()
        self.seen = []
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.process.stdout:
            self.events.put(json.loads(line))

    def wait_for(self, kind, timeout=20):
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


@pytest.mark.parametrize("ms", [15, 30, 100])
@pytest.mark.parametrize("pause", [0.0, 0.03, 0.12])
def test_the_hover_layer_leaves_no_pixel_behind(compositor, pointer, ms, pause):
    app = Child(ms)
    try:
        app.wait_for("configured")
        time.sleep(1.5)
        pointer.move(*OUTSIDE)
        time.sleep(0.8)
        box = (30, 5, 230, 50)  # the button and a margin round it
        before = live_compositor.region(box[0], box[1], box[2], box[3])
        center = (BAR_AT[0] + 140, BAR_AT[1] + 30)
        for round_ in range(6):
            pointer.move(*center)
            time.sleep(pause)
            pointer.press()
            time.sleep(pause)
            pointer.release()
            time.sleep(pause)
            pointer.move(*OUTSIDE)
            time.sleep(0.8)
            after = live_compositor.region(box[0], box[1], box[2], box[3])
            differing = sum(1 for a, b in zip(before, after) if a != b)
            assert differing == 0, f"round {round_}: {differing} bytes differ at rest (hover_layer_ms={ms}, pause={pause})"
    finally:
        app.stop()
