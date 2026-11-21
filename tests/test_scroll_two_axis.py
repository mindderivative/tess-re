"""A scroll view of `orientation="both"` (0.6.11, UXREQ-001): two offsets, two thumbs drawn
only where an axis overflows, the wheel, keys, lane clicks, thumb drags and `scroll_into_view`
on each axis, the `scroll` event's `x` and `y`, and the scrollbar properties. Headless: the
Window's input simulation and `advance`."""

import pytest

from tess_re import Window
from helpers import add

BACKGROUND = (20, 20, 24, 255)


def both_view(window, content=(800, 400), size=(200, 100), parent=None):
    """A `size` view of `orientation="both"` over a `content` box that holds a focusable item."""
    view = window.create("scroll_view", orientation="both", width=size[0], height=size[1])
    box = window.create("box", width=content[0], height=content[1], flex_shrink=0)
    item = window.create("box", width=50, height=20, focusable=True)
    box.add_child(item)
    view.add_child(box)
    (parent or window.root).add_child(view)
    return view, box, item


def at(view):
    return view.get("scroll_offset_x"), view.get("scroll_offset")


def test_both_is_an_orientation_that_reads_back():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    assert view.get("orientation") == "both"
    view.set(orientation="vertical")
    assert view.get("orientation") == "vertical"
    view.set(orientation="both")
    assert view.get("orientation") == "both"
    with pytest.raises(ValueError, match="must be one of: vertical, horizontal, both"):
        view.set(orientation="diagonal")


def test_the_two_offsets_are_settable_readable_and_clamped():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    assert at(view) == (0.0, 0.0)
    view.set(scroll_offset=50, scroll_offset_x=70)
    assert at(view) == (70.0, 50.0)
    # 800 x 400 of content in a 200 x 100 view: 600 and 300 of travel.
    view.set(scroll_offset=5000, scroll_offset_x=5000)
    assert at(view) == (600.0, 300.0)
    view.set(scroll_offset_x=0)
    assert at(view) == (0.0, 300.0)
    # Given to `create`, before the content: clamped at the first layout.
    other = window.create("scroll_view", orientation="both", width=200, height=100, scroll_offset_x=99999)
    other.add_child(window.create("box", width=500, height=100, flex_shrink=0))
    window.root.add_child(other)
    window.advance(0)
    assert other.get("scroll_offset_x") == 300.0


def test_the_horizontal_offset_animates_and_eases_to_the_real_end():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    window.advance(0)
    view.animate("scroll_offset_x", 200.0, 100)
    window.advance(50)
    mid = view.get("scroll_offset_x")
    assert 0.0 < mid < 200.0
    window.advance(60)
    assert view.get("scroll_offset_x") == 200.0
    # A target past the end is the end: the animation eases all the way there.
    view.animate("scroll_offset_x", 99999.0, 100)
    window.advance(120)
    assert view.get("scroll_offset_x") == 600.0
    with pytest.raises(ValueError, match="applies only to a scroll_view"):
        add(window, "box").animate("scroll_offset_x", 10.0, 100)


def test_only_a_both_view_has_a_horizontal_range():
    window = Window(width=800, height=600)
    vertical = add(window, "scroll_view", width=100, height=100)
    vertical.add_child(window.create("box", width=500, height=500))
    vertical.set(scroll_offset_x=40)
    assert vertical.get("scroll_offset_x") == 0.0
    horizontal = add(window, "scroll_view", width=100, height=100, orientation="horizontal")
    horizontal.add_child(window.create("box", width=500, height=100, flex_shrink=0))
    horizontal.set(scroll_offset=30)
    assert (horizontal.get("scroll_offset"), horizontal.get("scroll_offset_x")) == (30.0, 0.0)
    box = add(window, "box")
    with pytest.raises(ValueError, match="applies only to a scroll_view"):
        box.set(scroll_offset_x=1)
    with pytest.raises(ValueError, match="applies only to a scroll_view"):
        box.get("scroll_offset_x")
    with pytest.raises(ValueError, match="non-negative"):
        vertical.set(scroll_offset_x=-1)


def test_the_wheel_moves_each_axis_by_a_notch_and_shift_moves_it_sideways():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    window.simulate("wheel", node=view, delta_y=40.0)
    assert at(view) == (0.0, 40.0)
    window.simulate("wheel", node=view, delta_x=40.0)
    assert at(view) == (40.0, 40.0)
    # Shift turns a plain wheel sideways.
    window.simulate("wheel", node=view, delta_y=25.0, shift=True)
    assert at(view) == (65.0, 40.0)
    window.simulate("wheel", node=view, delta_x=10.0, delta_y=-10.0)
    assert at(view) == (75.0, 30.0)


