"""0.6.11: the named easings of `engine_core::EASINGS` are taken everywhere an easing
is: `Node.animate` (and the path, layout and shader animations it shares a parser
with), and the scroll snap property `scroll_snap_easing`. The snap and fling timings
are properties of the scroll view and the virtual list. Driven headless with
`Window.advance`."""

import pytest

from tess_re import Shader, Window
from helpers import add

# The Tesserae Motion table (Motion.md, "Easing"): y at x = 0.25 / 0.5 / 0.75.
MOTION = {
    "linear": (0.250, 0.500, 0.750),
    "standard": (0.607, 0.878, 0.975),
    "standard_decelerate": (0.691, 0.890, 0.976),
    "standard_accelerate": (0.129, 0.372, 0.668),
    "emphasized": (0.607, 0.878, 0.975),
    "emphasized_decelerate": (0.832, 0.950, 0.991),
    "emphasized_accelerate": (0.035, 0.154, 0.406),
}
CSS = ("ease", "ease_in", "ease_out", "ease_in_out")
PRESETS = tuple(MOTION) + CSS


def box_window():
    window = Window(width=200, height=100)
    box = window.create("box", width=40, height=40, fill=(200, 0, 0, 255))
    window.root.add_child(box)
    return window, box


@pytest.mark.parametrize("name", PRESETS)
def test_every_preset_reaches_the_end_value(name):
    window, box = box_window()
    box.animate("translate_x", 100, 200, easing=name)
    window.advance(1)
    window.advance(400)
    assert box.get("translate_x") == 100.0
    assert box.get_target("translate_x") == 100.0


@pytest.mark.parametrize("name", MOTION)
def test_a_preset_follows_the_motion_table_at_a_quarter_half_and_three_quarters(name):
    window, box = box_window()
    box.animate("translate_x", 100, 400, easing=name)
    window.advance(1)  # starts it on the window's clock
    seen = []
    for step in (100, 100, 100):
        window.advance(step)
        seen.append(box.get("translate_x") / 100.0)
    # advance(1) already moved the clock 1 ms: stay within the 0.005 of the spec plus that ms.
    for got, want in zip(seen, MOTION[name]):
        assert got == pytest.approx(want, abs=0.02), (name, seen)


def test_a_name_is_the_same_curve_as_its_bezier():
    window, named = box_window()
    other = window.create("box", width=40, height=40)
    window.root.add_child(other)
    named.animate("translate_x", 100, 300, easing="emphasized_decelerate")
    other.animate("translate_x", 100, 300, easing=(0.05, 0.7, 0.1, 1.0))
    window.advance(1)
    for _ in range(5):
        window.advance(50)
        assert named.get("translate_x") == pytest.approx(other.get("translate_x"), abs=0.1)


def test_the_existing_forms_still_work():
    window, box = box_window()
    for easing in (None, "linear", (0.2, 0.0, 0.0, 1.0), "spring", ("spring", 0.4)):
        box.animate("translate_x", 50, 100, easing=easing)
        window.advance(1)
        window.advance(5000)
        assert box.get("translate_x") == pytest.approx(50.0, abs=1e-6), easing
        box.animate("translate_x", 0, 0)
        window.advance(1)


def test_a_preset_works_on_every_animation_that_takes_an_easing():
    window = Window(width=100, height=100)
    path = window.create("path", data="M0,0 L10,10", width=50, height=50, fill=(255, 0, 0, 255))
    window.root.add_child(path)
    path.animate("trim_end", 0.25, 200, easing="emphasized_accelerate")
    window.advance(1)
    window.advance(300)
    assert path.get("trim_end") == pytest.approx(0.25)
    shader = Shader("fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.level, 0.0, 0.0, 1.0);\n}\n", uniforms={"level": 0.0})
    box = window.create("box", width=20, height=20)
    window.root.add_child(box)
    box.set(shader=shader)
    shader.animate("level", 1.0, 200, easing="standard_decelerate")
    window.advance(1)
    window.advance(300)
    assert shader.uniforms["level"] == 1.0
    width = window.create("box", width=10, height=10)
    window.root.add_child(width)
    width.animate("width", 80, 200, easing="ease_in_out")
    window.advance(1)
    window.advance(300)
    assert width.get("width") == 80.0


@pytest.mark.parametrize("bad", ["bouncy", "", "Standard", (0.1, 0.2), (1.5, 0, 0, 1), ("spring", 1.0), 5, ("ease", 0.2)])
def test_an_unknown_easing_raises_and_lists_the_valid_ones(bad):
    window, box = box_window()
    with pytest.raises(ValueError) as caught:
        box.animate("translate_x", 10, 100, easing=bad)
    message = str(caught.value)
    assert message.startswith("easing must be")
    for name in PRESETS:
        assert name in message, name
    assert "spring" in message and "cubic bezier" in message
    assert box.get("translate_x") == 0.0, "nothing was started"


def test_an_unknown_easing_raises_on_the_shader_and_the_animate_param_too():
    shader = Shader("fn shade(p: Pixel) -> vec4<f32> {\n    return vec4<f32>(u.level, 0.0, 0.0, 1.0);\n}\n", uniforms={"level": 0.5})
    with pytest.raises(ValueError, match="emphasized_decelerate"):
        shader.animate("level", 1.0, 100, easing="bouncy")


SNAP_DEFAULTS = {
    "scroll_snap_settle_ms": 120.0,
    "scroll_snap_duration_ms": 200.0,
    "scroll_snap_easing": "standard",
    "fling_tau_ms": 350.0,
    "fling_stop_speed": 30.0,
    "fling_min_speed": 150.0,
}


