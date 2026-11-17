"""T-020 (TRE-REQ-020): a `LayerWindow.set(...)` made before the surface exists must reach the compositor.

`set()` right after `App.add_window`, before the loop has run the surface's created hook, was stored and
then dropped: `get()` read the new value back but the compositor never heard of it, so a launcher opened
without keyboard focus. The stored configuration is now sent as soon as the surface exists, in the order
it was made.

The child runs with `WAYLAND_DEBUG=1`, and the test reads the requests the new surface sent. Needs a
Wayland compositor with the layer shell: runs in a private sway (`tools/sway_live.sh`). libwayland prints an
object as `name#id` or `name@id` depending on its version (CI's is older than a rolling distro's), so both are read.
"""

import os
import re
import subprocess
import sys
import textwrap

import pytest

SCRIPT = """
import os, threading, time
from tess_re import App, LayerWindow

app = App()
bar = LayerWindow("update-early-bar", anchor=("top", "left", "right"), size=(0, 40), exclusive_zone=40, keyboard="none")
bar.root.set(fill=(30, 30, 30, 255))
app.add_window(bar)
late = LayerWindow("update-early", anchor=("bottom", "left", "right"), size=(0, 30), keyboard="none")
late.root.set(fill=(40, 90, 40, 255))
handle = app.thread_handle()

def act():
    app.add_window(late)
    %(change)s
    print("GET", %(read)s, flush=True)

threading.Thread(
    target=lambda: (time.sleep(2.0), handle.call_soon(act), time.sleep(3.0), os._exit(0)), daemon=True
).start()
app.run()
"""


def run(change, read):
    if not os.environ.get("WAYLAND_DISPLAY"):
        pytest.skip("needs a Wayland compositor with the layer shell (tools/sway_live.sh)")
    script = textwrap.dedent(SCRIPT) % {"change": change, "read": read}
    done = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True,
        text=True,
        timeout=60,
        env=dict(os.environ, WAYLAND_DEBUG="1"),
    )
    got = next((line[4:] for line in done.stdout.splitlines() if line.startswith("GET ")), None)
    made = re.search(
        r'get_layer_surface\(new id zwlr_layer_surface_v1[#@](\d+), wl_surface[#@](\d+), [^"]*"update-early"\)',
        done.stderr,
    )
    assert made, done.stderr[-2000:]
    layer, surface = made.groups()
    return got, layer, surface, done.stderr


def test_a_keyboard_mode_set_before_the_surface_exists_reaches_the_compositor():
    got, layer, _, trace = run('late.set(keyboard="exclusive")', 'late.get("keyboard")')
    assert got == "exclusive"
    modes = re.findall(rf"zwlr_layer_surface_v1[#@]{layer}\.set_keyboard_interactivity\((\d)\)", trace)
    assert modes and modes[-1] == "1", modes  # exclusive, and last


def test_an_input_region_set_before_the_surface_exists_reaches_the_compositor():
    got, _, surface, trace = run("late.set(input_region=[(0, 0, 10, 10)])", 'late.get("input_region")')
    assert got is not None
    regions = re.findall(rf"wl_surface[#@]{surface}\.set_input_region\(([^)]*)\)", trace)
    assert regions and regions[-1].startswith("wl_region"), regions  # a rectangle, not "nil", and last


def test_several_sets_before_the_surface_exists_arrive_as_the_last_one():
    _, layer, _, trace = run(
        'late.set(keyboard="exclusive"); late.set(keyboard="on_demand")', 'late.get("keyboard")'
    )
    modes = re.findall(rf"zwlr_layer_surface_v1[#@]{layer}\.set_keyboard_interactivity\((\d)\)", trace)
    assert modes and modes[-1] == "2", modes  # on_demand
