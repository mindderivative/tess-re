"""0.6.12 (P10): `PopupWindow(edge_margin=...)` keeps the body clear of the output's edge.

The compositor flips and slides the window geometry until it is on the output. `edge_margin` grows that
geometry by the margin (inside the popup's transparent shadow room) and moves the anchor rectangle so the
body still attaches where it would without one. Read from a screenshot (`tests/live_compositor.py`), as in
`test_popup_geometry_live.py`, whose child program this reuses.

Needs a layer-shell compositor: a private sway (`tools/sway_live.sh`, with `grim`) or a private KWin
(`tools/kwin_live.sh`, the `[kwin]` variants, with `jeepney`); skips without them.
"""

import json
import subprocess
import sys
import textwrap

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


def output_height():
    return live_compositor.output_rect()[3]


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
        shot = live_compositor.screenshot()
    finally:
        child.wait(timeout=20)
    box = shot.find((255, 0, 0))
    assert box, "the popup's red body is not on screen"
    return info, box


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
