"""UXREQ-007 (0.6.12): opt-in edge auto-scroll of a scroll view and a virtual list while a
gesture is held: `edge_scroll`, `edge_scroll_band`, `edge_scroll_speed`. A pointer held 10 dp
from the bottom edge moves the offset by 8 to 32 dp each frame and by zero the frame after it
leaves. Headless, with `window.advance`."""

import pytest

from tess_re import Window
from helpers import add

FRAME = 1000 / 60


def make_view(window, **props):
    view = window.create("scroll_view", width=200, height=100, flex_shrink=0, **props)
    column = window.create("box", width=180, flex_direction="vertical", flex_shrink=0)
    rows = []
    for _ in range(60):
        row = window.create("box", width=180, height=20, flex_shrink=0)
        column.add_child(row)
        rows.append(row)
    view.add_child(column)
    window.root.add_child(view)
    view.on("pointer_down", lambda e: view.capture_pointer())
    window.advance(0)
    return view, rows


def new_window():
    window = Window(width=300, height=200)
    window.root.set(padding=0)
    return window


def steps(window, view, frames):
    out = []
    for _ in range(frames):
        before = view.get("scroll_offset")
        window.advance(FRAME)
        out.append(view.get("scroll_offset") - before)
    return out


def test_a_pointer_10_dp_from_the_bottom_scrolls_8_to_32_a_frame_and_stops_when_it_leaves():
    window = new_window()
    view, _ = make_view(window, edge_scroll=True)
    scrolled = []
    view.on("scroll", lambda e: scrolled.append(e.new_value))
    window.simulate("pointer_down", node=view, x=50, y=50)
    window.simulate("pointer_move", node=view, x=50, y=90)
    got = steps(window, view, 6)
    assert all(8 <= step <= 32 for step in got), got
    assert got[0] == pytest.approx(22, abs=0.5)
    assert scrolled, "the scroll event fires"
    # Out of the band: zero the frame after.
    window.simulate("pointer_move", node=view, x=50, y=50)
    assert steps(window, view, 2) == [0, 0]
    # Back in the band, then the release ends it.
    window.simulate("pointer_move", node=view, x=50, y=95)
    assert steps(window, view, 1)[0] > 0
    window.simulate("pointer_up", node=view, x=50, y=95)
    assert steps(window, view, 2) == [0, 0]


def test_the_step_rises_with_the_depth_in_the_band_and_scales_with_the_frame_time():
    window = new_window()
    view, _ = make_view(window, edge_scroll=True)
    window.simulate("pointer_down", node=view, x=50, y=50)
    window.simulate("pointer_move", node=view, x=50, y=100)
    assert steps(window, view, 2)[1] == pytest.approx(32, abs=0.5)
    window.simulate("pointer_move", node=view, x=50, y=77)
    assert steps(window, view, 2)[1] == pytest.approx(8 + 24 / 24, abs=0.5)
    # 30 Hz: twice the distance per step.
    window.simulate("pointer_move", node=view, x=50, y=90)
    window.advance(FRAME)
    before = view.get("scroll_offset")
    window.advance(2 * FRAME)
    assert view.get("scroll_offset") - before == pytest.approx(44, abs=0.5)


def test_the_top_edge_scrolls_back_and_the_band_and_speed_are_settable():
    window = new_window()
    view, _ = make_view(window, edge_scroll=True, edge_scroll_band=40, edge_scroll_speed=(10, 20))
    view.set(scroll_offset=300)
    window.simulate("pointer_down", node=view, x=50, y=50)
    window.simulate("pointer_move", node=view, x=50, y=0)
    got = steps(window, view, 2)
    assert got[1] == pytest.approx(-20, abs=0.5)
    # 30 dp from the top is inside a 40 dp band: t = 10/40, 10 + 10 * 0.25.
    window.simulate("pointer_move", node=view, x=50, y=30)
    assert steps(window, view, 1)[0] == pytest.approx(-12.5, abs=0.5)


def test_it_is_opt_in_and_needs_a_gesture_and_stops_at_the_end():
    window = new_window()
    off, _ = make_view(window)
    window.simulate("pointer_down", node=off, x=50, y=50)
    window.simulate("pointer_move", node=off, x=50, y=95)
    assert steps(window, off, 3) == [0, 0, 0]
    window.simulate("pointer_up", node=off, x=50, y=95)

    window = new_window()
    view, _ = make_view(window, edge_scroll=True)
    window.simulate("pointer_move", node=view, x=50, y=95)
    assert steps(window, view, 3) == [0, 0, 0], "no gesture held"
    window.simulate("pointer_down", node=view, x=50, y=95)
    for _ in range(200):
        window.advance(FRAME)
    assert view.get("scroll_offset") == 1200 - 100


