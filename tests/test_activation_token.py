"""T-051: `request_activation_token` on a `Window` and a `LayerWindow`.

Without a window open there is nothing to ask, so the callback gets `None` on the next turn
(headless). Under a Wayland compositor that has `xdg_activation_v1` (a private sway has) a live
window gets a real token string. Whether a compositor then FOCUSES the launched app (KWin) is a
hand check: see docs/contributing.md.
"""

import os
import subprocess
import sys
import textwrap

import pytest

from tess_re import Window


def test_a_window_that_is_not_open_answers_none_on_the_next_turn():
    window = Window(width=100, height=100)
    got = []
    window.request_activation_token(got.append)
    assert got == [], "never inside the call"
    window.advance(10)
    assert got == [None]


def test_the_callback_must_be_callable():
    with pytest.raises(ValueError, match="callable"):
        Window().request_activation_token("not callable")  # type: ignore[arg-type]


LIVE = """
import os, threading, time
from tess_re import App, {kind}

app = App()
window = {make}
got = []

def answer(token):
    print("TOKEN", repr(token), flush=True)
    window.close()

def ask():
    window.request_activation_token(answer{extra})

app.add_window(window)
handle = app.thread_handle()
threading.Thread(
    target=lambda: (time.sleep(2.5), handle.call_soon(ask), time.sleep(10), print("TIMEOUT", flush=True), os._exit(1)),
    daemon=True,
).start()
app.run()
"""

LAYER = ("LayerWindow", 'LayerWindow("act-test", anchor=("top", "left", "right"), size=(0, 30), keyboard="none")', ', app_id="demo"')
TOPLEVEL = ("Window", "Window(width=200, height=100)", "")


def run_live(kind, make, extra):
    if not os.environ.get("WAYLAND_DISPLAY"):
        pytest.skip("needs a Wayland compositor (a private sway under tools/sway_live.sh)")
    script = textwrap.dedent(LIVE).format(kind=kind, make=make, extra=extra)
    done = subprocess.run([sys.executable, "-c", script], capture_output=True, text=True, timeout=60)
    lines = [line for line in done.stdout.splitlines() if line.startswith(("TOKEN", "TIMEOUT"))]
    assert lines, (done.stdout, done.stderr)
    return lines[-1]


@pytest.mark.parametrize("spec", [LAYER, TOPLEVEL], ids=["layer_window", "window"])
def test_a_live_window_gets_a_token_from_the_compositor(spec):
    line = run_live(*spec)
    assert line.startswith("TOKEN "), line
    value = line[len("TOKEN "):]
    if value == "None":
        pytest.skip("this compositor has no xdg_activation_v1")
    assert len(value) > 6, line
