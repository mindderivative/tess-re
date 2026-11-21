"""0.6.12: `LayerWindow(first_present=...)` and `PopupWindow(first_present=...)` hold the first present.

A layer surface and an `xdg_popup` are mapped by the first buffer they commit. The layer thread has
already acknowledged the configure by then (before the app hears of it), so holding the present is all
a hold takes: the surface has a role and a size but no buffer, and the compositor shows nothing until
`ready()` (or the deadline). Read from the protocol log (`WAYLAND_DEBUG`) and, for what the compositor
shows, from a screenshot of the headless sway's output.

Needs a layer-shell compositor with `grim`, a private sway (`tools/sway_live.sh`); skips without them.
The protocol log's timestamps differ by libwayland (`[HH:MM:SS.us]` locally, `[ms.us]` on Ubuntu's):
the helpers of `test_first_present_live` read both.
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
import tess_re

sys.path.insert(0, os.path.dirname(__file__))
from test_first_present_live import _first_attach, _index, _lines, _seconds

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux")
    or not tess_re.supports("layer_shell")
    or not shutil.which("grim")
    or not shutil.which("swaymsg")
    or not os.environ.get("SWAYSOCK"),
    reason="needs a private sway (tools/sway_live.sh) with grim",
)

LAYER_SURFACE = re.compile(
    r"get_layer_surface\(new id zwlr_layer_surface_v1[#@]\d+, wl_surface[#@](\d+)"
)
XDG_SURFACE = re.compile(
    r"get_xdg_surface\(new id xdg_surface[#@]\d+, wl_surface[#@](\d+)\)"
)

HEAD = textwrap.dedent(
    """
    import sys
    from tess_re import App, LayerWindow, PopupWindow

    def mark(text):
        print("MARK " + text, file=sys.stderr, flush=True)

    mode = MODE
    app = App()
    """
)

# A 200x40 red bar at the output's top-left corner, on the overlay layer.
LAYER_CHILD = HEAD + textwrap.dedent(
    """
    bar = LayerWindow("fp-layer", layer="overlay", anchor=("top", "left"), size=(200, 40),
                      keyboard=KEYBOARD, input_region=REGION, first_present=mode,
                      background=BACKGROUND)
    bar.root.set(fill=FILL)
    app.add_window(bar)
    bar.on("closed", lambda e: mark("closed " + e.reason))
    bar.after(1500, lambda: (bar.set(keyboard="on_demand"), mark("set")))
    for at, name in ((3000, "check"), (6000, "late")):
        bar.after(at, lambda name=name: mark(name))
    def go():
        mark("ready")
        bar.ready()
    if READY_AT:
        bar.after(READY_AT, go)
    bar.after(7500, app.thread_handle().stop)
    app.run()
    """
)

# The bar is immediate; the popup under it, 100x60 and green, is added 500 ms in with the mode under test.
POPUP_CHILD = HEAD + textwrap.dedent(
    """
    bar = LayerWindow("fp-parent", layer="overlay", anchor=("top", "left"), size=(300, 40))
    bar.root.set(fill=(30, 30, 200, 255))
    popup = PopupWindow(bar, size=(100, 60), anchor_rect=(100, 0, 60, 40), anchor="bottom_left",
                        gravity="bottom_right", grab=GRAB, first_present=mode)
    popup.root.set(fill=(30, 200, 30, 255))
    popup.on("closed", lambda e: mark("popup closed " + e.reason))
    popup.on("configure", lambda e: mark("popup configured"))
    app.add_window(bar)
    bar.after(500, lambda: (app.add_window(popup), mark("added")))
    for at, name in ((3000, "check"), (6000, "late")):
        bar.after(at, lambda name=name: mark(name))
    def go():
        mark("ready")
        popup.ready()
    if READY_AT:
        bar.after(READY_AT, go)
    if CLOSE_PARENT_AT:
        bar.after(CLOSE_PARENT_AT, lambda: (mark("close parent"), bar.close()))
    bar.after(7500, app.thread_handle().stop)
    app.run()
    """
)


def _pixel(x, y):
    """The colour at output coordinates (x, y), from a screenshot of the sway output."""
    out = subprocess.run(
        ["grim", "-g", f"{x},{y} 1x1", "-t", "ppm", "-"],
        capture_output=True,
        check=False,
    ).stdout
    return tuple(out[-3:])


def _origin():
    outputs = json.loads(
        subprocess.run(
            ["swaymsg", "-t", "get_outputs", "-r"], check=False, capture_output=True
        ).stdout
    )
    rect = outputs[0]["rect"]
    return rect["x"], rect["y"]


def run_child(template, at, **fill):
    """Runs the child to its end. `at` is {marker name: (dx, dy)}: the output pixel read right
    after the marker appears in the log. Returns (log lines, {marker: pixel})."""
    seen = {}
    values = {
        "MODE": "'immediate'",
        "BACKGROUND": "None",
        "FILL": "(200, 30, 30, 255)",
        "KEYBOARD": "'none'",
        "REGION": "None",
        "READY_AT": "0",
        "GRAB": "False",
        "CLOSE_PARENT_AT": "0",
    }
    values.update({k.upper(): v for k, v in fill.items()})
    child = template
    for key, value in values.items():
        child = child.replace(key, str(value))
    ox, oy = _origin()
    with tempfile.TemporaryDirectory() as tmp:
        log = os.path.join(tmp, "protocol.log")
        env = dict(os.environ, WAYLAND_DEBUG="1")
        with open(log, "w") as err:
            proc = subprocess.Popen(
                [sys.executable, "-c", child],
                stdout=subprocess.DEVNULL,
                stderr=err,
                env=env,
            )
        try:
            deadline = time.monotonic() + 40
            for name, (dx, dy) in at.items():
                while time.monotonic() < deadline and not any(
                    f"MARK {name}" in line for line in _lines(log)
                ):
                    time.sleep(0.05)
                time.sleep(0.15)
                seen[name] = _pixel(ox + dx, oy + dy)
            proc.wait(timeout=30)
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
        lines = _lines(log)
    return lines, seen


def _layer_surface(lines):
    for line in lines:
        match = LAYER_SURFACE.search(line)
        if match:
            return match.group(1)
    return None


def _popup_surface(lines):
    # The only xdg_surface of an app that has layer surfaces and popups.
    for line in lines:
        match = XDG_SURFACE.search(line)
        if match:
            return match.group(1)
    return None


def _context(lines):
    keep = ("MARK", "layer_surface", "xdg_", "attach", "interactivity", "region")
    return "\n".join(line for line in lines if any(k in line for k in keep))[-3500:]


RED = (200, 30, 30)
GREEN = (30, 200, 30)
CHECK_LAYER = {"check": (20, 20), "late": (20, 20)}


def test_immediate_layer_window_presents_at_once_and_ready_changes_nothing():
    lines, seen = run_child(LAYER_CHILD, CHECK_LAYER, ready_at=100)
    surface = _layer_surface(lines)
    assert surface, _context(lines)
    attach = _first_attach(lines, surface)
    assert attach is not None and attach < _index(lines, "MARK check"), _context(lines)
    assert seen["check"] == RED, seen


def test_a_layer_surface_commits_no_buffer_until_ready_and_the_compositor_shows_nothing():
    lines, seen = run_child(LAYER_CHILD, CHECK_LAYER, mode="'on_ready'", ready_at=3500)
    surface = _layer_surface(lines)
    assert surface, _context(lines)
    ready, check = _index(lines, "MARK ready"), _index(lines, "MARK check")
    assert ready is not None and check is not None and check < ready, _context(lines)
    attach = _first_attach(lines, surface)
    assert attach is not None, "presented after ready()\n" + _context(lines)
    assert attach > ready, "no buffer before ready()\n" + _context(lines)
    assert seen["check"] != RED, ("the compositor showed a held surface", seen)
    assert seen["late"] == RED, seen


def test_the_configure_is_acknowledged_while_held():
    lines, _ = run_child(
        LAYER_CHILD, {"check": (20, 20)}, mode="'on_ready'", ready_at=3500
    )
    surface = _layer_surface(lines)
    ack = _index(lines, "ack_configure")
    ready = _index(lines, "MARK ready")
    assert ack is not None and ready is not None and ack < ready, _context(lines)
    assert surface


def test_settings_made_before_the_first_commit_still_apply():
    """Keyboard mode and input region go with the role-setting commit; a `set` made while held is
    committed too (buffer-less), and none of it maps the surface."""
    lines, _ = run_child(
        LAYER_CHILD,
        {"check": (20, 20)},
        mode="'on_ready'",
        keyboard="'exclusive'",
        region="[(0, 0, 50, 20)]",
        ready_at=3500,
    )
    surface = _layer_surface(lines)
    ready = _index(lines, "MARK ready")
    assert surface and ready is not None, _context(lines)
    # zwlr_layer_surface_v1.set_keyboard_interactivity(1) = exclusive, (2) = on_demand
    exclusive = _index(lines, "set_keyboard_interactivity(1)")
    on_demand = _index(lines, "set_keyboard_interactivity(2)")
    assert exclusive is not None and exclusive < ready, _context(lines)
    assert on_demand is not None and _index(lines, "MARK set") < on_demand < ready, (
        _context(lines)
    )
    assert _index(lines, "wl_region") is not None and _index(lines, "wl_region") < ready
    assert any(
        re.search(r"wl_region[#@]\d+\.add\(0, 0, 50, 20\)", line)
        for line in lines[:ready]
    ), _context(lines)
    attach = _first_attach(lines, surface)
    assert attach is not None and attach > ready, _context(lines)


def test_a_number_of_milliseconds_is_a_deadline_without_ready_for_a_layer_surface():
    lines, seen = run_child(LAYER_CHILD, CHECK_LAYER, mode="2000")
    surface = _layer_surface(lines)
    assert surface, _context(lines)
    attach = _first_attach(lines, surface)
    check = _index(lines, "MARK check")
    assert attach is not None, "presented at the deadline\n" + _context(lines)
    assert attach < check, _context(lines)
    began = next(_seconds(line) for line in lines if LAYER_SURFACE.search(line))
    took = _seconds(lines[attach])
    assert began is not None and took is not None
    assert 1.8 <= took - began <= 2.9, (
        f"first buffer {took - began:.3f} s after creation"
    )
    assert seen["check"] == RED, seen


def test_ready_before_the_deadline_presents_at_once_for_a_layer_surface():
    lines, _ = run_child(LAYER_CHILD, {"check": (20, 20)}, mode="30000", ready_at=3500)
    surface = _layer_surface(lines)
    ready = _index(lines, "MARK ready")
    attach = _first_attach(lines, surface)
    assert ready is not None and attach is not None and attach > ready, _context(lines)


def test_a_long_deadline_holds_a_layer_surface_that_is_never_ready():
    lines, seen = run_child(LAYER_CHILD, CHECK_LAYER, mode="30000")
    surface = _layer_surface(lines)
    assert surface and _first_attach(lines, surface) is None, _context(lines)
    assert seen["late"] != RED, seen


def test_a_set_background_applies_to_a_layer_surface_and_the_default_is_see_through():
    _, seen = run_child(
        LAYER_CHILD,
        {"check": (20, 20)},
        fill="(0, 0, 0, 0)",
        background="(10, 200, 30, 255)",
    )
    assert seen["check"] == (10, 200, 30), seen
    # No background and no fill: the layer clears to nothing, so the output behind it shows.
    _, bare = run_child(LAYER_CHILD, {"check": (20, 20)}, fill="(0, 0, 0, 0)")
    assert bare["check"] != (10, 200, 30) and bare["check"] != RED, bare


# --- popups ------------------------------------------------------------------------------------------

POPUP_AT = {"check": (110, 60), "late": (110, 60)}


def test_an_immediate_popup_is_mapped_at_once():
    lines, seen = run_child(POPUP_CHILD, POPUP_AT, ready_at=100)
    popup = _popup_surface(lines)
    assert popup, _context(lines)
    attach = _first_attach(lines, popup)
    assert attach is not None and attach < _index(lines, "MARK check"), _context(lines)
    assert seen["check"] == GREEN, seen


@pytest.mark.parametrize("grab", ["False", "True"])
def test_a_held_popup_maps_after_ready_and_its_parent_is_not_blocked(grab):
    lines, seen = run_child(
        POPUP_CHILD, POPUP_AT, mode="'on_ready'", ready_at=3500, grab=grab
    )
    popup = _popup_surface(lines)
    assert popup, _context(lines)
    ready, check = _index(lines, "MARK ready"), _index(lines, "MARK check")
    assert ready is not None and check is not None and check < ready, _context(lines)
    assert _index(lines, "MARK popup configured") is not None, (
        "the configure was received"
    )
    attach = _first_attach(lines, popup)
    assert attach is not None and attach > ready, _context(lines)
    closed = _index(lines, "MARK popup closed")
    assert closed is None or closed > _index(lines, "MARK late"), (
        "a held popup is not dismissed"
    )
    assert seen["check"] != GREEN, ("the compositor showed a held popup", seen)
    assert seen["late"] == GREEN, seen
    # The parent (a layer surface) drew while the popup was held.
    parent = _layer_surface(lines)
    assert _first_attach(lines, parent) < _index(lines, "MARK added"), _context(lines)


def test_a_popup_deadline_maps_it_without_ready():
    lines, seen = run_child(POPUP_CHILD, POPUP_AT, mode="2000")
    popup = _popup_surface(lines)
    assert popup, _context(lines)
    attach = _first_attach(lines, popup)
    assert attach is not None, _context(lines)
    created = next(i for i, line in enumerate(lines) if XDG_SURFACE.search(line))
    began, took = _seconds(lines[created]), _seconds(lines[attach])
    assert began is not None and took is not None
    assert 1.8 <= took - began <= 2.9, (
        f"first buffer {took - began:.3f} s after creation"
    )
    assert seen["check"] == GREEN, seen


def test_a_parent_that_closes_while_its_popup_is_held_dismisses_the_popup():
    lines, seen = run_child(
        POPUP_CHILD,
        {"check": (110, 60)},
        mode="'on_ready'",
        ready_at=5000,
        close_parent_at=2000,
    )
    popup = _popup_surface(lines)
    assert popup, _context(lines)
    closing = _index(lines, "MARK close parent")
    closed = _index(lines, "MARK popup closed dismissed")
    assert closing is not None and closed is not None and closing < closed, _context(
        lines
    )
    # It never got a buffer: ready() came after it was gone.
    assert _first_attach(lines, popup) is None, _context(lines)
    assert seen["check"] != GREEN, seen
