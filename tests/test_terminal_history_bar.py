"""0.6.12 (UXREQ-006, second half): a terminal's history scrollbar. It is opt-in: `scrollbar_show`
is `"never"` until set, and then the spec's thumb (the `scrollbar_*` properties of a scroll view)
draws over the history and the screen. The properties are checked headless; the bar over a real
shell (history from a PTY, a drag and a lane click moving the view) runs in one `App.run()` and
skips when no display renders frames."""

import sys

import pytest

from tess_re import App, Window
from helpers import add

pytestmark = pytest.mark.skipif(sys.platform == "win32", reason="Terminal is POSIX-only in this v1")

BLACK = (0, 0, 0, 255)


def terminal(window, **props):
    props = {"shell": "/bin/sh", "cols": 40, "rows": 10, "palette": {"background": BLACK}, **props}
    return add(window, "terminal", **props)


def test_the_history_bar_is_off_by_default_and_reads_the_spec_numbers():
    window = Window(width=400, height=300)
    term = terminal(window)
    assert term.get("scrollbar_show") == "never"
    # The spec's measures, as a `both` scroll view has them (UXREQ-001).
    assert term.get("scrollbar_width") == 8
    assert term.get("scrollbar_hit_width") == 12
    assert term.get("scrollbar_margin") == 2
    assert term.get("scrollbar_min_length") == 32
    assert term.get("scrollbar_opacity") == pytest.approx(0.7)
    assert term.get("scrollbar_opacity_hover") == pytest.approx(0.8)
    assert term.get("scrollbar_opacity_drag") == 1.0
    assert term.get("scrollbar_lane_click") == "page"
    assert term.get("scrollbar_fill") is None
    assert term.get("page_fraction") == pytest.approx(0.9)


def test_opting_in_and_out():
    window = Window(width=400, height=300)
    term = terminal(window, scrollbar_show="overflow")
    assert term.get("scrollbar_show") == "overflow"
    term.set(scrollbar_show=None)
    assert term.get("scrollbar_show") == "never"
    term.set(scrollbar_show="auto_hide", scrollbar_width=6, scrollbar_fill=(255, 0, 0, 255))
    assert term.get("scrollbar_show") == "auto_hide"
    assert term.get("scrollbar_width") == 6
    assert term.get("scrollbar_fill") == (255, 0, 0, 255)


@pytest.mark.parametrize(
    "name, value",
    [
        ("scrollbar_gutter", 12),
        ("scrollbar_gutter_reserve", True),
        ("scrollbar_corner", 12),
        ("scrollbar_side_h", "top"),
        ("wheel_notch", 40),
        ("key_line", 40),
        ("scroll_smooth_ms", 100),
        ("scroll_easing", "linear"),
        ("scroll_snap_settle_ms", 1),
        ("fling_tau_ms", 100),
    ],
)
def test_the_properties_a_terminal_has_no_use_for_are_refused(name, value):
    window = Window(width=400, height=300)
    term = terminal(window)
    with pytest.raises(ValueError, match="does not apply to a terminal"):
        term.set(**{name: value})
    with pytest.raises(ValueError, match="does not apply to a terminal"):
        term.get(name)


def test_bad_values_raise_as_on_a_scroll_view():
    window = Window(width=400, height=300)
    term = terminal(window)
    for props in ({"scrollbar_show": "sometimes"}, {"scrollbar_width": -1}, {"scrollbar_opacity": 2}):
        with pytest.raises(ValueError, match="scrollbar_"):
            term.set(**props)
    assert term.get("scrollbar_show") == "never"


def test_the_bar_properties_still_name_the_old_kinds_elsewhere():
    window = Window(width=400, height=300)
    box = window.create("box")
    with pytest.raises(ValueError, match="applies only to a scroll_view or virtual_list node"):
        box.set(scrollbar_show="always")


