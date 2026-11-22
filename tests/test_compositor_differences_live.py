"""Where a private KWin and a private sway answer the same app differently (docs/contributing.md,
"What differs in KWin"). Each test states what both do, so a change on either side shows up here.

The desktop runs KWin; CI and `tools/sway_live.sh` run a tiling wlroots compositor. A test that passes in
sway has not shown it works on KWin: windows are placed, sized and decorated by different rules.
"""

import subprocess
import sys
import textwrap
import time

import live_compositor
import pytest

pytestmark = pytest.mark.usefixtures("compositor")

CHILD = textwrap.dedent(
    """
    from tess_re import App, Window
    app = App()
    window = Window(width=200, height=200, title="placement-probe", scheme="light", background=(10, 200, 30, 255))
    app.add_window(window)
    window.after(8000, app.thread_handle().stop)
    app.run()
    """
)


def probe_window():
    child = subprocess.Popen([sys.executable, "-c", CHILD])
    try:
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            found = live_compositor.toplevel("placement-probe")
            if found:
                time.sleep(1.0)  # let the compositor settle the size
                return live_compositor.toplevel("placement-probe"), live_compositor.output_rect()
            time.sleep(0.3)
        raise AssertionError("the window never appeared")
    finally:
        child.terminate()
        child.wait(timeout=20)


def test_a_new_toplevel_is_placed_and_sized_by_the_compositor(compositor):
    """KWin (floating) gives the window the size it asked for, centres it and adds its own title bar; sway
    tiles it over the whole output and ignores the size."""
    window, (ox, oy, width, height) = probe_window()
    if compositor == "kwin":
        assert (window["width"], window["height"]) == (200, 200), window
        assert window["x"] > ox + 100 and window["y"] > oy + 100, window
        assert window["x"] + window["width"] < ox + width - 100, "centred, not tiled"
    else:
        assert window["width"] > 1000 and window["height"] > 600, window
