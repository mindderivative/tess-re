"""0.6.12 (P10): `PopupWindow(edge_margin=...)` keeps the body clear of the output's edge.

The compositor flips and slides the window geometry until it is on the output. `edge_margin` grows that
geometry by the margin (inside the popup's transparent shadow room) and moves the anchor rectangle so the
body still attaches where it would without one. Read from a screenshot (grim, headless sway), as in
`test_popup_geometry_live.py`, whose child program this reuses.

Needs a layer-shell compositor with `grim` and a private sway (`tools/sway_live.sh`); skips without them.
"""

import json
import os
import shutil
import subprocess
import sys
import textwrap

import pytest

import tess_re

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux")
    or not tess_re.supports("layer_shell")
    or not shutil.which("grim")
    or not os.environ.get("SWAYSOCK"),
    reason="needs a private sway (tools/sway_live.sh) with grim",
)

MARGIN = 20
EDGE = 8
CHILD = """
import json
import tess_re

M = %(margin)d
GEOMETRY = %(geometry)s
app = tess_re.App()
bar = tess_re.LayerWindow("tre-test-bar", layer="overlay", anchor=("top", "left"), size=(400, 32),
                          margin=(%(top)d, 0, 0, 40), keyboard="none", exclusive_zone=0)
bar.root.set(fill=(30, 30, 40, 255))
settings = dict(size=(120 + 2 * M, 60 + 2 * M), anchor_rect=(100, 0, 60, 32),
                anchor="bottom_left", gravity="bottom_right", grab=False, constraint_adjustment=%(adjust)s)
if GEOMETRY:
    settings["geometry"] = (M, M, 120, 60)
settings["edge_margin"] = %(edge)s
popup = tess_re.PopupWindow(bar, **settings)
body = popup.create("box", width=120, height=60, position="absolute", x=M, y=M, fill=(255, 0, 0, 255))
popup.root.add_child(body)
placed = []
popup.on("configure", lambda e: placed.append((e.x, e.y)))
app.add_window(bar)
bar.after(400, lambda: app.add_window(popup))
handle = app.thread_handle()


def report():
    print(json.dumps({"placed": placed[-1] if placed else None, "geometry": popup.get("geometry"),
                      "edge": popup.get("edge_margin"), "applied": popup.get("edge_margin_applied")}),
          flush=True)


bar.after(2500, report)
bar.after(5500, handle.stop)
app.run(keep_alive=True)
"""


WIDTH = 400


def output_height():
    outputs = json.loads(subprocess.run(["swaymsg", "-t", "get_outputs"], capture_output=True, check=True).stdout)
    return outputs[0]["rect"]["height"]


def run(geometry=True, top=50, adjust="()", edge=EDGE):
    height = output_height()
    code = textwrap.dedent(CHILD) % {
        "margin": MARGIN,
        "geometry": "True" if geometry else "False",
        "edge": edge,
        "top": top if top >= 0 else height + top,
        "adjust": adjust,
    }
    child = subprocess.Popen(
        [sys.executable, "-c", code], stdout=subprocess.PIPE, text=True, stderr=subprocess.DEVNULL
    )
    assert child.stdout is not None
    try:
        line = child.stdout.readline()
        assert line, "the child printed nothing"
        info = json.loads(line)
        region = f"0,0 {WIDTH}x{height}"
        shot = subprocess.run(
            ["grim", "-g", region, "-t", "ppm", "-"], capture_output=True, timeout=20, check=False
        ).stdout
    finally:
        child.wait(timeout=20)
    header, rest = shot.split(b"\n255\n", 1)
    width, height = (int(v) for v in header.split()[1:3])
    xs, ys = [], []
    for y in range(height):
        row = rest[y * width * 3 : (y + 1) * width * 3]
        for x in range(width):
            if row[x * 3 : x * 3 + 3] == b"\xff\x00\x00":
                xs.append(x)
                ys.append(y)
    assert xs, "the popup's red body is not on screen"
    return info, (min(xs), min(ys), max(xs) + 1, max(ys) + 1)


# The bar is at (40, 50) on the output; the anchor point is the bottom-left of the rectangle (100, 0, 60, 32)
# in the bar's own coordinates: (140, 82) on screen.
ANCHOR = (140, 82)


def test_the_body_still_lands_on_the_anchor_point_with_an_edge_margin():
    info, (left, top, right, bottom) = run()
    assert (left, top) == ANCHOR, f"the body's corner is at {(left, top)}"
    assert (right - left, bottom - top) == (120, 60)
    assert info["edge"] == [EDGE] * 4 and info["applied"] == [EDGE] * 4
    assert tuple(info["placed"]) == (100, 32), "configure reports the body's position, not the grown geometry's"


def test_the_margin_the_surface_has_room_for_is_what_is_applied():
    info, (left, top, _, _) = run(edge="(30, 30, 4, 4)")
    assert info["edge"] == [30, 30, 4, 4]
    assert info["applied"] == [MARGIN, MARGIN, 4, 4], "the shadow margin is all the room there is"
    assert (left, top) == ANCHOR


def test_without_an_edge_margin_the_body_stays_below_with_two_pixels_to_spare():
    # The anchor point is 94 px above the bottom edge: the 60 px body fits below it with 2 px to spare.
    anchor_y = output_height() - 62
    _, (_, top, _, _) = run(top=-94, adjust='("flip_y",)', edge=0)
    assert top == anchor_y


def test_the_margin_flips_the_popup_before_it_touches_the_edge():
    height = output_height()
    bar_top = height - 94
    _, (left, top, _, bottom) = run(top=-94, adjust='("flip_y",)')
    # 2 px of room is under the 8 px margin, so the compositor flips: the body's bottom edge is on the
    # rectangle's top, and its left edge still on the rectangle's left.
    assert bottom == bar_top, f"the body's bottom is at {bottom}, the bar's top at {bar_top}"
    assert left == ANCHOR[0]
    assert top == bar_top - 60


def test_a_margin_of_the_right_size_keeps_the_popup_below_when_there_is_room():
    # 2 px to spare against a margin of 2: it stays.
    anchor_y = output_height() - 62
    _, (_, top, _, _) = run(top=-94, adjust='("flip_y",)', edge=2)
    assert top == anchor_y
