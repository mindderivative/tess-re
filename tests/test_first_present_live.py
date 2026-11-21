"""0.6.11 (P5): `Window(first_present=...)` holds the first present until the app is ready.

On a headless sway (`tools/sway_live.sh --renderer pixman -- tools/with_adapter.sh lvp -- python -m
pytest tests/test_first_present_live.py`). A Wayland surface is mapped by the first buffer it commits,
so the proof is in the protocol log (`WAYLAND_DEBUG=1`): the toplevel's first `wl_surface.attach` with a
buffer comes after `window.ready()` for `'on_ready'`, after the deadline for a number of milliseconds,
and at once for `'immediate'`. The child writes its own markers into the same stderr, so line order
is the order things happened in. The window's presence in sway's tree and a screenshot of the first
frame's colour (`background`) say the same from the compositor's side. Skips without sway and grim.
X11, macOS and Windows (where the OS window is also kept hidden until the first present) are not
covered here.
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

TITLE = "first-present-probe"

CHILD = textwrap.dedent(
    """
    import sys
    from tess_re import App, Window

    def mark(text):
        print("MARK " + text, file=sys.stderr, flush=True)

    mode = MODE
    app = App()
    window = Window(width=200, height=200, title="first-present-probe", first_present=mode,
                    scheme="light", background=BACKGROUND)
    window.root.add_child(window.create("box", width=60, height=60, fill=(200, 50, 50, 255)))
    app.add_window(window)
    for at, name in ((3000, "check"), (6000, "late")):
        window.after(at, lambda name=name: mark(name))
    def go():
        mark("ready")
        window.ready()
    if READY_AT:
        window.after(READY_AT, go)
    window.after(7500, app.thread_handle().stop)
    app.run()
    """
)

CLOCK = re.compile(r"^\[(\d+):(\d+):(\d+\.\d+)\]")  # newer libwayland: [HH:MM:SS.us] (UTC)
# Ubuntu 24.04's libwayland prints `[%7u.%03u]` of (microseconds / 1000, microseconds % 1000): the
# number is MILLISECONDS with a microsecond fraction (a 2.0095 s gap reads as 2009.523).
MONO = re.compile(r"^\[\s*(\d+\.\d+)\]")
XDG_SURFACE = re.compile(r"get_xdg_surface\(new id xdg_surface[#@]\d+, wl_surface[#@](\d+)\)")
ATTACH = r"wl_surface[#@]%s\.attach\(wl_buffer[#@]\d+"


def _sway():
    if not (shutil.which("swaymsg") and os.environ.get("SWAYSOCK")):
        pytest.skip("needs a headless sway (SWAYSOCK; tools/sway_live.sh)")
    if not shutil.which("grim"):
        pytest.skip("needs grim to read pixels")


def _seconds(line):
    """The protocol log's own timestamp of a line, whichever libwayland wrote it."""
    match = CLOCK.match(line)
    if match:
        h, m, s = match.groups()
        return int(h) * 3600 + int(m) * 60 + float(s)
    match = MONO.match(line)
    return float(match.group(1)) / 1000.0 if match else None


def _lines(path):
    with open(path, errors="ignore") as log:
        return log.read().splitlines()


def _walk(node):
    yield node
    for child in node.get("nodes", []) + node.get("floating_nodes", []):
        yield from _walk(child)


def _node():
    tree = subprocess.run(
        ["swaymsg", "-t", "get_tree"], capture_output=True, text=True, check=False
    ).stdout
    return next((n for n in _walk(json.loads(tree)) if n.get("name") == TITLE), None)


def _pixel(node):
    """The colour of a pixel inside the window, from a screenshot of the sway output."""
    rect, inner = node["rect"], node["window_rect"]
    x = rect["x"] + inner["x"] + inner["width"] - 20
    y = rect["y"] + inner["y"] + inner["height"] - 20
    out = subprocess.run(
        ["grim", "-g", f"{x},{y} 1x1", "-t", "ppm", "-"], capture_output=True, check=False
    ).stdout
    # P6\n1 1\n255\n then r g b
    return tuple(out[-3:])


