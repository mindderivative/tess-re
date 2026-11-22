"""0.6.12 (P6): the gesture, multi-click and drag start thresholds are window properties.

Headless: `window.simulate` delivers the events the platform does and `window.advance` moves the clock.
The recogniser's own arithmetic is tested in `engine-core/src/gesture.rs`, the multi-click counter and the
drag start gate in `engine-py` (`text_interaction.rs`, `pointer_settings.rs`); this file proves the
properties reach them.
"""

import math

import pytest

from tess_re import Window

DEFAULTS = {
    "gesture_long_press_ms": 500.0,
    "gesture_tap_ms": 300.0,
    "gesture_double_tap_ms": 300.0,
    "gesture_double_tap_px": 30.0,
    "gesture_swipe_speed": 600.0,
    "gesture_swipe_px": 40.0,
    "gesture_rotate_slop": 0.12,
    "multi_click_ms": 500.0,
    "multi_click_px": 4.0,
    "drag_start_px": 0.0,
}


def scene():
    window = Window(width=300, height=300)
    window.root.set(padding=0)
    box = window.create("box", width=200, height=200, x=20, y=20, position="absolute",
                        fill=(200, 200, 200, 255))
    window.root.add_child(box)
    log = []
    for name in ("tap", "long_press", "pan", "swipe", "rotate", "pointer_down", "pointer_move",
                 "pointer_up", "click"):
        box.on(name, lambda e, name=name: log.append((name, e)))
    return window, box, log


def names(log):
    return [n for n, _ in log]


def touch(window, phase, x, y, id=0):
    window.simulate(f"touch_{phase}", x=x, y=y, id=id)


# -- get, set and validation ---------------------------------------------------------------------


def test_the_defaults_are_the_values_they_always_had():
    window = Window()
    for name, value in DEFAULTS.items():
        assert window.get(name) == pytest.approx(value), name


def test_every_threshold_reads_back_as_set_and_leaves_the_others():
    window = Window()
    new = {name: value * 2 + 1 for name, value in DEFAULTS.items()}
    new["gesture_rotate_slop"] = 0.5
    window.set(**new)
    for name, value in new.items():
        assert window.get(name) == pytest.approx(value), name
    # One at a time leaves the rest alone.
    window = Window()
    window.set(gesture_tap_ms=123)
    assert window.get("gesture_tap_ms") == pytest.approx(123.0)
    for name, value in DEFAULTS.items():
        if name != "gesture_tap_ms":
            assert window.get(name) == pytest.approx(value), name


@pytest.mark.parametrize("name,bad", [
    ("gesture_long_press_ms", 0), ("gesture_long_press_ms", -5), ("gesture_long_press_ms", 4e6),
    ("gesture_tap_ms", 0), ("gesture_tap_ms", "slow"),
    ("gesture_double_tap_ms", -1), ("gesture_double_tap_ms", float("nan")),
    ("gesture_double_tap_px", -1), ("gesture_double_tap_px", float("inf")),
    ("gesture_swipe_speed", -1), ("gesture_swipe_speed", None),
    ("gesture_swipe_px", -0.5),
    ("gesture_rotate_slop", -0.1), ("gesture_rotate_slop", 4.0),
    ("multi_click_ms", -1), ("multi_click_ms", 3.7e6),
    ("multi_click_px", -1), ("multi_click_px", "near"),
    ("drag_start_px", -1), ("drag_start_px", float("nan")),
])
def test_a_bad_value_raises_naming_the_property_and_changes_nothing(name, bad):
    window = Window()
    with pytest.raises(ValueError, match=name):
        window.set(**{name: bad})
    # Even with a good one beside it.
    other = "gesture_swipe_px" if name != "gesture_swipe_px" else "gesture_tap_ms"
    with pytest.raises(ValueError, match=name):
        window.set(**{other: 99, name: bad})
    assert window.get(name) == pytest.approx(DEFAULTS[name])
    assert window.get(other) == pytest.approx(DEFAULTS[other])


def test_zero_is_allowed_where_it_means_off():
    window = Window()
    window.set(gesture_double_tap_ms=0, multi_click_ms=0, drag_start_px=0, gesture_swipe_px=0,
               gesture_rotate_slop=0, gesture_double_tap_px=0, multi_click_px=0)
    assert window.get("gesture_double_tap_ms") == 0.0 and window.get("multi_click_ms") == 0.0


