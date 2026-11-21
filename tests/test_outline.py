"""0.6.11 (UXREQ-008): `outline_*` and `focus_outline_*`, a ring drawn outside the border box.
Headless: pixels come from `Window.snapshot`, keyboard focus from `simulate("key_down", key="tab")`.
The renderer's own geometry and damage proofs are `engine-render`'s `outline_paint.rs`."""

import pytest

from tess_re import Window
from helpers import add

BACKGROUND = (0x11, 0x11, 0x11, 255)
GREEN = (0, 255, 0, 255)
WHITE = (255, 255, 255, 255)
BLUE = (0, 0, 255, 255)
SECONDARY_LIGHT = (0x62, 0x5B, 0x71, 255)
SECONDARY_DARK = (0xCC, 0xC2, 0xDC, 255)


def window_with_box(**props):
    """A 120 by 120 window with a 40 by 40 green box at (40, 40), corner radius 8."""
    window = Window(width=120, height=120)
    window.root.set(padding=0, fill=BACKGROUND)
    box = add(
        window,
        "box",
        fill=GREEN,
        width=40,
        height=40,
        position="absolute",
        x=40,
        y=40,
        corner_radius=8,
        focusable=True,
        **props,
    )
    return window, box


def pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    start = (y * width + x) * 4
    return tuple(rgba[start : start + 4])


def near(actual, expected, tolerance=1):
    return all(abs(a - e) <= tolerance for a, e in zip(actual, expected))


def test_the_defaults_draw_nothing_and_read_back():
    window, box = window_with_box()
    assert box.get("outline_width") == 0
    assert box.get("outline_offset") == 0
    assert box.get("outline_color") is None
    assert box.get("outline_gap_color") is None
    assert box.get("outline_radius") is None
    assert box.get("focus_outline_width") == 0
    assert box.get("focus_outline_color") is None
    assert pixel(window, 36, 60) == BACKGROUND


def test_a_ring_is_drawn_outside_the_box_with_a_transparent_gap():
    window, box = window_with_box(outline_width=3, outline_offset=2, outline_color=WHITE)
    # The box spans 40..80: gap 38..40, ring 35..38, on all four sides.
    for gap, ring, outside, inside in [
        ((39, 60), (36, 60), (34, 60), (41, 60)),
        ((80, 60), (82, 60), (85, 60), (78, 60)),
        ((60, 39), (60, 36), (60, 34), (60, 41)),
        ((60, 80), (60, 82), (60, 85), (60, 78)),
    ]:
        assert near(pixel(window, *gap), BACKGROUND), f"gap {gap}"
        assert near(pixel(window, *ring), WHITE), f"ring {ring}"
        assert near(pixel(window, *outside), BACKGROUND), f"outside {outside}"
        assert near(pixel(window, *inside), GREEN), f"box {inside}"
    # Concentric corner: the box's radius 8 plus the offset 2 is 10; centre (48, 48).
    assert near(pixel(window, 35, 35), BACKGROUND), "the ring's outer corner is rounded"
    assert near(pixel(window, 39, 39), WHITE), "on the diagonal the ring is there"


def test_a_gap_color_fills_the_gap():
    window, _ = window_with_box(outline_width=3, outline_offset=2, outline_color=WHITE, outline_gap_color=BLUE)
    assert near(pixel(window, 38, 60), BLUE) and near(pixel(window, 39, 60), BLUE)
    assert near(pixel(window, 36, 60), WHITE)
    assert near(pixel(window, 34, 60), BACKGROUND)


def test_a_ring_with_no_colour_is_the_scheme_secondary():
    for scheme, expected in (("light", SECONDARY_LIGHT), ("dark", SECONDARY_DARK)):
        window, box = window_with_box(outline_width=3, outline_offset=2)
        window.set(scheme=scheme)
        assert near(pixel(window, 36, 60), expected), scheme
    # Setting the colour back to None goes back to the scheme's.
    window, box = window_with_box(outline_width=3, outline_offset=2, outline_color=WHITE)
    box.set(outline_color=None)
    assert near(pixel(window, 36, 60), SECONDARY_LIGHT)


def test_width_zero_draws_nothing_and_the_outline_follows_set():
    window, box = window_with_box(outline_width=3, outline_offset=2, outline_color=WHITE)
    assert near(pixel(window, 36, 60), WHITE)
    box.set(outline_width=0)
    assert pixel(window, 36, 60) == BACKGROUND
    box.set(outline_width=3, outline_offset=0)
    assert near(pixel(window, 38, 60), WHITE)


