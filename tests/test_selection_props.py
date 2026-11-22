"""0.6.12 (P8): `selection_fill` on a static `text` (as on `text_input`), `selection_foreground`
and `selection_radius` on both, and `caret_width` on a text input. The defaults are what was
painted before (`None`, `None`, `0`), a bad value raises `ValueError`, `set_defaults` takes them,
and the painted pixels are checked in engine-render's `selection_paint` tests."""

import pytest

from tess_re import Window
from helpers import add

GREEN = (0, 255, 0, 255)


def make(window, kind, **props):
    """A node of `kind`: a `text` needs its content."""
    return window.create(kind, **({"text": "Hello"} if kind == "text" else {}), **props)

WHITE = (255, 255, 255, 255)


@pytest.mark.parametrize("kind", ["text", "text_input"])
def test_selection_props_default_to_what_was_painted_before(kind):
    window = Window(width=200, height=100)
    node = make(window, kind)
    assert node.get("selection_fill") is None
    assert node.get("selection_foreground") is None
    assert node.get("selection_radius") == 0.0


@pytest.mark.parametrize("kind", ["text", "text_input"])
def test_selection_props_set_get_and_reset(kind):
    window = Window(width=200, height=100)
    node = make(window, kind)
    node.set(selection_fill=(0, 0, 255, 80), selection_foreground=WHITE, selection_radius=3.5)
    assert node.get("selection_fill") == (0, 0, 255, 80)
    assert node.get("selection_foreground") == WHITE
    assert node.get("selection_radius") == 3.5
    node.set(selection_fill=None, selection_foreground=None, selection_radius=0)
    assert node.get("selection_fill") is None
    assert node.get("selection_foreground") is None
    assert node.get("selection_radius") == 0.0


@pytest.mark.parametrize("kind", ["text", "text_input"])
def test_selection_props_take_constructor_arguments(kind):
    window = Window(width=200, height=100)
    node = make(window, kind, selection_fill=GREEN, selection_foreground=WHITE, selection_radius=2)
    assert node.get("selection_fill") == GREEN
    assert node.get("selection_foreground") == WHITE
    assert node.get("selection_radius") == 2.0


@pytest.mark.parametrize(
    "props, message",
    [
        ({"selection_fill": "green"}, "selection_fill"),
        ({"selection_foreground": 3}, "selection_foreground"),
        ({"selection_foreground": (1, 2)}, "selection_foreground"),
        ({"selection_radius": -1}, "selection_radius.*non-negative"),
        ({"selection_radius": "round"}, "selection_radius.*non-negative"),
        ({"selection_radius": float("inf")}, "selection_radius.*non-negative"),
        ({"selection_radius": None}, "selection_radius.*non-negative"),
    ],
)
def test_selection_props_reject_bad_values(props, message):
    window = Window(width=200, height=100)
    node = make(window, "text")
    with pytest.raises(ValueError, match=message):
        node.set(**props)


@pytest.mark.parametrize("name", ["selection_fill", "selection_foreground", "selection_radius"])
def test_selection_props_apply_only_to_text_and_text_input(name):
    window = Window(width=200, height=100)
    box = window.create("box")
    value = 3 if name == "selection_radius" else GREEN
    with pytest.raises(ValueError, match="applies only to a text or text_input node"):
        box.set(**{name: value})
    with pytest.raises(ValueError, match="applies only to a text or text_input node"):
        box.get(name)


def test_a_static_text_selection_fill_does_not_touch_the_selection_itself():
    window = Window(width=300, height=100)
    label = add(window, "text", text="Hello world", selectable=True)
    label.set(selection=(0, 5), selection_fill=GREEN, selection_foreground=WHITE)
    assert label.get("selection") == (0, 5)
    assert label.get("text") == "Hello world"


def test_set_defaults_takes_the_selection_props():
    window = Window(width=200, height=100)
    window.set_defaults("text", selection_fill=GREEN, selection_radius=4)
    window.set_defaults("text_input", selection_foreground=WHITE)
    label = make(window, "text")
    field = make(window, "text_input")
    assert label.get("selection_fill") == GREEN
    assert label.get("selection_radius") == 4.0
    assert field.get("selection_foreground") == WHITE
    assert make(window, "text").get("selection_foreground") is None


def test_caret_width_default_is_one_and_a_half_pixels_and_is_settable():
    window = Window(width=200, height=100)
    field = window.create("text_input")
    assert field.get("caret_width") == 1.5
    field.set(caret_width=4)
    assert field.get("caret_width") == 4.0
    assert window.create("text_input", caret_width=0).get("caret_width") == 0.0
    with pytest.raises(ValueError, match="caret_width"):
        field.set(caret_width=-2)
