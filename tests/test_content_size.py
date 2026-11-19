"""TRE-REQ-022: a `LayerWindow` whose `size` is `"content"` on an axis is the size of what is in it.

`size=("content", "content")` (or one axis: `(480, "content")`) makes the surface the root node's outer size,
padding included, and keeps it that way as the content changes: the new size is asked of the compositor once
per change, and the frame that shows the new content is held until the compositor has answered, so the new
content is never drawn at the old size. `min_size` and `max_size` clamp it.

The argument handling is tested headless. What the compositor is asked and what is drawn is tested on a
private sway: the child runs with `WAYLAND_DEBUG=1`, and the test reads the protocol trace and a series of
screenshots taken while rows are added. Needs `grim`; runs under `tools/sway_live.sh`.
"""

import os
import re
import shutil
import subprocess
import sys
import textwrap
import threading

import pytest

from tess_re import LayerWindow, UnsupportedPlatform

PAD, ROW = 16, 24


def make(**kw):
    try:
        return LayerWindow("content-size-test", **kw)
    except UnsupportedPlatform:
        pytest.skip("LayerWindow is not available here")


# ---- the arguments, headless -----------------------------------------------------------------------


def test_size_keeps_the_word_content_and_the_pixels_of_a_fixed_axis():
    window = make(size=("content", 300), min_size=(100, None), max_size=(480, 400))
    assert window.get("size") == ("content", 300)
    assert window.get("min_size") == (100, None)
    assert window.get("max_size") == (480, 400)
    stretched = make(size=(0, 30), anchor=("top", "left", "right"))
    assert stretched.get("size") == (0, 30), "0 still stretches between opposite anchors"
    assert make(size=(200, 30)).get("min_size") is None


def test_the_size_can_change_to_content_and_back_while_the_window_lives():
    window = make(size=(200, 30))
    window.set(size=("content", "content"))
    assert window.get("size") == ("content", "content")
    window.set(size=(150, 40))
    assert window.get("size") == (150, 40)
    window.set(max_size=(300, 300))
    assert window.get("max_size") == (300, 300)
    window.set(max_size=None)
    assert window.get("max_size") is None


@pytest.mark.parametrize(
    "bad",
    [("content",), ("fit", 10), ("content", "content", 1), (-1, 5), "content", (1.5, 2)],
)
def test_a_size_that_is_not_pixels_or_content_is_refused(bad):
    with pytest.raises(ValueError, match="LayerWindow size"):
        make(size=bad)


def test_limits_are_whole_pixels_above_zero_and_ordered():
    with pytest.raises(ValueError, match="min_size"):
        make(size=("content", "content"), min_size=(0, 10))
    with pytest.raises(ValueError, match="max_size"):
        make(size=("content", "content"), max_size=(10,))
    with pytest.raises(ValueError, match="minimum 300 is above the maximum 200"):
        make(size=("content", "content"), min_size=(300, None), max_size=(200, None))
    window = make(size=("content", "content"))
    with pytest.raises(ValueError):
        window.set(min_size=(50, 50), max_size=(40, 60))
    assert window.get("min_size") is None, "a refused set changes nothing"


def test_a_content_axis_is_the_root_with_its_padding_and_a_fixed_one_is_as_given():
    window = make(size=("content", "content"))
    window.root.set(padding=16)
    box = window.create("box", width=200, height=80)
    window.root.add_child(box)
    assert (window.root.get("layout_width"), window.root.get("layout_height")) == (232, 112)
    mixed = make(size=(480, "content"))
    mixed.root.set(padding=16)
    inner = mixed.create("box", width=200, height=80)
    mixed.root.add_child(inner)
    assert mixed.root.get("layout_height") == 112


# ---- the compositor, on a private sway -------------------------------------------------------------