def view_and_list(window):
    view = add(window, "scroll_view", width=100, height=100)
    lst = add(
        window,
        "virtual_list",
        item_count=20,
        item_extent=20,
        materialize=lambda i: window.create("box", width=100, height=20),
        width=100,
        height=100,
        flex_shrink=0,
    )
    return view, lst


def test_the_snap_and_fling_properties_read_their_defaults_and_are_settable_on_a_view_and_a_list():
    window = Window(width=300, height=300)
    window.root.set(padding=0)
    for node in view_and_list(window):
        for name, default in SNAP_DEFAULTS.items():
            assert node.get(name) == default, name
        node.set(
            scroll_snap_settle_ms=50,
            scroll_snap_duration_ms=400,
            scroll_snap_easing="ease_out",
            fling_tau_ms=700,
            fling_stop_speed=10,
            fling_min_speed=300,
        )
        assert node.get("scroll_snap_settle_ms") == 50.0
        assert node.get("scroll_snap_duration_ms") == 400.0
        assert node.get("scroll_snap_easing") == "ease_out"
        assert (node.get("fling_tau_ms"), node.get("fling_stop_speed"), node.get("fling_min_speed")) == (700.0, 10.0, 300.0)
        node.set(scroll_snap_easing=(0.1, 0.2, 0.3, 0.4))
        assert node.get("scroll_snap_easing") == (0.1, 0.2, 0.3, 0.4)
        node.set(**{name: None for name in SNAP_DEFAULTS})
        for name, default in SNAP_DEFAULTS.items():
            assert node.get(name) == default, name


@pytest.mark.parametrize(
    ("props", "message"),
    [
        ({"scroll_snap_settle_ms": -1}, "milliseconds"),
        ({"scroll_snap_duration_ms": 10**9}, "milliseconds"),
        ({"scroll_snap_easing": "bounce"}, "a preset name"),
        ({"scroll_snap_easing": (0.1, 0.2)}, "cubic bezier"),
        ({"fling_tau_ms": 0}, "milliseconds"),
        ({"fling_stop_speed": 0}, "a number from"),
        ({"fling_min_speed": -5}, "a number from"),
    ],
)
def test_bad_snap_and_fling_values_are_rejected(props, message):
    window = Window(width=300, height=300)
    window.root.set(padding=0)
    for node in view_and_list(window):
        with pytest.raises(ValueError, match=message):
            node.set(**props)


def strip(**props):
    window = Window(width=400, height=200)
    scroller = add(window, "scroll_view", orientation="horizontal", width=100, height=60, **props)
    row = window.create("box", flex_direction="horizontal", fill=(0, 0, 0, 0))
    scroller.add_child(row)
    for _ in range(5):
        row.add_child(window.create("box", width=100, height=40, fill=(255, 0, 0, 255)))
    scroller.set(scroll_snap="start")
    window.advance(0)
    return window, scroller


def wheel(window, scroller, dx):
    window.simulate("wheel", node=scroller, delta_x=dx, delta_y=0)


def test_the_snap_settle_wait_and_duration_are_the_properties_set():
    window, scroller = strip(scroll_snap_settle_ms=500, scroll_snap_duration_ms=1000, scroll_snap_easing="linear")
    wheel(window, scroller, 140)
    window.advance(0)
    window.advance(400)
    assert scroller.get("scroll_offset") == pytest.approx(140), "still waiting out the settle"
    window.advance(100)  # 500 ms after the last input: the settle starts
    window.advance(500)
    assert scroller.get("scroll_offset") == pytest.approx(120, abs=1.0), "half way, linearly"
    window.advance(1000)
    assert scroller.get("scroll_offset") == pytest.approx(100)


def test_the_default_snap_settles_by_120_plus_200_ms():
    window, scroller = strip()
    wheel(window, scroller, 140)
    window.advance(0)
    window.advance(100)
    assert scroller.get("scroll_offset") == pytest.approx(140), "120 ms not yet"
    window.advance(50)  # 150 ms after the last input: the settle starts
    window.advance(100)
    assert 100 < scroller.get("scroll_offset") < 140, "mid-settle"
    window.advance(200)
    assert scroller.get("scroll_offset") == pytest.approx(100)


def test_a_snap_takes_no_time_under_reduced_motion_whatever_its_duration():
    window, scroller = strip(scroll_snap_duration_ms=1000)
    window.set(reduced_motion=True)
    wheel(window, scroller, 140)
    window.advance(0)
    window.advance(125)
    window.advance(5)
    assert scroller.get("scroll_offset") == pytest.approx(100), "no 1000 ms settle"


def flick(window, y0=150, y1=40, steps=5, ms=16):
    window.simulate("touch_start", x=100, y=y0)
    for i in range(1, steps + 1):
        window.advance(ms)
        window.simulate("touch_move", x=100, y=y0 + (y1 - y0) * i / steps)
    window.simulate("touch_end", x=100, y=y1)


def coast(**props):
    window = Window(width=200, height=200)
    window.root.set(padding=0)
    view = add(window, "scroll_view", width=200, height=200, **props)
    view.add_child(window.create("box", width=200, height=3000))
    window.snapshot()
    flick(window)
    at_lift = view.get("scroll_offset")
    window.advance(5000)
    return view.get("scroll_offset") - at_lift


def test_fling_tau_ms_sets_how_far_a_flick_coasts_and_fling_min_speed_can_forbid_it():
    default = coast()
    assert default > 100
    assert coast(fling_tau_ms=700) > default * 1.5
    assert coast(fling_tau_ms=20) < default / 3
    assert coast(fling_min_speed=100000) == pytest.approx(0, abs=0.5), "never coasts"