def pixel(window, x, y):
    rgba, width, _ = window.snapshot()
    start = (y * width + x) * 4
    return tuple(rgba[start : start + 4])


def test_always_draws_a_thumb_as_long_as_the_lane_over_a_terminal_without_history():
    window = Window(width=400, height=300)
    window.root.set(padding=0)
    term = terminal(window, scrollbar_show="always", position="absolute", x=0, y=0)
    window.advance(16)
    width, height = int(term.get("layout_width")), int(term.get("layout_height"))
    x = width - 2 - 4  # the thumb's middle: 2 px margin and half of 8 px
    # The thumb is the scheme's on_surface_variant (#49454F) at 70 % opacity: a grey on black.
    # What it blends with depends on the window's own alpha, so compare it with what is beside it.
    beside = pixel(window, x - 6, height // 2)
    assert beside[:3] == (0, 0, 0), "8 px wide: nothing beside it"
    for y in (3, height // 2, height - 4):  # from the top of the lane to its foot
        thumb = pixel(window, x, y)
        assert all(40 <= c <= 70 for c in thumb[:3]), (y, thumb)
    assert pixel(window, 5, height // 2)[:3] == (0, 0, 0), "and nowhere else"
    term.set(scrollbar_show="never")
    window.advance(16)
    assert pixel(window, x, height // 2)[:3] == (0, 0, 0)


def test_a_real_shell_history_moves_with_the_bar():
    """500-line-style history on a real PTY: the thumb sits at the foot, a drag toward the top
    and a lane click move the view through the history, and the session follows the bar."""
    window = Window(width=420, height=300)
    window.root.set(padding=0)
    term = terminal(window, scrollback_lines=1000, scrollbar_show="overflow", position="absolute", x=0, y=0)
    window.simulate("click", node=term)
    window.simulate("input", text="seq 1 300")
    window.simulate("key_down", key="enter")

    app = App()
    app.add_window(window)
    handle = app.thread_handle()
    seen: dict = {}
    frames_ran = []
    handle.call_soon(lambda: frames_ran.append(True))

    def last_number():
        numbers = [int(line) for line in term.get("text").splitlines() if line.strip().isdigit()]
        return numbers[-1] if numbers else None

    def geometry():
        width, height = term.get("layout_width"), term.get("layout_height")
        return width - 6.0, height  # the thumb's column, the box height

    def drag():
        try:
            seen["before"] = last_number()
            x, height = geometry()
            # The thumb is at the foot, 32 px long: grab its middle, pull toward the top.
            y = height - 2 - 16
            window.simulate("pointer_down", x=x, y=y)
            window.simulate("pointer_move", x=x, y=y - height / 2)
            window.simulate("pointer_up", x=x, y=y - height / 2)
        except Exception as error:  # noqa: BLE001 -- reported after the run
            seen["error"] = error

    def after_drag():
        try:
            seen["dragged"] = last_number()
            x, height = geometry()
            # The thumb is now above the middle: a press in the lane below it pages toward the end.
            window.simulate("pointer_down", x=x, y=height - 6)
            window.simulate("pointer_up", x=x, y=height - 6)
        except Exception as error:  # noqa: BLE001
            seen["error"] = error

    def after_click():
        seen["paged"] = last_number()
        handle.stop()

    window.after(1500, drag)
    window.after(1900, after_drag)
    window.after(2300, after_click)
    window.after(15000, handle.stop)  # a stuck run still ends
    app.run()
    if not frames_ran:
        pytest.skip("no display reachable: App.run() rendered no frames, so PTY output never drained")
    assert "error" not in seen, seen.get("error")
    assert seen["before"] == 300 or seen["before"] is not None and seen["before"] >= 299, seen
    assert seen["dragged"] < seen["before"] - 50, f"a drag up the bar shows earlier output: {seen}"
    assert seen["dragged"] < 300
    assert seen["paged"] > seen["dragged"], f"a lane click below the thumb pages toward the end: {seen}"
