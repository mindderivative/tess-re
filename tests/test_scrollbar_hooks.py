"""Every aspect of a scrollbar and of the scroll feel is a property of a scroll view and of a
virtual list (0.6.11, P1): the radius, a track, per-state colours, the side of each bar, when it
shows, the fades, arrows, a lane click that jumps, the wheel notch, the key line, the page, smooth
scrolling and the page repeat. Unset is the Tesserae UX spec (the legacy bar for a one-axis view and
a list); `reduced_motion` on the window collapses the motion. Headless."""

import pytest

from tess_re import Window
from helpers import add

BACKGROUND = (20, 20, 24, 255)
THUMB = (0xCA, 0xC4, 0xD0)
OVER = tuple(round(c * 0.7 + b * 0.3) for c, b in zip(THUMB, BACKGROUND))


def both_view(window, content=(800, 400), size=(200, 100), **props):
    view = window.create("scroll_view", orientation="both", width=size[0], height=size[1], **props)
    view.add_child(window.create("box", width=content[0], height=content[1], flex_shrink=0))
    window.root.add_child(view)
    view.set(fill=BACKGROUND)
    return view


def make_list(window, count=20, **props):
    lst = window.create(
        "virtual_list",
        item_count=count,
        item_extent=20,
        materialize=lambda i: window.create("box", width=100, height=20),
        width=200,
        height=100,
        flex_shrink=0,
        **props,
    )
    window.root.add_child(lst)
    lst.set(fill=BACKGROUND)
    window.advance(16)
    return lst


def new_window(width=300, height=200):
    window = Window(width=width, height=height)
    window.root.set(padding=0)
    return window


def at(view):
    return view.get("scroll_offset_x") if view.get("orientation") == "both" else 0.0, view.get("scroll_offset")


def pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    start = (y * width + x) * 4
    return tuple(rgba[start : start + 4])


def close(got, want, tolerance=2):
    return all(abs(g - w) <= tolerance for g, w in zip(got, want))


# The defaults: the spec for a `both` view, the legacy bar for a one-axis view and for a list.
DEFAULTS = {
    "scrollbar_radius": (4.0, 2.0, 2.0),
    "scrollbar_track_fill": (None, None, None),
    "scrollbar_track_opacity": (1.0, 1.0, 1.0),
    "scrollbar_fill_hover": (None, None, None),
    "scrollbar_fill_drag": (None, None, None),
    "scrollbar_side_v": ("end", "end", "end"),
    "scrollbar_side_h": ("end", "end", "end"),
    "scrollbar_show": ("overflow", "overflow", "overflow"),
    "scrollbar_hide_delay_ms": (1000.0, 1000.0, 1000.0),
    "scrollbar_fade_in_ms": (100.0, 100.0, 100.0),
    "scrollbar_fade_out_ms": (100.0, 100.0, 100.0),
    "scrollbar_hover_delay_ms": (0.0, 0.0, 0.0),
    "scrollbar_fade_easing": ("standard", "standard", "standard"),
    "scrollbar_gutter_reserve": (True, True, True),
    "scrollbar_arrows": ("none", "none", "none"),
    "scrollbar_arrow_step": (40.0, 40.0, 40.0),
    "scrollbar_lane_click": ("page", "none", "none"),
    "wheel_notch": (40.0, 40.0, 40.0),
    "key_line": (40.0, 40.0, 40.0),
    "page_fraction": (0.9, 1.0, 0.9),
    "scroll_smooth_ms": (0.0, 0.0, 0.0),
    "scroll_easing": ("standard", "standard", "standard"),
    "page_repeat_delay_ms": (400.0, 400.0, 400.0),
    "page_repeat_ms": (100.0, 100.0, 100.0),
}


def test_the_defaults_are_the_spec_for_a_both_view_and_the_legacy_bar_for_the_rest():
    window = new_window()
    nodes = (both_view(window), add(window, "scroll_view", width=100, height=100), make_list(window))
    for name, expected in DEFAULTS.items():
        assert tuple(node.get(name) for node in nodes) == expected, name
    # The list's existing bar is what its unset measures read.
    lst = nodes[2]
    assert [lst.get(n) for n in ("scrollbar_width", "scrollbar_hit_width", "scrollbar_opacity")] == [4.0, 16.0, 1.0]


