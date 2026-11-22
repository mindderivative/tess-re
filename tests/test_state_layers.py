"""0.6.11 (P13): `hover_layer`, `pressed_layer`, `focus_layer`, `focus_layer_any_modality`,
`state_layer_ms` and `state_layer_easing`. The engine tracks hover, press and focus, draws the
layer over the node's fill and fades it; Python only declares the colours. Headless: input from
`Window.simulate`, time from `Window.advance`, pixels from `Window.snapshot`."""

import pytest

from tess_re import Window
from helpers import add

BACK = (0x10, 0x20, 0x30, 255)
FILL = (200, 200, 200, 255)
HOVER = (0, 0, 0, 20)  # 8 %
PRESSED = (0, 0, 0, 26)  # 10 %
FOCUS = (0, 0, 0, 26)


def over(under, layer):
    """What `layer` (black, straight alpha) makes of an opaque grey `under`."""
    a = layer[3] / 255
    return round(under * (1 - a))


def grey(window, node):
    """The red channel of the pixel at the centre of `node` (its fill and layers are grey)."""
    rgba, width, _ = window.snapshot()
    x, y, w, h = (round(v) for v in (node.get("layout_x"), node.get("layout_y"),
                                      node.get("layout_width"), node.get("layout_height")))
    i = ((y + h // 2) * width + x + w // 2) * 4
    return rgba[i]


def scene(**props):
    window = Window(width=200, height=120)
    window.root.set(fill=BACK, padding=0)
    box = add(window, "box", width=100, height=60, fill=FILL, **props)
    return window, box


def settle(window, ms=500):
    window.advance(0)
    window.advance(ms)


# --- the properties ----------------------------------------------------------------


def test_the_defaults_draw_nothing_and_read_back():
    window, box = scene()
    assert box.get("hover_layer") is None
    assert box.get("pressed_layer") is None
    assert box.get("focus_layer") is None
    assert box.get("focus_layer_any_modality") is False
    assert box.get("state_layer_ms") == 100.0
    assert box.get("state_layer_easing") == "standard"
    window.simulate("pointer_move", node=box)
    settle(window)
    assert grey(window, box) == FILL[0]


def test_they_set_get_and_clear_and_work_in_create():
    window = Window(width=100, height=100)
    box = add(window, "box", hover_layer=HOVER, pressed_layer=PRESSED, focus_layer=FOCUS,
              focus_layer_any_modality=True, state_layer_ms=150, state_layer_easing=(0.3, 0, 0.7, 1))
    assert box.get("hover_layer") == HOVER
    assert box.get("pressed_layer") == PRESSED
    assert box.get("focus_layer") == FOCUS
    assert box.get("focus_layer_any_modality") is True
    assert box.get("state_layer_ms") == 150.0
    assert box.get("state_layer_easing") == (0.3, 0.0, 0.7, 1.0)
    box.set(hover_layer=None, state_layer_ms=None, state_layer_easing="linear",
            focus_layer_any_modality=None)
    assert box.get("hover_layer") is None
    assert box.get("state_layer_ms") == 100.0
    assert box.get("state_layer_easing") == "linear"
    assert box.get("focus_layer_any_modality") is False
    box.set(state_layer_easing=None)
    assert box.get("state_layer_easing") == "standard"


@pytest.mark.parametrize("name, value", [
    ("hover_layer", "red"),
    ("pressed_layer", (1, 2)),
    ("focus_layer", 5),
    ("focus_layer_any_modality", 1),
    ("state_layer_ms", -1),
    ("state_layer_ms", True),
    ("state_layer_ms", "fast"),
    ("state_layer_ms", 3_600_001),
    ("state_layer_easing", "wobble"),
    ("state_layer_easing", (2, 0, 0, 1)),
    ("hover_layer_ms", -1),
    ("pressed_layer_ms", True),
    ("focus_layer_ms", "fast"),
    ("hover_layer_ms", 3_600_001),
    ("hover_layer_ms", float("nan")),
    ("hover_layer_easing", "wobble"),
    ("pressed_layer_easing", (2, 0, 0, 1)),
    ("focus_layer_easing", 5),
])
def test_bad_values_raise_without_changing_anything(name, value):
    _, box = scene(hover_layer=HOVER)
    with pytest.raises(ValueError, match=name):
        box.set(**{name: value}, opacity=0.5)
    assert box.get("hover_layer") == HOVER
    assert box.get("opacity") == 1.0


def test_per_state_timings_default_to_none_and_read_back():
    _, box = scene()
    for state in ("hover", "pressed", "focus"):
        assert box.get(f"{state}_layer_ms") is None
        assert box.get(f"{state}_layer_easing") is None
    box.set(hover_layer_ms=15, pressed_layer_ms=105.5, focus_layer_ms=0,
            hover_layer_easing="linear", pressed_layer_easing=(0.3, 0, 0.7, 1),
            focus_layer_easing="standard")
    assert box.get("hover_layer_ms") == 15.0
    assert box.get("pressed_layer_ms") == 105.5
    assert box.get("focus_layer_ms") == 0.0
    assert box.get("hover_layer_easing") == "linear"
    assert box.get("pressed_layer_easing") == (0.3, 0.0, 0.7, 1.0)
    assert box.get("focus_layer_easing") == "standard"
    # The shared pair is untouched, and None falls back to it again.
    assert box.get("state_layer_ms") == 100.0
    box.set(hover_layer_ms=None, hover_layer_easing=None)
    assert box.get("hover_layer_ms") is None
    assert box.get("hover_layer_easing") is None


def test_hover_layer_ms_replaces_state_layer_ms_for_hover_only():
    window, box = scene(hover_layer=HOVER, state_layer_ms=400, hover_layer_ms=20,
                        state_layer_easing="linear")
    window.simulate("pointer_move", node=box)
    window.advance(0)
    window.advance(30)
    assert grey(window, box) == over(200, HOVER), "hover fades over its own 20 ms, not 400"
    window.simulate("pointer_leave")
    window.advance(0)
    window.advance(30)
    assert grey(window, box) == FILL[0], "leaving hover fades out with hover's timing"


def test_pressed_layer_ms_decides_the_change_from_hover_to_pressed():
    window, box = scene(hover_layer=HOVER, pressed_layer=PRESSED, hover_layer_ms=10,
                        pressed_layer_ms=400, state_layer_easing="linear")
    window.simulate("pointer_move", node=box)
    settle(window, 100)
    assert grey(window, box) == over(200, HOVER)
    window.simulate("pointer_down", node=box)
    window.advance(0)
    window.advance(200)
    assert over(200, PRESSED) < grey(window, box) < over(200, HOVER)
    window.advance(300)
    assert grey(window, box) == over(200, PRESSED)


def test_reduced_motion_ignores_the_per_state_timings():
    window, box = scene(hover_layer=HOVER, hover_layer_ms=400)
    window.set(reduced_motion=True)
    window.simulate("pointer_move", node=box)
    window.advance(0)
    assert grey(window, box) == over(200, HOVER)


# --- hover -----------------------------------------------------------------------


def test_hover_fades_the_layer_in_and_out_over_state_layer_ms():
    window, box = scene(hover_layer=HOVER)
    window.simulate("pointer_move", node=box)
    window.advance(0)
    assert grey(window, box) == FILL[0]
    window.advance(50)
    mid = grey(window, box)
    assert over(200, HOVER) < mid < FILL[0], mid
    window.advance(100)
    assert grey(window, box) == over(200, HOVER)
    window.simulate("pointer_leave")
    window.advance(0)
    window.advance(50)
    assert over(200, HOVER) < grey(window, box) < FILL[0]
    window.advance(100)
    assert grey(window, box) == FILL[0]


def test_state_layer_ms_and_easing_change_the_fade():
    window, box = scene(hover_layer=HOVER, state_layer_ms=400, state_layer_easing="linear")
    window.simulate("pointer_move", node=box)
    window.advance(0)
    window.advance(200)
    # Linear, half way: half the 8 %.
    assert grey(window, box) in (FILL[0] - 8, FILL[0] - 7), grey(window, box)
    window.advance(300)
    assert grey(window, box) == over(200, HOVER)


def test_zero_ms_is_at_once():
    window, box = scene(hover_layer=HOVER, state_layer_ms=0)
    window.simulate("pointer_move", node=box)
    window.advance(0)
    assert grey(window, box) == over(200, HOVER)


def test_a_child_under_the_pointer_hovers_its_ancestors_layer():
    window, box = scene(hover_layer=HOVER)
    label = window.create("box", width=20, height=20, fill=(0, 0, 0, 0))
    box.add_child(label)
    window.simulate("pointer_move", node=label)
    settle(window)
    assert grey(window, box) == over(200, HOVER)
    window.simulate("pointer_leave")
    settle(window)
    assert grey(window, box) == FILL[0]


def test_the_layer_is_under_the_children():
    window, box = scene(hover_layer=(0, 0, 0, 255))
    child = window.create("box", width=100, height=60, fill=(255, 0, 0, 255))
    box.add_child(child)
    window.simulate("pointer_move", node=box)
    settle(window)
    rgba, width, _ = window.snapshot()
    i = (30 * width + 50) * 4
    assert tuple(rgba[i:i + 4]) == (255, 0, 0, 255)


def test_the_layer_follows_the_rounded_corner():
    window, box = scene(hover_layer=(0, 0, 0, 255), corner_radius=30)
    window.simulate("pointer_move", node=box)
    settle(window)
    rgba, width, _ = window.snapshot()
    assert tuple(rgba[(1 * width + 1) * 4:(1 * width + 1) * 4 + 4]) == BACK
    i = (30 * width + 50) * 4
    assert tuple(rgba[i:i + 4]) == (0, 0, 0, 255)


# --- pressed and focus, and which shows ---------------------------------------------


def test_pressed_shows_while_held_and_hover_returns_on_release():
    window, box = scene(hover_layer=HOVER, pressed_layer=(0, 0, 0, 51))
    window.simulate("pointer_move", node=box)
    settle(window)
    window.simulate("pointer_down", node=box)
    settle(window)
    assert grey(window, box) == over(200, (0, 0, 0, 51)), "pressed beats hover"
    window.simulate("pointer_up", node=box)
    settle(window)
    assert grey(window, box) == over(200, HOVER)


def test_keyboard_focus_shows_the_focus_layer_and_a_pointer_press_does_not():
    window = Window(width=200, height=120)
    window.root.set(fill=BACK, padding=0)
    a = add(window, "box", width=100, height=40, fill=FILL, focusable=True, focus_layer=FOCUS)
    b = add(window, "box", width=100, height=40, fill=FILL, focusable=True, focus_layer=FOCUS)
    window.simulate("key_down", key="tab")
    settle(window)
    assert grey(window, a) == over(200, FOCUS)
    assert grey(window, b) == FILL[0]
    window.simulate("key_down", key="tab")
    settle(window)
    assert grey(window, a) == FILL[0]
    assert grey(window, b) == over(200, FOCUS)
    # A click moves focus back to `a`, as a pointer: no layer.
    window.simulate("pointer_down", node=a)
    window.simulate("pointer_up", node=a)
    settle(window)
    assert grey(window, a) == FILL[0]
    assert grey(window, b) == FILL[0]
    # With the flag, pointer focus shows it too.
    a.set(focus_layer_any_modality=True)
    window.simulate("pointer_down", node=b)
    window.simulate("pointer_up", node=b)
    window.simulate("pointer_down", node=a)
    window.simulate("pointer_up", node=a)
    settle(window)
    assert grey(window, a) == over(200, FOCUS)


def test_focus_beats_hover():
    window = Window(width=200, height=120)
    window.root.set(fill=BACK, padding=0)
    a = add(window, "box", width=100, height=60, fill=FILL, focusable=True,
            hover_layer=HOVER, focus_layer=(0, 0, 0, 51))
    window.simulate("pointer_move", node=a)
    window.simulate("key_down", key="tab")
    settle(window)
    assert grey(window, a) == over(200, (0, 0, 0, 51))


# --- reduced motion, disabled ------------------------------------------------------


def test_reduced_motion_shows_the_layer_in_one_step():
    window, box = scene(hover_layer=HOVER)
    window.set(reduced_motion=True)
    window.simulate("pointer_move", node=box)
    window.advance(0)
    assert grey(window, box) == over(200, HOVER)
    window.simulate("pointer_leave")
    window.advance(0)
    assert grey(window, box) == FILL[0]


def test_a_disabled_node_draws_no_layer_and_gets_it_when_enabled():
    window, box = scene(hover_layer=HOVER, disabled=True)
    window.simulate("pointer_move", node=box)
    settle(window)
    assert grey(window, box) == FILL[0]
    box.set(disabled=False)
    settle(window)
    assert grey(window, box) == over(200, HOVER)
    box.set(disabled=True)
    settle(window)
    assert grey(window, box) == FILL[0]


def test_setting_a_layer_on_the_hovered_node_applies_at_once():
    window, box = scene()
    window.simulate("pointer_move", node=box)
    settle(window)
    box.set(hover_layer=HOVER)
    settle(window)
    assert grey(window, box) == over(200, HOVER)
    box.set(hover_layer=None)
    settle(window)
    assert grey(window, box) == FILL[0]