def test_a_wheel_a_both_view_cannot_use_scrolls_the_view_around_it():
    window = Window(width=800, height=600)
    outer, outer_box, _ = both_view(window, content=(500, 500))
    inner = window.create("scroll_view", orientation="both", width=100, height=50)
    inner.add_child(window.create("box", width=200, height=200, flex_shrink=0))
    outer_box.add_child(inner)
    window.simulate("wheel", node=inner, delta_y=40.0)
    assert (at(inner), at(outer)) == ((0.0, 40.0), (0.0, 0.0))
    inner.set(scroll_offset=150)  # its vertical end
    window.simulate("wheel", node=inner, x=20, y=20, delta_y=40.0)
    assert at(outer) == (0.0, 40.0), "it passed the vertical wheel on"
    window.simulate("wheel", node=inner, x=20, y=20, delta_x=40.0)
    assert (inner.get("scroll_offset_x"), outer.get("scroll_offset_x")) == (40.0, 0.0), (
        "but kept the sideways one"
    )


@pytest.mark.parametrize("modifier", ["ctrl", "alt", "meta"])
def test_a_shortcut_modifier_keeps_keys_from_scrolling_a_both_view(modifier):
    window = Window(width=800, height=600)
    view, _, item = both_view(window)
    item.focus()
    window.simulate("key_down", key="arrow_right", **{modifier: True})
    window.simulate("key_down", key="page_down", **{modifier: True})
    assert at(view) == (0.0, 0.0)


def test_keys_scroll_each_axis_by_the_spec_table():
    window = Window(width=800, height=1000)
    view, _, item = both_view(window, content=(800, 1000))
    item.focus()
    for key, shift, expected in [
        ("arrow_down", False, (0.0, 40.0)),
        ("arrow_right", False, (40.0, 40.0)),
        ("page_down", False, (40.0, 130.0)),  # the viewport less 10 percent: 90
        ("arrow_left", False, (0.0, 130.0)),
        ("arrow_up", False, (0.0, 90.0)),
        ("page_down", True, (180.0, 90.0)),  # with Shift, along: 90 percent of 200
        ("page_up", True, (0.0, 90.0)),
        ("end", False, (0.0, 900.0)),
        ("page_down", False, (0.0, 900.0)),  # at the end
        ("home", False, (0.0, 0.0)),
    ]:
        window.simulate("key_down", key=key, shift=shift)
        assert at(view) == expected, (key, shift)


def test_a_vertical_view_still_pages_by_a_whole_viewport_and_ignores_sideways_keys():
    window = Window(width=800, height=600)
    view = add(window, "scroll_view", width=200, height=100)
    box = window.create("box", width=200, height=1000, flex_shrink=0)
    item = window.create("box", width=50, height=20, focusable=True)
    box.add_child(item)
    view.add_child(box)
    item.focus()
    window.simulate("key_down", key="page_down")
    assert view.get("scroll_offset") == 100.0
    window.simulate("key_down", key="arrow_right")
    assert (view.get("scroll_offset"), view.get("scroll_offset_x")) == (100.0, 0.0)


def test_scroll_into_view_moves_each_axis_by_the_least_that_shows_the_node():
    window = Window(width=800, height=600)
    view, box, _ = both_view(window)
    far = window.create("box", position="absolute", x=500, y=250, width=50, height=20, focusable=True)
    box.add_child(far)
    far.scroll_into_view()
    assert at(view) == (362.0, 182.0), "its far edges at the viewport, short of the 12 px gutter"
    view.set(scroll_offset_x=600, scroll_offset=300)
    far.focus()
    assert at(view) == (500.0, 250.0), "its near edges at the viewport's"


def test_the_scroll_event_reports_both_offsets():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    seen = []
    view.on("scroll", lambda e: seen.append((e.x, e.y, e.old_value, e.new_value)))
    window.simulate("wheel", node=view, delta_x=40.0)
    assert seen == [(40.0, 0.0, 0.0, 0.0)], "only x moved: the main axis' old and new match"
    window.simulate("wheel", node=view, delta_y=40.0)
    assert seen[-1] == (40.0, 40.0, 0.0, 40.0)
    view.animate("scroll_offset_x", 100.0, 100)
    window.advance(0)
    window.advance(100)
    assert seen[-1][:2] == (100.0, 40.0)
    # A one-axis view reports x and y too.
    plain = add(window, "scroll_view", width=100, height=100)
    plain.add_child(window.create("box", width=100, height=500))
    plain_seen = []
    plain.on("scroll", lambda e: plain_seen.append((e.x, e.y, e.new_value)))
    window.simulate("wheel", node=plain, delta_y=30.0)
    assert plain_seen == [(0.0, 30.0, 30.0)]