def test_every_property_reads_back_what_it_was_set_to_and_none_restores_the_default():
    window = new_window()
    values = {
        "scrollbar_radius": 1.5,
        "scrollbar_track_fill": (1, 2, 3, 4),
        "scrollbar_track_opacity": 0.25,
        "scrollbar_fill_hover": (5, 6, 7, 8),
        "scrollbar_fill_drag": (9, 10, 11, 12),
        "scrollbar_side_v": "start",
        "scrollbar_side_h": "top",
        "scrollbar_show": "auto_hide",
        "scrollbar_hide_delay_ms": 250,
        "scrollbar_fade_in_ms": 30,
        "scrollbar_fade_out_ms": 40,
        "scrollbar_hover_delay_ms": 70,
        "scrollbar_fade_easing": (0.1, 0.2, 0.3, 0.4),
        "scrollbar_gutter_reserve": False,
        "scrollbar_arrows": "both",
        "scrollbar_arrow_step": 12,
        "scrollbar_lane_click": "jump",
        "wheel_notch": 18,
        "key_line": 22,
        "page_fraction": 0.5,
        "scroll_smooth_ms": 150,
        "scroll_easing": "ease_out",
        "page_repeat_delay_ms": 300,
        "page_repeat_ms": 50,
    }
    for node in (both_view(window), make_list(window)):
        defaults = {name: node.get(name) for name in values}
        node.set(**values)
        for name, value in values.items():
            got = node.get(name)
            assert got == (float(value) if isinstance(value, int) and not isinstance(value, bool) else value), name
        node.set(**{name: None for name in values})
        assert {name: node.get(name) for name in values} == defaults


@pytest.mark.parametrize(
    ("props", "message"),
    [
        ({"scrollbar_radius": -1}, "a non-negative number"),
        ({"scrollbar_track_opacity": 2}, "a number from 0 to 1"),
        ({"scrollbar_side_v": "top"}, "one of: end, start, left, right"),
        ({"scrollbar_side_h": "left"}, "one of: end, start, bottom, top"),
        ({"scrollbar_show": "sometimes"}, "one of: overflow, always, auto_hide, never"),
        ({"scrollbar_arrows": True}, "one of: none, both"),
        ({"scrollbar_lane_click": "fling"}, "one of: page, jump, none"),
        ({"scrollbar_gutter_reserve": 1}, "a bool"),
        ({"scrollbar_hide_delay_ms": -5}, "milliseconds"),
        ({"scroll_smooth_ms": 10**9}, "milliseconds"),
        ({"page_repeat_ms": 0}, "milliseconds"),
        ({"page_fraction": 0}, "a number from 0.01 to 1"),
        ({"page_fraction": 1.5}, "a number from 0.01 to 1"),
        ({"wheel_notch": float("nan")}, "a non-negative number"),
        ({"scrollbar_fade_easing": "bounce"}, "a preset name"),
        ({"scrollbar_fade_easing": (1.5, 0, 0, 1)}, "cubic bezier"),
        ({"scroll_easing": (0.1, 0.2)}, "cubic bezier"),
        ({"scrollbar_track_fill": (1, 2, 3)}, "an \\(r, g, b, a\\) tuple"),
    ],
)
def test_bad_values_are_rejected(props, message):
    window = new_window()
    for node in (both_view(window), make_list(window)):
        with pytest.raises(ValueError, match=message):
            node.set(**props)


@pytest.mark.parametrize("name", ["scrollbar_radius", "scrollbar_show", "wheel_notch", "scroll_easing", "page_fraction"])
def test_the_properties_apply_only_to_a_scroll_view_or_a_virtual_list(name):
    window = new_window()
    box = add(window, "box")
    with pytest.raises(ValueError, match="applies only to a scroll_view or virtual_list node"):
        box.set(**{name: 1})
    with pytest.raises(ValueError, match="applies only to a scroll_view or virtual_list node"):
        box.get(name)


