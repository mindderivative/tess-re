"""0.6.10 (#248): a resizing window presents at most ~250 times a second.

On a headless sway (`tools/sway_live.sh -- python -m pytest tests/test_resize_present_cap_live.py`).
A resize presents without a vsync barrier (#155), which never waits: on KWin with RADV an animating
window committed ~10,000 times a second for the 400 ms after each fullscreen change. The burst
needs a fast adapter (`tools/with_adapter.sh radv`); on lavapipe the present itself is slow, so this
test passes there with or without the cap. It counts `wl_surface.commit` in the protocol log.
"""

import json
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

CLOCK = re.compile(r"^\[(\d+):(\d+):(\d+\.\d+)\]")  # newer libwayland: [HH:MM:SS.us] (UTC)
# Ubuntu 24.04's libwayland prints `[%7u.%03u]` of (microseconds / 1000, microseconds % 1000): the
# number is MILLISECONDS with a microsecond fraction.
MONO = re.compile(r"^\[\s*(\d+\.\d+)\]")
COMMIT = re.compile(r"wl_surface[#@]\d+\.commit\(\)")


def _sway():
    if not (shutil.which("swaymsg") and os.environ.get("SWAYSOCK")):
        pytest.skip("needs a headless sway (SWAYSOCK; tools/sway_live.sh)")


def _seconds(line):
    """The protocol log's own timestamp of a line, in seconds, whichever libwayland wrote it."""
    match = CLOCK.match(line)
    if match:
        h, m, s = match.groups()
        return int(h) * 3600 + int(m) * 60 + float(s)
    match = MONO.match(line)
    return float(match.group(1)) / 1000.0 if match else None


def _lines(path):
    with open(path, errors="ignore") as log:
        return log.read().splitlines()


def _burst(lines, start, span):
    """Commits within `span` seconds of the first commit at or after line `start`, by the
    log's own clock. Returns (count, first commit time or None)."""
    first, count = None, 0
    for line in lines[start:]:
        at = _seconds(line)
        if at is None or not COMMIT.search(line):
            continue
        if first is None:
            first = at
        if at - first >= span:
            break
        count += 1
    return count, first


def _diagnosis(lines, start, extra=""):
    stamps = [t for t in map(_seconds, lines) if t is not None]
    commits = sum(1 for line in lines if COMMIT.search(line))
    return (
        f"{extra} log lines {len(lines)}, change at line {start}, commits in log {commits}, "
        f"first/last stamp {stamps[:1]}/{stamps[-1:]}; last lines:\n" + "\n".join(lines[-15:])
    )


def _fullscreen(change):
    out = subprocess.run(
        ["swaymsg", f'[title="resize-cap"] fullscreen {change}'],
        capture_output=True,
        text=True,
        check=False,
    )
    return out


def _is_fullscreen():
    tree = subprocess.run(
        ["swaymsg", "-t", "get_tree"], capture_output=True, text=True, check=False
    ).stdout
    node = next(
        (n for n in _walk(json.loads(tree)) if n.get("name") == "resize-cap"), None
    )
    return bool(node and node.get("fullscreen_mode"))


def _walk(node):
    yield node
    for child in node.get("nodes", []) + node.get("floating_nodes", []):
        yield from _walk(child)


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
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline and not _is_window():
                time.sleep(0.2)
            time.sleep(2.0)
            for change in ("enable", "disable"):
                start = len(_lines(log))
                out = _fullscreen(change)
                assert out.returncode == 0 and '"success": true' in out.stdout, (
                    f"swaymsg fullscreen {change}: rc {out.returncode} {out.stdout} {out.stderr}"
                )
                assert _is_fullscreen() == (change == "enable")
                time.sleep(2.0)
                lines = _lines(log)
                n, first = _burst(lines, start, 0.5)
                assert first is not None, _diagnosis(lines, start, f"{change}: no commit.")
                # 250 a second is 125 in half a second; unthrottled it was ~4,000.
                assert n < 400, _diagnosis(lines, start, f"{change}: {n} commits in 0.5 s.")
        finally:
            proc.kill()
            proc.wait()


def _is_window():
    tree = subprocess.run(
        ["swaymsg", "-t", "get_tree"], capture_output=True, text=True, check=False
    ).stdout
    return any(n.get("name") == "resize-cap" for n in _walk(json.loads(tree)))