def test_an_inward_ring_stays_inside_the_box():
    window, box = window_with_box(outline_width=3, outline_offset=-3, outline_color=WHITE)
    assert near(pixel(window, 60, 41), WHITE)
    assert near(pixel(window, 60, 45), GREEN)
    assert near(pixel(window, 60, 39), BACKGROUND)


def test_the_radius_overrides_the_concentric_default():
    window, box = window_with_box(outline_width=3, outline_offset=0, outline_color=WHITE, outline_radius=0)
    assert box.get("outline_radius") == 0
    # A square inner edge: the box's corner is rounded (8), so the box leaves a corner of
    # background inside the ring's square hole, which the ring does not fill.
    assert near(pixel(window, 38, 38), WHITE) or near(pixel(window, 37, 37), WHITE)


def test_the_focus_ring_shows_for_keyboard_focus_only():
    window, box = window_with_box(
        focus_outline_width=3, focus_outline_offset=2, focus_outline_color=WHITE
    )
    assert pixel(window, 36, 60) == BACKGROUND, "no focus, no ring"

    window.simulate("key_down", key="tab")
    assert box.get("focused")
    assert near(pixel(window, 36, 60), WHITE), "Tab shows the ring in the same frame"
    assert near(pixel(window, 39, 60), BACKGROUND), "the gap stays clear"

    window.simulate("click", node=box)
    assert pixel(window, 36, 60) == BACKGROUND, "a pointer press hides it"

    window.simulate("key_down", key="tab")
    window.simulate("key_down", key="tab", shift=True)
    assert box.get("focused")
    assert near(pixel(window, 36, 60), WHITE), "the next key shows it again"

    window.simulate("unfocus", node=box)
    assert pixel(window, 36, 60) == BACKGROUND, "no focus, no ring"


def test_the_focus_style_replaces_the_plain_one_while_shown():
    window, box = window_with_box(
        outline_width=1,
        outline_offset=0,
        outline_color=BLUE,
        focus_outline_width=3,
        focus_outline_offset=2,
        focus_outline_color=WHITE,
    )
    assert near(pixel(window, 39, 60), BLUE), "the plain outline hugs the box"
    window.simulate("key_down", key="tab")
    assert near(pixel(window, 36, 60), WHITE)
    assert near(pixel(window, 39, 60), BACKGROUND), "and is replaced, not added to"


def test_a_focus_ring_without_a_colour_is_the_scheme_secondary():
    window, box = window_with_box(focus_outline_width=3, focus_outline_offset=2)
    window.simulate("key_down", key="tab")
    assert near(pixel(window, 36, 60), SECONDARY_LIGHT)


def test_every_kind_takes_the_properties_in_create():
    window = Window(width=120, height=120)
    for kind, extra in (("box", {}), ("text", {"text": "x"}), ("text_input", {}), ("scroll_view", {}), ("virtual_list", {"item_count": 3, "item_extent": 20, "materialize": lambda i: None})):
        node = window.create(kind, outline_width=2, outline_offset=1, **extra)
        assert node.get("outline_width") == 2
        assert node.get("outline_offset") == 1


@pytest.mark.parametrize(
    "props",
    [
        {"outline_width": -1},
        {"outline_width": True},
        {"outline_width": "3"},
        {"outline_offset": float("nan")},
        {"outline_offset": None},
        {"outline_color": "red"},
        {"outline_color": (1, 2, 3)},
        {"outline_gap_color": 5},
        {"outline_radius": -2},
        {"focus_outline_width": -1},
        {"focus_outline_color": "red"},
    ],
)
def test_bad_values_raise_and_change_nothing(props):
    window, box = window_with_box(outline_width=3, outline_color=WHITE)
    with pytest.raises(ValueError, match="node property `(focus_)?outline_"):
        box.set(**props)
    assert box.get("outline_width") == 3


def test_a_clipping_ancestor_cuts_the_ring_and_padding_keeps_it():
    window = Window(width=120, height=120)
    window.root.set(padding=0, fill=BACKGROUND)
    clip = add(
        window, "box", fill=(0, 0, 0, 0), width=120, height=120, position="absolute", x=38, y=0, clip_children=True
    )
    box = window.create(
        "box", fill=GREEN, width=40, height=40, position="absolute", x=2, y=40, outline_width=3, outline_offset=2, outline_color=WHITE
    )
    clip.add_child(box)
    # The clipping parent's left edge is at 38: the ring's left part (35..38) is cut, the top stays.
    assert near(pixel(window, 36, 60), BACKGROUND)
    assert near(pixel(window, 60, 36), WHITE)