# The 200 x 100 view's vertical thumb is at x 190..198, y 2..34, its horizontal one at y 90..98,
# x 2..48: both centred 6 px inside the view's edges.
VERTICAL = {"x": 194, "y": 18}
HORIZONTAL = {"x": 20, "y": 94}


def test_dragging_a_thumb_moves_its_axis_linearly_and_on_outside_the_strip():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    window.simulate("pointer_down", node=view, **VERTICAL)
    window.simulate("pointer_move", node=view, x=194, y=18 + 42)
    # d * (content - viewport) / (lane - thumb) = 42 * 300 / 52
    assert at(view)[1] == pytest.approx(42 * 300 / 52, abs=1.0)
    window.simulate("pointer_move", node=view, x=-300, y=18 + 20)
    assert at(view)[1] == pytest.approx(20 * 300 / 52, abs=1.0)
    window.simulate("pointer_up", node=view, x=-300, y=38)
    window.simulate("pointer_move", node=view, x=194, y=90)
    assert at(view)[1] == pytest.approx(20 * 300 / 52, abs=1.0), "released: no longer following"
    assert at(view)[0] == 0.0

    window.simulate("pointer_down", node=view, **HORIZONTAL)
    window.simulate("pointer_move", node=view, x=20 + 46, y=400)
    assert at(view)[0] == pytest.approx(46 * 600 / 138, abs=1.0)
    window.simulate("pointer_up", node=view, x=66, y=400)


def test_the_hit_strip_is_12_wide_centred_on_the_thumb():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    # 6 from the thumb's centre is inside the strip; 8 is not.
    window.simulate("pointer_down", node=view, x=194 - 6, y=18)
    window.simulate("pointer_move", node=view, x=188, y=18 + 20)
    assert at(view)[1] > 0.0
    window.simulate("pointer_up", node=view, x=188, y=38)
    view.set(scroll_offset=0)
    window.simulate("pointer_down", node=view, x=194 - 8, y=18)
    window.simulate("pointer_move", node=view, x=186, y=18 + 20)
    assert at(view)[1] == 0.0
    window.simulate("pointer_up", node=view, x=186, y=38)


def test_a_press_on_the_lane_pages_and_repeats_while_held():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window, content=(200, 1000), size=(100, 100))
    window.advance(0)
    # Only the vertical axis overflows: the lane is the whole 96 and the thumb is 32 long (y 2..34).
    window.simulate("pointer_down", node=view, x=94, y=60)
    assert at(view) == (0.0, 90.0), "a page: the viewport less 10 percent, at once"
    window.advance(399)
    assert at(view)[1] == 90.0
    window.advance(1)
    assert at(view)[1] == 180.0, "400 ms later"
    window.advance(100)
    assert at(view)[1] == 270.0
    window.advance(300)
    assert at(view)[1] == 450.0, "the thumb reached the press and it stopped"
    window.advance(1000)
    assert at(view)[1] == 450.0
    window.simulate("pointer_up", node=view, x=94, y=60)


def test_releasing_stops_the_paging_and_a_press_above_the_thumb_pages_back():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window, content=(200, 1000), size=(100, 100))
    window.advance(0)
    view.set(scroll_offset=900)
    window.simulate("pointer_down", node=view, x=94, y=10)
    assert view.get("scroll_offset") == 810.0
    window.simulate("pointer_up", node=view, x=94, y=10)
    window.advance(2000)
    assert view.get("scroll_offset") == 810.0
    # A horizontal lane pages sideways.
    wide, _, _ = both_view(window, content=(1000, 100), size=(100, 100))
    window.advance(0)
    window.simulate("pointer_down", node=wide, x=80, y=94)
    assert at(wide) == (90.0, 0.0)
    window.simulate("pointer_up", node=wide, x=80, y=94)


def test_a_press_on_the_lane_does_not_click_the_content_under_it():
    window = Window(width=800, height=600)
    view, box, _ = both_view(window, content=(200, 1000), size=(100, 100))
    clicks = []
    box.on("click", lambda: clicks.append(True))
    window.simulate("click", node=view, x=94, y=60)
    assert clicks == []
    window.simulate("click", node=view, x=40, y=60)
    assert clicks == [True]


