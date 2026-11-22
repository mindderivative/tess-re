"""0.6.12 (P9): a path's stroke style (`stroke_cap`, `stroke_join`, `stroke_miter_limit`,
`stroke_dash`, `stroke_dash_offset`), an image's `image_sampling`, and `shadows` on every node kind.
Headless: pixels come from `Window.snapshot`. The renderer's own proofs are `engine-render`'s
`paint_hooks.rs`."""

import pytest

from tess_re import Window
from helpers import add

BACKGROUND = (0x11, 0x11, 0x11, 255)
WHITE = (255, 255, 255, 255)
RED = (255, 0, 0, 255)
GREEN = (0, 255, 0, 255)


def pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    start = (y * width + x) * 4
    return tuple(rgba[start : start + 4])


def stroked(data="M20,50 L80,50", **props):
    window = Window(width=100, height=100)
    window.root.set(padding=0, fill=BACKGROUND)
    props.setdefault("stroke_width", 20)
    path = add(window, "path", data=data, width=100, height=100, position="absolute", x=0, y=0,
               stroke_color=WHITE, **props)
    return window, path


def test_the_defaults_are_what_the_stroke_always_was():
    _, path = stroked()
    assert path.get("stroke_cap") == "round"
    assert path.get("stroke_join") == "round"
    assert path.get("stroke_miter_limit") == 4
    assert path.get("stroke_dash") is None
    assert path.get("stroke_dash_offset") == 0


def test_caps_and_joins_change_the_pixels_and_set_none_restores_the_default():
    window, path = stroked(stroke_cap="butt")
    assert pixel(window, 17, 50) == BACKGROUND
    path.set(stroke_cap="square")
    assert pixel(window, 12, 50) == WHITE and pixel(window, 11, 41) == WHITE
    path.set(stroke_cap=None)
    assert path.get("stroke_cap") == "round"
    assert pixel(window, 11, 41) == BACKGROUND
    window, _ = stroked("M20,80 L20,20 L80,20", stroke_cap="butt", stroke_join="miter")
    assert pixel(window, 11, 11) == WHITE
    window, _ = stroked("M20,80 L20,20 L80,20", stroke_cap="butt", stroke_join="bevel")
    assert pixel(window, 11, 11) == BACKGROUND
    window, _ = stroked("M20,80 L20,20 L80,20", stroke_cap="butt", stroke_join="miter",
                        stroke_miter_limit=1.2)
    assert pixel(window, 11, 11) == BACKGROUND


def test_dashes_and_their_offset():
    window, path = stroked("M0,50 L100,50", stroke_width=10, stroke_cap="butt", stroke_dash=[20, 20])
    assert path.get("stroke_dash") == [20.0, 20.0]
    assert [pixel(window, x, 50) for x in (10, 30, 50)] == [WHITE, BACKGROUND, WHITE]
    path.set(stroke_dash_offset=20)
    assert [pixel(window, x, 50) for x in (10, 30)] == [BACKGROUND, WHITE]
    path.set(stroke_dash=None, stroke_dash_offset=None)
    assert path.get("stroke_dash") is None and path.get("stroke_dash_offset") == 0
    assert pixel(window, 30, 50) == WHITE
    path.set(stroke_dash=[])
    assert path.get("stroke_dash") is None


@pytest.mark.parametrize(
    "props",
    [
        {"stroke_cap": "flat"},
        {"stroke_cap": 3},
        {"stroke_join": "sharp"},
        {"stroke_miter_limit": 0.5},
        {"stroke_miter_limit": True},
        {"stroke_miter_limit": float("inf")},
        {"stroke_dash": [5, -1]},
        {"stroke_dash": [0, 0]},
        {"stroke_dash": "5 5"},
        {"stroke_dash": [True]},
        {"stroke_dash": [float("nan")]},
        {"stroke_dash_offset": "1"},
        {"stroke_dash_offset": float("nan")},
    ],
)
def test_bad_values_raise_and_change_nothing(props):
    _, path = stroked(stroke_cap="butt", stroke_dash=[4, 2])
    with pytest.raises(ValueError, match="node property `stroke_"):
        path.set(**props)
    assert path.get("stroke_cap") == "butt"
    assert path.get("stroke_dash") == [4.0, 2.0]


def test_stroke_style_applies_to_a_path_only():
    window = Window(width=50, height=50)
    box = window.create("box")
    with pytest.raises(ValueError, match="applies only to a path"):
        box.set(stroke_cap="butt")
    with pytest.raises(ValueError, match="applies only to a path"):
        box.get("stroke_dash")
    with pytest.raises(ValueError, match="applies only to a path"):
        window.create("box", stroke_join="bevel")