CHILD = """
import json, os, threading, time
from tess_re import App, LayerWindow

app = App()
bar = LayerWindow("content-size", **%(kwargs)s)
bar.root.set(fill=(30, 60, 120, 255), padding=16, flex_direction="vertical")
rows = []

def add_row():
    row = bar.create("box", width=200, height=24, fill=(240, 180, 60, 255))
    bar.root.add_child(row)
    rows.append(row)

for _ in range(3):
    add_row()
bar.on("configure", lambda e: print(json.dumps({"kind": "configure", "w": e.width, "h": e.height}), flush=True))
app.add_window(bar)
handle = app.thread_handle()

def go():
    time.sleep(2.5)
    print(json.dumps({"kind": "settled"}), flush=True)
    for _ in range(4):
        handle.call_soon(add_row)
        time.sleep(0.8)
    time.sleep(2.5)
    handle.call_soon(finish)  # a window is only read on the loop's thread

def finish():
    print(json.dumps({"kind": "done", "size": %(read)s}), flush=True)
    os._exit(0)

threading.Thread(target=go, daemon=True).start()
app.run()
"""


class Shots:
    """Screenshots taken continuously while the child runs: `(width, height, rgb bytes)` each."""

    def __init__(self):
        self.frames = []
        self._stop = False
        self._thread = threading.Thread(target=self._run, daemon=True)

    def _run(self):
        while not self._stop:
            out = subprocess.run(["grim", "-t", "ppm", "-"], capture_output=True, timeout=20, check=False).stdout
            try:
                header, data = out.split(b"\n255\n", 1)
                width, height = (int(v) for v in header.split()[1:3])
            except ValueError:
                continue
            if len(data) == width * height * 3:
                self.frames.append((width, height, data))

    def start(self):
        self._thread.start()

    def stop(self):
        self._stop = True
        self._thread.join(timeout=15)


def _aligned(row, pattern, last):
    """The pixel index of the first (or last) whole pixel of `row` equal to `pattern`, or -1."""
    at = len(row) if last else -1
    while True:
        at = row.rfind(pattern, 0, at + 2) if last else row.find(pattern, at + 1)
        if at < 0 or at % 3 == 0:
            return at // 3 if at >= 0 else -1
        if last:
            at -= 1


def boxes(frame):
    """`(blue_box, orange_box)` of a screenshot: `(x0, y0, x1, y1)` inclusive, or `None` if absent."""
    width, height, data = frame
    found: dict[str, tuple[int, int, int, int] | None] = {}
    for name, rgb in (("blue", (30, 60, 120)), ("orange", (240, 180, 60))):
        pattern = bytes(rgb)
        box = None
        for y in range(height):
            row = data[y * width * 3 : (y + 1) * width * 3]
            left = _aligned(row, pattern, False)
            if left < 0:
                continue
            right = _aligned(row, pattern, True)
            box = (left, y, right, y) if box is None else (min(box[0], left), box[1], max(box[2], right), y)
        found[name] = box
    return found["blue"], found["orange"]


def run_child(kwargs, read):
    if not os.environ.get("WAYLAND_DISPLAY"):
        pytest.skip("needs a Wayland compositor with the layer shell (tools/sway_live.sh)")
    if not shutil.which("grim"):
        pytest.skip("needs grim to read pixels")
    script = textwrap.dedent(CHILD) % {"kwargs": kwargs, "read": read}
    shots = Shots()
    shots.start()
    try:
        done = subprocess.run(
            [sys.executable, "-c", script],
            capture_output=True,
            text=True,
            timeout=90,
            check=False,
            env=dict(os.environ, WAYLAND_DEBUG="1"),
        )
    finally:
        shots.stop()
    surface = re.search(r'zwlr_layer_surface_v1[#@](\d+), wl_surface[#@](\d+), [^"]*"content-size"\)', done.stderr)
    assert surface, done.stderr[-1500:]
    return done.stderr, surface.group(1), surface.group(2), shots.frames, done.stdout


def sizes_asked(trace, layer):
    return [
        (int(w), int(h)) for w, h in re.findall(rf"zwlr_layer_surface_v1[#@]{layer}\.set_size\((\d+), (\d+)\)", trace)
    ]


def tall(rows):
    return rows * ROW + 2 * PAD


def test_a_content_surface_follows_its_content_with_one_request_per_change_and_no_loop():
    trace, layer, _, _, _ = run_child('dict(anchor=("bottom",), size=("content", "content"))', "bar.get('size')")
    asked = sizes_asked(trace, layer)
    wide = 200 + 2 * PAD
    # The placeholder, then each content size once, and nothing more over the idle seconds after the last.
    assert asked == [(1, 1)] + [(wide, tall(n)) for n in (3, 4, 5, 6, 7)], asked