SCROLLBAR = [
    "scrollbar_width",
    "scrollbar_hit_width",
    "scrollbar_margin",
    "scrollbar_min_length",
    "scrollbar_gutter",
    "scrollbar_corner",
    "scrollbar_opacity",
    "scrollbar_opacity_hover",
    "scrollbar_opacity_drag",
]


def test_the_scrollbar_properties_default_to_the_spec_on_a_both_view_and_to_the_old_look_otherwise():
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    spec = [8.0, 12.0, 2.0, 32.0, 12.0, 12.0, 0.7, 0.8, 1.0]
    assert [view.get(n) for n in SCROLLBAR] == spec
    one = add(window, "scroll_view", width=100, height=100)
    assert [one.get(n) for n in SCROLLBAR] == [4.0, 16.0, 2.0, 32.0, 0.0, 0.0, 1.0, 1.0, 1.0]
    view.set(scrollbar_width=6, scrollbar_opacity_hover=0.5, scrollbar_gutter=0)
    assert (view.get("scrollbar_width"), view.get("scrollbar_opacity_hover"), view.get("scrollbar_gutter")) == (
        6.0,
        0.5,
        0.0,
    )
    view.set(scrollbar_width=None, scrollbar_opacity_hover=None, scrollbar_gutter=None)
    assert [view.get(n) for n in SCROLLBAR] == spec
    # The default follows the orientation until a value is set.
    view.set(orientation="vertical")
    assert view.get("scrollbar_width") == 4.0


@pytest.mark.parametrize(
    ("props", "message"),
    [
        ({"scrollbar_width": -1}, "a non-negative number"),
        ({"scrollbar_hit_width": "wide"}, "a non-negative number"),
        ({"scrollbar_opacity": 1.5}, "a number from 0 to 1"),
        ({"scrollbar_opacity_drag": -0.1}, "a number from 0 to 1"),
        ({"scrollbar_gutter": float("inf")}, "a non-negative number"),
    ],
)
def test_bad_scrollbar_values_are_rejected(props, message):
    window = Window(width=800, height=600)
    view, _, _ = both_view(window)
    with pytest.raises(ValueError, match=message):
        view.set(**props)


@pytest.mark.parametrize("name", ["scrollbar_hit_width", "scrollbar_corner", "scrollbar_opacity"])
def test_a_scrollbar_property_on_another_node_raises(name):
    window = Window(width=800, height=600)
    box = add(window, "box")
    with pytest.raises(ValueError, match="applies only to a scroll_view or virtual_list node"):
        box.set(**{name: 1})
    with pytest.raises(ValueError, match="applies only to a scroll_view or virtual_list node"):
        box.get(name)


def pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    start = (y * width + x) * 4
    return tuple(rgba[start : start + 4])


def test_two_thumbs_are_drawn_where_the_axes_overflow_and_leave_the_corner_empty():
    # The dark scheme: `on_surface_variant` is #CAC4D0 there (the light scheme's is #49454F).
    window = Window(width=300, height=200, scheme="dark")
    window.root.set(padding=0)
    view, _, _ = both_view(window)
    view.set(fill=BACKGROUND)
    # At rest: 0.70 of #CAC4D0 over the view's colour.
    over = tuple(round(c * 0.7 + b * 0.3) for c, b in zip((0xCA, 0xC4, 0xD0), BACKGROUND))
    for where, name in [((194, 18), "vertical"), ((20, 94), "horizontal")]:
        got = pixel(window, *where)
        assert all(abs(g - w) <= 2 for g, w in zip(got, over)), (name, got, over)
    # At the far ends the thumbs stop at the lane ends, 12 px short of the corner.
    view.set(scroll_offset=300, scroll_offset_x=600)
    assert all(abs(g - w) <= 2 for g, w in zip(pixel(window, 194, 80), over)), "vertical thumb's end"
    assert pixel(window, 194, 88) == BACKGROUND, "below its lane"
    assert all(abs(g - w) <= 2 for g, w in zip(pixel(window, 180, 94), over)), "horizontal thumb's end"
    assert pixel(window, 188, 94) == BACKGROUND, "right of its lane"
    for x in range(188, 200):
        for y in range(88, 100):
            assert pixel(window, x, y) == BACKGROUND, (x, y)


def test_an_axis_that_fits_shows_no_thumb():
    window = Window(width=300, height=200)
    window.root.set(padding=0)
    view, _, _ = both_view(window, content=(200, 400))  # only the vertical axis overflows
    view.set(fill=BACKGROUND)
    assert pixel(window, 194, 18) != BACKGROUND, "vertical thumb"
    for x in range(2, 190):
        assert pixel(window, x, 94) == BACKGROUND, "no horizontal thumb"