def test_the_pointer_leaving_the_window_keeps_scrolling_at_its_last_position():
    window = new_window()
    view, _ = make_view(window, edge_scroll=True)
    window.simulate("pointer_down", node=view, x=50, y=50)
    window.simulate("pointer_move", node=view, x=50, y=98)
    window.simulate("pointer_leave")
    assert all(step > 0 for step in steps(window, view, 3))


def test_hover_follows_the_scroll_under_a_stationary_pointer():
    window = new_window()
    view, rows = make_view(window, edge_scroll=True)
    entered = []
    for i, row in enumerate(rows):
        row.on("pointer_enter", lambda e, i=i: entered.append(i))
    window.simulate("pointer_down", node=view, x=50, y=50)
    window.simulate("pointer_move", node=view, x=50, y=95)
    steps(window, view, 12)
    assert len(entered) >= 3 and entered == sorted(entered), entered


def test_the_dock_drag_scrolls_a_view_under_the_pointer_and_stops_with_the_drop():
    window = Window(width=500, height=120)
    window.root.set(padding=0)
    left = add(window, "box", width=100, height=100, flex_shrink=0)
    window.add_dock_zone("left", left, 100.0)
    panel = add(window, "box", width=100, height=80)
    window.dock_panel("left", panel)
    view, _ = make_view(window, edge_scroll=True)
    window.start_panel_drag(panel)
    window.simulate("pointer_move", node=view, x=50, y=95)
    assert all(8 <= step <= 32 for step in steps(window, view, 3))
    window.simulate("pointer_up", node=view, x=50, y=95)
    assert steps(window, view, 2) == [0, 0]
    # Escape stops it as well.
    window.start_panel_drag(panel)
    window.simulate("pointer_move", node=view, x=50, y=95)
    assert steps(window, view, 1)[0] > 0
    window.simulate("key_down", key="escape")
    assert steps(window, view, 2) == [0, 0]


def test_a_virtual_list_takes_the_same_properties():
    window = new_window()
    lst = window.create(
        "virtual_list",
        item_count=100,
        item_extent=20,
        materialize=lambda i: window.create("box", width=100, height=20),
        width=200,
        height=100,
        flex_shrink=0,
        edge_scroll=True,
    )
    window.root.add_child(lst)
    lst.on("pointer_down", lambda e: lst.capture_pointer())
    window.advance(0)
    window.simulate("pointer_down", node=lst, x=50, y=50)
    window.simulate("pointer_move", node=lst, x=50, y=92)
    got = steps(window, lst, 3)
    assert all(8 <= step <= 32 for step in got), got
    assert lst.get("edge_scroll") is True


def test_the_properties_round_trip_and_validate():
    window = new_window()
    view, _ = make_view(window)
    assert view.get("edge_scroll") is False
    assert view.get("edge_scroll_band") == 24.0
    assert view.get("edge_scroll_speed") == (8.0, 32.0)
    view.set(edge_scroll=True, edge_scroll_band=10, edge_scroll_speed=(2, 5))
    assert (view.get("edge_scroll"), view.get("edge_scroll_band"), view.get("edge_scroll_speed")) == (True, 10.0, (2.0, 5.0))
    view.set(edge_scroll=None, edge_scroll_band=None, edge_scroll_speed=None)
    assert view.get("edge_scroll") is False and view.get("edge_scroll_speed") == (8.0, 32.0)
    for name, bad in [
        ("edge_scroll", 1),
        ("edge_scroll_band", -1),
        ("edge_scroll_band", True),
        ("edge_scroll_speed", 8),
        ("edge_scroll_speed", (8, -1)),
        ("edge_scroll_speed", (1, 2, 3)),
        ("edge_scroll_speed", (float("nan"), 1)),
    ]:
        with pytest.raises(ValueError, match=name):
            view.set(**{name: bad})
    box = add(window, "box", width=10, height=10)
    with pytest.raises(ValueError, match="applies only to a scroll_view or virtual_list"):
        box.set(edge_scroll=True)