def run_child(mode, background="None", ready_at=0):
    """Runs the child to its end; returns (protocol log lines, what sway's tree said at each
    probe: {'hold': node or None, 'after': node or None, 'pixel': colour or None})."""
    seen = {}
    with tempfile.TemporaryDirectory() as tmp:
        log = os.path.join(tmp, "protocol.log")
        env = dict(os.environ, WAYLAND_DEBUG="1")
        child = (
            CHILD.replace("MODE", repr(mode))
            .replace("BACKGROUND", background)
            .replace("READY_AT", str(ready_at))
        )
        with open(log, "w") as err:
            proc = subprocess.Popen(
                [sys.executable, "-c", child], stdout=subprocess.DEVNULL, stderr=err, env=env
            )
        try:
            # Wait for the "check" marker (3 s after the window opened), then look at the tree.
            deadline = time.monotonic() + 40
            while time.monotonic() < deadline and not any("MARK check" in l for l in _lines(log)):
                time.sleep(0.1)
            seen["hold"] = _node()
            while time.monotonic() < deadline and not any("MARK late" in l for l in _lines(log)):
                time.sleep(0.1)
            seen["after"] = _node()
            seen["pixel"] = _pixel(seen["after"]) if seen["after"] else None
            proc.wait(timeout=30)
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
        lines = _lines(log)
    return lines, seen


def _index(lines, text, start=0):
    return next((i for i in range(start, len(lines)) if text in lines[i]), None)


def _toplevel_surface(lines):
    for line in lines:
        match = XDG_SURFACE.search(line)
        if match:
            return match.group(1)
    return None


def _first_attach(lines, surface):
    pattern = re.compile(ATTACH % surface)
    return next((i for i, line in enumerate(lines) if pattern.search(line)), None)


def _context(lines):
    return "\n".join(
        line for line in lines if "MARK" in line or "xdg_" in line or "attach" in line
    )[-3000:]


def test_immediate_presents_at_once_and_ready_changes_nothing():
    _sway()
    lines, seen = run_child("immediate", ready_at=100)
    surface = _toplevel_surface(lines)
    assert surface, _context(lines)
    attach = _first_attach(lines, surface)
    check = _index(lines, "MARK check")
    assert attach is not None and attach < check, _context(lines)
    assert seen["hold"] is not None, "mapped by the time the app looks"


def test_on_ready_commits_no_buffer_until_ready():
    _sway()
    lines, seen = run_child("on_ready", ready_at=3500)
    surface = _toplevel_surface(lines)
    assert surface, "the toplevel was created (hidden)\n" + _context(lines)
    ready = _index(lines, "MARK ready")
    check = _index(lines, "MARK check")
    assert ready is not None and check is not None and check < ready, _context(lines)
    attach = _first_attach(lines, surface)
    assert attach is not None, "presented after ready()\n" + _context(lines)
    assert attach > ready, "no buffer before ready()\n" + _context(lines)
    # Not mapped: the compositor has no window for it until the buffer comes.
    assert seen["hold"] is None, seen["hold"]
    assert seen["after"] is not None


def test_a_number_of_milliseconds_is_a_deadline_without_ready():
    _sway()
    lines, seen = run_child(2000)
    surface = _toplevel_surface(lines)
    assert surface, _context(lines)
    attach = _first_attach(lines, surface)
    check = _index(lines, "MARK check")
    assert attach is not None, "presented at the deadline\n" + _context(lines)
    # 2 s after the window was made; the check marker is 3 s after it opened, the first
    # buffer is therefore before it, and not at once.
    assert attach < check, _context(lines)
    created = next(i for i, line in enumerate(lines) if XDG_SURFACE.search(line))
    began, took = _seconds(lines[created]), _seconds(lines[attach])
    assert began is not None and took is not None
    assert 1.8 <= took - began <= 2.9, f"first buffer {took - began:.3f} s after creation"
    assert seen["after"] is not None


def test_ready_before_the_deadline_presents_at_once():
    """A 30 s deadline, ready() at 3.5 s: the buffer comes with ready(), not at the deadline."""
    _sway()
    lines, seen = run_child(30_000, ready_at=3500)
    surface = _toplevel_surface(lines)
    assert surface, _context(lines)
    ready = _index(lines, "MARK ready")
    attach = _first_attach(lines, surface)
    assert ready is not None and attach is not None and attach > ready, _context(lines)
    assert seen["hold"] is None and seen["after"] is not None


def test_a_long_deadline_holds_a_window_that_is_never_ready():
    _sway()
    lines, seen = run_child(30_000)
    surface = _toplevel_surface(lines)
    assert surface
    assert _first_attach(lines, surface) is None, _context(lines)
    assert seen["hold"] is None and seen["after"] is None


def test_the_first_frame_is_cleared_to_the_background_colour():
    _sway()
    lines, seen = run_child("immediate", background="(10, 200, 30, 255)")
    assert seen["pixel"] == (10, 200, 30), (seen["pixel"], _context(lines))


def test_the_default_background_is_the_scheme_surface():
    _sway()
    lines, seen = run_child("immediate")
    # scheme="light": role `surface` is #FEF7FF.
    assert seen["pixel"] == (0xFE, 0xF7, 0xFF), (seen["pixel"], _context(lines))
