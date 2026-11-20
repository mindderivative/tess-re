"""0.6.10 (#248): a resizing window presents at most ~250 times a second.

On a headless sway (`tools/sway_live.sh -- python -m pytest tests/test_resize_present_cap_live.py`).
A resize presents without a vsync barrier (#155), which never waits: on KWin with RADV an animating
window committed ~10,000 times a second for the 400 ms after each fullscreen change. The burst
needs a fast adapter (`tools/with_adapter.sh radv`); on lavapipe the present itself is slow, so this
test passes there with or without the cap. It counts `wl_surface.commit` in the protocol log.
"""

import datetime
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import time

import pytest

CHILD = textwrap.dedent(
    """
    from tess_re import App, Window

    app = App()
    window = Window(width=200, height=200, title="resize-cap")
    node = window.create("box", width=60, height=60, position="absolute", x=10, y=10,
                         fill=(200, 50, 50, 255))
    window.root.add_child(node)
    node.animate("rotation_deg", 360000.0, 3_600_000, easing="linear")
    app.add_window(window)
    window.after(25000, app.thread_handle().stop)
    app.run()
    """
)

STAMP = re.compile(r"^\[(\d+):(\d+):(\d+\.\d+)\]")


def _sway():
    if not (shutil.which("swaymsg") and os.environ.get("SWAYSOCK")):
        pytest.skip("needs a headless sway (SWAYSOCK; tools/sway_live.sh)")


def _seconds(line):
    match = STAMP.match(line)
    if not match:
        return None
    h, m, s = match.groups()
    return int(h) * 3600 + int(m) * 60 + float(s)


def _commits(path, t0, span):
    """`wl_surface.commit` lines of the child's protocol log between t0 and t0 + span."""
    count = 0
    with open(path, errors="ignore") as log:
        for line in log:
            at = _seconds(line)
            if at is not None and t0 <= at < t0 + span and re.search(
                r"wl_surface#\d+\.commit\(\)", line
            ):
                count += 1
    return count


def _now():
    now = datetime.datetime.now(datetime.timezone.utc)
    return now.hour * 3600 + now.minute * 60 + now.second + now.microsecond / 1e6


def test_a_size_change_does_not_make_an_animating_window_commit_in_a_burst():
    _sway()
    with tempfile.TemporaryDirectory() as tmp:
        log = os.path.join(tmp, "protocol.log")
        env = dict(os.environ, WAYLAND_DEBUG="1")
        with open(log, "w") as err:
            proc = subprocess.Popen(
                [sys.executable, "-c", CHILD],
                stdout=subprocess.DEVNULL,
                stderr=err,
                env=env,
            )
        try:
            time.sleep(3.0)
            for change in ("enable", "disable"):
                t0 = _now()
                subprocess.run(
                    ["swaymsg", f'[title="resize-cap"] fullscreen {change}'],
                    capture_output=True,
                    check=False,
                )
                time.sleep(1.5)
                n = _commits(log, t0, 0.5)
                # 250 a second is 125 in half a second; unthrottled it was ~4,000.
                assert 0 < n < 400, f"fullscreen {change}: {n} commits in 0.5 s"
            # Settled: back to the compositor's pace, well under the cap.
            t0 = _now()
            time.sleep(1.5)
            assert _commits(log, t0 + 0.2, 1.0) < 200
        finally:
            proc.kill()
            proc.wait()