def test_an_unknown_property_error_lists_the_thresholds():
    window = Window()
    with pytest.raises(ValueError, match="drag_start_px"):
        window.get("nope")
    with pytest.raises(ValueError, match="gesture_long_press_ms"):
        window.set(nope=1)


# -- the touch gestures follow the properties ----------------------------------------------------


def test_the_long_press_time_decides_when_a_held_touch_is_a_long_press():
    window, box, log = scene()
    touch(window, "start", 100, 100)
    window.advance(300)
    assert "long_press" not in names(log)
    touch(window, "end", 100, 100)
    # A shorter time: the same hold now is one.
    window.set(gesture_long_press_ms=200)
    log.clear()
    touch(window, "start", 100, 100)
    window.advance(250)
    assert names(log).count("long_press") == 1
    touch(window, "end", 100, 100)
    # A longer one: the default's 550 ms hold is not.
    window.set(gesture_long_press_ms=900)
    log.clear()
    touch(window, "start", 100, 100)
    window.advance(550)
    assert "long_press" not in names(log)
    window.advance(400)
    assert names(log).count("long_press") == 1


def taps(window, log, gap, second=(100, 100)):
    log.clear()
    window.advance(2000)
    touch(window, "start", 100, 100)
    touch(window, "end", 100, 100)
    window.advance(gap)
    touch(window, "start", *second)
    touch(window, "end", *second)
    return [e.count for n, e in log if n == "tap"]


def test_the_double_tap_gap_and_distance_decide_what_is_a_double_tap():
    window, box, log = scene()
    assert taps(window, log, 400) == [1, 1]
    window.set(gesture_double_tap_ms=500)
    assert taps(window, log, 400) == [1, 2]
    window.set(gesture_double_tap_ms=100)
    assert taps(window, log, 200) == [1, 1]
    window.set(gesture_double_tap_ms=300)
    assert taps(window, log, 100, second=(145, 100)) == [1, 1], "45 px apart is past 30"
    window.set(gesture_double_tap_px=60)
    assert taps(window, log, 100, second=(145, 100)) == [1, 2]


def test_the_tap_time_decides_how_long_a_touch_may_be_and_still_tap():
    window, box, log = scene()
    window.set(gesture_long_press_ms=2000)
    touch(window, "start", 100, 100)
    window.advance(400)
    touch(window, "end", 100, 100)
    assert "tap" not in names(log), "400 ms is past the default 300"
    window.set(gesture_tap_ms=500)
    touch(window, "start", 100, 100)
    window.advance(400)
    touch(window, "end", 100, 100)
    assert "tap" in names(log)


def flick(window, x0, x1, ms, steps=5):
    touch(window, "start", x0, 120)
    for i in range(1, steps + 1):
        window.advance(ms / steps)
        touch(window, "move", x0 + (x1 - x0) * i / steps, 120)
    touch(window, "end", x1, 120)


def test_the_swipe_speed_and_distance_decide_what_is_a_swipe():
    window, box, log = scene()
    flick(window, 100, 300, 150)                       # 200 px in 150 ms
    assert names(log).count("swipe") == 1
    window.set(gesture_swipe_speed=2000)
    log.clear()
    flick(window, 100, 300, 150)
    assert "swipe" not in names(log) and "pan" in names(log)
    window.set(gesture_swipe_speed=600)
    log.clear()
    flick(window, 100, 300, 1000, steps=50)
    assert "swipe" not in names(log), "200 px/s is under the default 600"
    window.set(gesture_swipe_speed=600, gesture_swipe_px=300)
    log.clear()
    flick(window, 100, 300, 150)
    assert "swipe" not in names(log), "200 px is short of 300"
    window.set(gesture_swipe_speed=150, gesture_swipe_px=40)
    log.clear()
    flick(window, 100, 300, 1000, steps=50)             # slow: 200 px/s
    assert names(log).count("swipe") == 1


def turn(window, radians, r=100):
    def at(rad):
        dx, dy = math.cos(rad) * r / 2, math.sin(rad) * r / 2
        return (120 - dx, 120 - dy), (120 + dx, 120 + dy)
    (a0, b0) = at(0)
    touch(window, "start", *a0, id=1)
    touch(window, "start", *b0, id=2)
    a, b = at(radians)
    touch(window, "move", *a, id=1)
    touch(window, "move", *b, id=2)
    touch(window, "end", *b, id=2)
    touch(window, "end", *a, id=1)