def test_they_can_be_given_to_create():
    window = new_window()
    view = window.create("scroll_view", scrollbar_radius=1, wheel_notch=7, scrollbar_show="never")
    lst = window.create("virtual_list", item_count=3, item_extent=10, materialize=lambda i: window.create("box"), key_line=9, scrollbar_arrows="both")
    assert (view.get("scrollbar_radius"), view.get("wheel_notch"), view.get("scrollbar_show")) == (1.0, 7.0, "never")
    assert (lst.get("key_line"), lst.get("scrollbar_arrows")) == (9.0, "both")


def test_the_keys_and_the_page_take_their_numbers_on_a_view_and_a_list():
    window = new_window()
    view = both_view(window, wheel_notch=10, key_line=25, page_fraction=0.5)
    item = window.create("box", width=10, height=10, focusable=True)
    view.children()[0].add_child(item)
    lst = make_list(window, wheel_notch=10, key_line=25, page_fraction=0.5)
    row = window.create("box", width=10, height=10, focusable=True)
    lst.add_child(row)
    for node, focus in ((view, item), (lst, row)):
        # (`wheel_notch` scales a wheel that reports lines; a simulated wheel is in pixels.)
        focus.focus()
        window.simulate("key_down", key="arrow_down")
        assert node.get("scroll_offset") == 25.0
        window.simulate("key_down", key="page_down")
        assert node.get("scroll_offset") == 75.0, "half of the 100 viewport"


def test_smooth_scrolling_eases_the_wheel_and_reduced_motion_makes_it_instant():
    window = new_window()
    view = both_view(window, scroll_smooth_ms=200, scroll_easing="linear")
    window.advance(0)
    window.simulate("wheel", node=view, delta_y=40.0)
    assert at(view) == (0.0, 0.0), "eased, not at once"
    window.advance(100)
    assert at(view)[1] == pytest.approx(20.0)
    window.advance(150)
    assert at(view)[1] == 40.0
    window.set(reduced_motion=True)
    window.simulate("wheel", node=view, delta_y=40.0)
    assert at(view)[1] == 80.0, "reduced motion: at once"
    window.set(reduced_motion=False)
    window.simulate("wheel", node=view, delta_y=40.0)
    assert at(view)[1] == 80.0
    window.advance(300)
    assert at(view)[1] == 120.0


def test_the_window_reduced_motion_is_a_bool_or_none_and_none_follows_the_os():
    window = new_window()
    os_value = window.get("reduced_motion")
    window.set(reduced_motion=True)
    assert window.get("reduced_motion") is True
    window.set(reduced_motion=False)
    assert window.get("reduced_motion") is False
    window.set(reduced_motion=None)
    assert window.get("reduced_motion") == os_value
    with pytest.raises(ValueError, match="must be a bool or None"):
        window.set(reduced_motion="yes")


def test_a_lane_click_can_jump_the_thumb_to_the_press():
    window = new_window()
    view = both_view(window, scrollbar_lane_click="jump")
    window.advance(0)
    # The thumb is 32 long from y 2; a press at 44 puts its middle there: half of 52 of travel.
    window.simulate("pointer_down", node=view, x=194, y=44)
    assert view.get("scroll_offset") == 150.0
    window.simulate("pointer_up", node=view, x=194, y=44)
    view.set(scrollbar_lane_click="none", scroll_offset=0)
    window.simulate("pointer_down", node=view, x=194, y=70)
    assert view.get("scroll_offset") == 0.0, "nothing paged"
    window.simulate("pointer_up", node=view, x=194, y=70)


def test_an_arrow_scrolls_a_step_and_repeats_while_held():
    window = new_window()
    view = both_view(window, scrollbar_arrows="both", scrollbar_arrow_step=15, page_repeat_delay_ms=50, page_repeat_ms=10)
    window.advance(0)
    window.simulate("pointer_down", node=view, x=194, y=80)  # the end arrow
    assert view.get("scroll_offset") == 15.0
    window.advance(80)
    assert view.get("scroll_offset") == 75.0
    window.simulate("pointer_up", node=view, x=194, y=80)
    window.advance(200)
    assert view.get("scroll_offset") == 75.0