def test_a_painted_hit_shape_follows_the_stroke_style():
    window = Window(width=100, height=100)
    path = add(window, "path", data="M20,50 L80,50", width=100, height=100, position="absolute",
               x=0, y=0, stroke_color=WHITE, stroke_width=20, hit_shape="painted", stroke_cap="butt")
    clicks = []
    path.on("click", lambda e: clicks.append("path"))
    window.simulate("click", x=15, y=50)
    assert clicks == []
    path.set(stroke_cap="round")
    window.simulate("click", x=15, y=50)
    assert clicks == ["path"]


def test_set_defaults_declares_the_stroke_style_and_sampling():
    window = Window(width=50, height=50)
    window.set_defaults("path", stroke_cap="butt", stroke_dash=[3, 3])
    window.set_defaults("image", image_sampling="nearest")
    path = window.create("path", data="M0,0 L10,10")
    assert path.get("stroke_cap") == "butt" and path.get("stroke_dash") == [3.0, 3.0]
    assert window.create("path", data="M0,0 L1,1", stroke_cap="square").get("stroke_cap") == "square"
    image = window.create("image", rgba=b"\xff" * 16, pixel_width=2, pixel_height=2)
    assert image.get("image_sampling") == "nearest"
    with pytest.raises(ValueError, match="image_sampling"):
        window.set_defaults("image", image_sampling="sharp")


def red_green_image(window, **props):
    return add(window, "image", rgba=bytes([255, 0, 0, 255, 0, 255, 0, 255]), pixel_width=2,
               pixel_height=1, width=40, height=40, position="absolute", x=0, y=0, **props)


def test_image_sampling_nearest_keeps_hard_edges_and_linear_blends():
    window = Window(width=100, height=100)
    window.root.set(padding=0, fill=BACKGROUND)
    image = red_green_image(window)
    assert image.get("image_sampling") == "linear"
    mix = pixel(window, 20, 20)
    assert mix[0] > 40 and mix[1] > 40
    image.set(image_sampling="nearest")
    assert pixel(window, 18, 20) == RED and pixel(window, 22, 20) == GREEN
    image.set(image_sampling="cubic")
    assert image.get("image_sampling") == "cubic"
    image.set(image_sampling=None)
    assert image.get("image_sampling") == "linear"


@pytest.mark.parametrize("value", ["bilinear", 1, True])
def test_bad_sampling_raises(value):
    window = Window(width=50, height=50)
    image = red_green_image(window)
    with pytest.raises(ValueError, match="image_sampling"):
        image.set(image_sampling=value)
    with pytest.raises(ValueError, match="applies only to an image"):
        window.create("box").set(image_sampling="nearest")


SVG = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="white"/></svg>'


@pytest.mark.parametrize(
    "kind, props",
    [
        ("box", {"fill": WHITE}),
        ("circle", {"fill": WHITE, "corner_radius": 15}),
        ("ring", {"stroke_color": WHITE, "stroke_width": 4, "corner_radius": 15}),
        ("image", {"rgba": b"\xff" * 16, "pixel_width": 2, "pixel_height": 2}),
        ("svg", {"svg": SVG}),
        ("canvas", {"draw": lambda painter: None}),
        ("path", {"data": "M0,0 L30,0 L30,30 L0,30 Z", "fill": WHITE}),
        ("scroll_view", {}),
        ("text", {"text": "x"}),
        ("text_input", {}),
        ("virtual_list", {"item_count": 0, "item_extent": 10, "materialize": lambda i: None}),
        ("terminal", {"shell": "/bin/sh", "cols": 4, "rows": 2}),
    ],
)
def test_shadows_are_cast_by_every_kind(kind, props):
    window = Window(width=100, height=100)
    window.root.set(padding=0, fill=BACKGROUND)
    real = {"circle": "box", "ring": "box"}.get(kind, kind)
    node = add(window, real, width=30, height=30, position="absolute", x=10, y=30,
               shadows=[(RED, 50, 0, 0, 0)], **props)
    assert pixel(window, 75, 45) == RED, kind
    assert pixel(window, 75, 70) == BACKGROUND, kind
    if kind in ("circle", "ring"):
        assert pixel(window, 61, 31) == BACKGROUND, "a round node's shadow is round"
    node.set(shadows=[])
    assert pixel(window, 75, 45) == BACKGROUND