def test_padding_is_inside_the_size_and_the_new_content_is_never_drawn_at_the_old_size():
    trace, layer, surface, frames, _ = run_child(
        'dict(anchor=("bottom",), size=("content", "content"))', "bar.get('size')"
    )
    # Protocol: a buffer of a new size is not attached before the compositor has answered the request.
    sizes, requested, configured, early = {}, None, None, []
    for line in trace.splitlines():
        if made := re.search(r"create_buffer\(new id wl_buffer[#@](\d+), \d+, (\d+), (\d+),", line):
            sizes[made.group(1)] = (int(made.group(2)), int(made.group(3)))
        elif ask := re.search(rf"zwlr_layer_surface_v1[#@]{layer}\.set_size\((\d+), (\d+)\)", line):
            requested = (int(ask.group(1)), int(ask.group(2)))
        elif got := re.search(rf"zwlr_layer_surface_v1[#@]{layer}\.configure\(\d+, (\d+), (\d+)\)", line):
            configured = (int(got.group(1)), int(got.group(2)))
        elif att := re.search(rf"wl_surface[#@]{surface}\.attach\(wl_buffer[#@](\d+),", line):
            shape = sizes.get(att.group(1))
            if shape and requested and shape == requested and configured != requested:
                early.append(line.strip())
    assert not early, early
    # Pixels: every screenshot that shows the surface shows a whole frame, the content inside its padding.
    shown = 0
    for frame in frames:
        blue, orange = boxes(frame)
        if not blue or not orange:
            continue
        shown += 1
        assert (orange[0] - blue[0], blue[2] - orange[2]) == (PAD, PAD), (blue, orange)
        assert (orange[1] - blue[1], blue[3] - orange[3]) == (PAD, PAD), (blue, orange)
        assert (orange[3] - orange[1] + 1) % ROW == 0, (blue, orange)
        # The compositor lays the surface out at the new size on the commit that asks for it, one frame
        # before the new buffer arrives, so the old content can sit one step high for that frame.
        assert frame[1] - 1 - blue[3] <= ROW, "a bottom-anchored surface keeps its bottom edge as it grows"
    assert shown >= 5, f"only {shown} of {len(frames)} screenshots showed the surface"
    last = next(f for f in reversed(frames) if all(boxes(f)))
    assert boxes(last)[0][3] == last[1] - 1, "settled, the bottom edge is exactly at the bottom"


def test_max_size_clamps_and_the_fixed_axis_keeps_its_width():
    trace, layer, _, _, _ = run_child(
        'dict(anchor=("bottom",), size=(300, "content"), max_size=(None, 120))', "bar.get('size')"
    )
    asked = sizes_asked(trace, layer)
    assert asked[0] == (300, 1), asked
    assert all(w == 300 for w, _ in asked), asked
    assert asked[-1] == (300, 120), asked
    assert asked.count((300, 120)) == 1, "once clamped, more rows ask for nothing more"


def test_min_size_is_a_floor_for_the_content():
    trace, layer, _, _, _ = run_child(
        'dict(anchor=("bottom",), size=("content", "content"), min_size=(300, 200))', "bar.get('size')"
    )
    asked = sizes_asked(trace, layer)
    assert asked[0] == (300, 200), asked  # the floor, until the first layout
    assert all(w == 300 and h >= 200 for w, h in asked), asked
    assert asked[-1] == (300, tall(7)), asked  # the content outgrows the floor on its own


def test_an_unanchored_surface_stays_centred_as_it_grows():
    _, _, _, frames, _ = run_child('dict(size=("content", "content"))', "bar.get('size')")
    seen = 0
    for width, height, data in frames:
        blue, orange = boxes((width, height, data))
        if not blue or not orange:
            continue
        seen += 1
        assert abs((blue[0] + blue[2]) / 2 - (width - 1) / 2) <= 1, blue
        # For the one frame between the compositor laying the surface out at the new size and the new
        # buffer arriving, the old content sits half a step off centre; settled it is exact.
        assert abs((blue[1] + blue[3]) / 2 - (height - 1) / 2) <= ROW / 2 + 1, blue
    assert seen >= 5
    width, height, data = next(f for f in reversed(frames) if all(boxes(f)))
    blue, _ = boxes((width, height, data))
    assert abs((blue[1] + blue[3]) / 2 - (height - 1) / 2) <= 1, "settled, it is centred"