def test_the_rotate_slop_decides_when_a_turn_is_a_rotation():
    window, box, log = scene()
    turn(window, 0.2)
    assert "rotate" in names(log)
    window.set(gesture_rotate_slop=0.5)
    log.clear()
    turn(window, 0.2)
    assert "rotate" not in names(log)


# -- multi-click in text -------------------------------------------------------------------------


def text_window():
    window = Window(width=300, height=120)
    window.root.set(padding=0)
    label = window.create("text", text="hello world", font_size=16, width=280, height=40,
                          selectable=True)
    window.root.add_child(label)
    window.advance(1)
    return window, label


def press_at(window, label, x, gap):
    window.advance(gap)
    window.simulate("pointer_down", node=label, x=x, y=8)
    window.simulate("pointer_up", node=label, x=x, y=8)


def selected(window, label):
    start, end = sorted(label.get("selection"))
    return "hello world"[start:end]


def test_the_multi_click_time_decides_what_a_double_click_is():
    window, label = text_window()
    press_at(window, label, 10, 2000)
    press_at(window, label, 10, 700)
    assert selected(window, label) == "", "700 ms is past the 500 ms default"
    window.set(multi_click_ms=800)
    press_at(window, label, 10, 2000)
    press_at(window, label, 10, 700)
    assert selected(window, label) == "hello"
    window.set(multi_click_ms=100)
    press_at(window, label, 10, 2000)
    press_at(window, label, 10, 200)
    assert selected(window, label) == ""


def test_the_multi_click_distance_decides_what_a_double_click_is():
    window, label = text_window()
    press_at(window, label, 10, 2000)
    press_at(window, label, 18, 100)
    assert selected(window, label) == "", "8 px apart is past the 4 px default"
    window.set(multi_click_px=10)
    press_at(window, label, 10, 2000)
    press_at(window, label, 18, 100)
    assert selected(window, label) == "hello"


def test_the_multi_click_distance_is_also_the_title_bar_double_click_slop():
    window = Window(width=400, height=300)
    bar = window.create("box", width=300, height=40, window_region="drag")
    window.root.add_child(bar)
    window.advance(0)

    def double(second_x):
        window.simulate("pointer_down", node=bar, x=200, y=20)
        window.simulate("pointer_up", node=bar, x=200, y=20)
        window.advance(100)
        window.simulate("pointer_down", node=bar, x=second_x, y=20)
        window.simulate("pointer_up", node=bar, x=second_x, y=20)

    double(220)
    assert window.get("maximized") is False, "20 px apart is past the 4 px default"
    window.advance(1000)
    window.set(multi_click_px=30)
    double(220)
    assert window.get("maximized") is True


# -- drag start ----------------------------------------------------------------------------------


def drag(window, box, xs):
    window.simulate("pointer_down", node=box, x=xs[0], y=100)
    for x in xs[1:]:
        window.simulate("pointer_move", node=box, x=x, y=100)
    window.simulate("pointer_up", node=box, x=xs[-1], y=100)


def test_by_default_every_move_of_a_held_press_is_heard():
    window, box, log = scene()
    drag(window, box, [100, 101, 102, 110])
    assert names(log).count("pointer_move") == 3


def test_a_held_press_is_silent_until_it_has_moved_drag_start_px_and_then_always_heard():
    window, box, log = scene()
    window.set(drag_start_px=4)
    drag(window, box, [100, 101, 102, 103, 106, 101, 100])
    moves = [e.window_x for n, e in log if n == "pointer_move"]
    assert moves == [126, 121, 120], "nothing inside 4 px, then everything, even back at the press"
    assert names(log).count("click") == 1 and names(log).count("pointer_down") == 1
    # A press that never gets that far is a click with no move at all.
    log.clear()
    drag(window, box, [100, 102, 100, 103])
    assert "pointer_move" not in names(log) and names(log).count("click") == 1
    # A new press starts afresh, and a move with no button held is never held back.
    log.clear()
    window.simulate("pointer_move", node=box, x=101, y=100)
    assert names(log) == ["pointer_move"]
    drag(window, box, [100, 101, 120])
    assert [e.window_x for n, e in log if n == "pointer_move"][1:] == [140]
    window.set(drag_start_px=0)
    log.clear()
    drag(window, box, [100, 101])
    assert names(log).count("pointer_move") == 1