def test_a_list_has_hover_drag_and_lane_like_a_scroll_view():
    window = new_window()
    lst = make_list(window, scrollbar_lane_click="page", page_fraction=0.5)
    window.simulate("pointer_down", node=lst, x=196, y=80)
    assert lst.get("scroll_offset") == 50.0
    window.simulate("pointer_up", node=lst, x=196, y=80)
    # The thumb is draggable as it always was.
    lst.set(scroll_offset=0)
    window.simulate("pointer_down", node=lst, x=196, y=18)
    window.simulate("pointer_move", node=lst, x=196, y=18 + 32)
    assert lst.get("scroll_offset") == pytest.approx(150.0)
    window.simulate("pointer_up", node=lst, x=196, y=50)


# Pixels.


def test_a_list_that_sets_nothing_keeps_its_bar_and_one_that_sets_the_spec_measures_matches_a_view():
    window = new_window()
    lst = make_list(window)
    assert pixel(window, 196, 18) != BACKGROUND
    assert pixel(window, 191, 18) == BACKGROUND, "4 wide: 194 to 198"
    lst.set(scrollbar_width=8, scrollbar_opacity=0.7, scrollbar_fill=(*THUMB, 255), scrollbar_radius=4)
    got = pixel(window, 194, 18)
    assert close(got, OVER), (got, OVER)
    # A view of the same extents paints the same thumb.
    other = new_window()
    both_view(other, content=(200, 400), size=(200, 100))
    assert close(pixel(other, 194, 18), got)


def test_the_side_of_each_bar_is_a_property():
    window = new_window()
    view = both_view(window)
    assert close(pixel(window, 194, 18), OVER) and pixel(window, 6, 18) == BACKGROUND
    view.set(scrollbar_side_v="start")
    assert close(pixel(window, 6, 18), OVER)
    assert pixel(window, 194, 18) == BACKGROUND
    view.set(scrollbar_side_v="end", scrollbar_side_h="start")
    assert close(pixel(window, 20, 6), OVER), "the horizontal bar on top"
    assert pixel(window, 20, 94) == BACKGROUND


def test_the_radius_the_track_and_the_state_colours_are_painted():
    window = new_window()
    view = both_view(window)
    assert pixel(window, 190, 2) == BACKGROUND, "a pill: the corner is empty"
    view.set(scrollbar_radius=0)
    assert close(pixel(window, 190, 2), OVER), "square"
    assert pixel(window, 194, 60) == BACKGROUND, "no track by default"
    view.set(scrollbar_track_fill=(255, 0, 0, 255))
    assert close(pixel(window, 194, 60), (255, 0, 0, 255))
    view.set(scrollbar_track_fill=(255, 0, 0, 255), scrollbar_track_opacity=0.5)
    assert close(pixel(window, 194, 60), tuple(round(r * 0.5 + b * 0.5) for r, b in zip((255, 0, 0, 255), BACKGROUND)))
    view.set(scrollbar_fill=(0, 200, 0, 255), scrollbar_fill_hover=(0, 0, 200, 255), scrollbar_opacity=1.0, scrollbar_opacity_hover=1.0)
    assert close(pixel(window, 194, 18), (0, 200, 0, 255))
    window.simulate("pointer_move", node=view, x=194, y=18)
    window.advance(200)
    assert close(pixel(window, 194, 18), (0, 0, 200, 255))


def test_arrows_are_drawn_at_the_ends_of_the_lane():
    window = new_window()
    view = both_view(window)
    assert pixel(window, 194, 81) == BACKGROUND
    view.set(scrollbar_arrows="both")
    assert close(pixel(window, 194, 81), OVER), "the end arrow's triangle"
    assert close(pixel(window, 194, 7), OVER), "the start arrow's triangle"
    assert pixel(window, 191, 83) == BACKGROUND, "outside the triangle"


def test_show_never_paints_nothing_and_always_paints_a_full_lane_without_overflow():
    window = new_window()
    view = both_view(window, content=(100, 50))
    assert pixel(window, 194, 18) == BACKGROUND
    view.set(scrollbar_show="always")
    assert close(pixel(window, 194, 18), OVER) and close(pixel(window, 194, 80), OVER)
    view.set(scrollbar_show="never")
    assert pixel(window, 194, 18) == BACKGROUND
    big = both_view(window)
    big.set(scrollbar_show="never")
    window.simulate("wheel", node=big, delta_y=40.0)
    assert big.get("scroll_offset") == 40.0
