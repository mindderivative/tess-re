"""0.6.12 (P8): the terminal's font family, line height, cursor (`caret_shape`, `caret_width`,
`caret_blink`, `caret_unfocused`), bold weight, dim opacity, underline geometry, and selection
foreground and radius. Every default is what the terminal always drew; bad values raise
`ValueError`; the pixels are checked in engine-render's `terminal_appearance_paint` tests."""

import sys

import pytest

from tess_re import MONOSPACE_FONT_FAMILY, Window
from helpers import add

pytestmark = pytest.mark.skipif(sys.platform == "win32", reason="Terminal is POSIX-only in this v1")

GREEN = (0, 255, 0, 255)

DEFAULTS = {
    "caret_shape": "block",
    "caret_width": 1.5,
    "caret_blink": None,
    "caret_unfocused": "hidden",
    "bold_weight": 700.0,
    "dim_opacity": 0.6,
    "underline_offset": None,
    "underline_thickness": None,
    "line_height": None,
    "selection_foreground": None,
    "selection_radius": 0.0,
    "font_family": MONOSPACE_FONT_FAMILY,
}


def terminal(window, **props):
    return add(window, "terminal", shell="/bin/sh", cols=20, rows=4, **props)


def test_every_default_is_what_the_terminal_always_drew():
    window = Window(width=400, height=300)
    term = terminal(window)
    for name, value in DEFAULTS.items():
        assert term.get(name) == pytest.approx(value), name


def test_set_get_and_reset():
    window = Window(width=400, height=300)
    term = terminal(window)
    changes = {
        "caret_shape": "bar",
        "caret_width": 3.0,
        "caret_blink": 530.0,
        "caret_unfocused": "outline",
        "bold_weight": 600.0,
        "dim_opacity": 0.4,
        "underline_offset": 12.0,
        "underline_thickness": 2.0,
        "line_height": 1.5,
        "selection_foreground": GREEN,
        "selection_radius": 3.0,
        "font_family": "Roboto",
    }
    term.set(**changes)
    for name, value in changes.items():
        assert term.get(name) == pytest.approx(value), name
    term.set(caret_blink=None, underline_offset=None, underline_thickness=None,
             line_height=None, selection_foreground=None)
    assert term.get("caret_blink") is None
    assert term.get("underline_offset") is None
    assert term.get("underline_thickness") is None
    assert term.get("line_height") is None
    assert term.get("selection_foreground") is None


def test_they_are_constructor_arguments():
    window = Window(width=400, height=300)
    term = terminal(window, caret_shape="underline", line_height=2, bold_weight=500)
    assert term.get("caret_shape") == "underline"
    assert term.get("line_height") == 2.0
    assert term.get("bold_weight") == 500.0


def test_the_box_follows_the_line_height_and_the_font_family():
    window = Window(width=600, height=600)
    term = terminal(window, font_size=20)
    natural = term.get("layout_height")
    term.set(line_height=2.0)
    assert term.get("layout_height") == pytest.approx(4 * 20 * 2.0)
    term.set(line_height=None)
    assert term.get("layout_height") == pytest.approx(natural)
    width = term.get("layout_width")
    term.set(font_family="Roboto")
    assert term.get("layout_width") != width, "the cell width is the family's"


@pytest.mark.parametrize(
    "props, message",
    [
        ({"caret_shape": "wavy"}, "caret_shape"),
        ({"caret_width": -1}, "caret_width"),
        ({"caret_blink": -5}, "caret_blink"),
        ({"caret_unfocused": "dotted"}, "caret_unfocused.*hidden, outline"),
        ({"caret_unfocused": None}, "caret_unfocused"),
        ({"bold_weight": 0}, "bold_weight.*1 to 1000"),
        ({"bold_weight": 1001}, "bold_weight"),
        ({"bold_weight": True}, "bold_weight"),
        ({"dim_opacity": 1.5}, "dim_opacity.*0 to 1"),
        ({"dim_opacity": -0.1}, "dim_opacity"),
        ({"underline_offset": -1}, "underline_offset.*non-negative"),
        ({"underline_thickness": "thin"}, "underline_thickness"),
        ({"line_height": 0}, "line_height"),
        ({"line_height": -1}, "line_height"),
        ({"font_family": ""}, "font_family"),
        ({"selection_radius": -2}, "selection_radius"),
        ({"selection_foreground": "green"}, "selection_foreground"),
    ],
)
def test_bad_values_raise_value_error(props, message):
    window = Window(width=400, height=300)
    term = terminal(window)
    with pytest.raises(ValueError, match=message):
        term.set(**props)
    for name in props:
        assert term.get(name) == pytest.approx(DEFAULTS[name]), "nothing changed"


@pytest.mark.parametrize(
    "name, value",
    [
        ("caret_unfocused", "outline"),
        ("bold_weight", 600),
        ("dim_opacity", 0.5),
        ("underline_offset", 3),
        ("underline_thickness", 2),
    ],
)
def test_terminal_only_properties_are_refused_elsewhere(name, value):
    window = Window(width=400, height=300)
    box = window.create("box")
    with pytest.raises(ValueError, match="applies only to a terminal node"):
        box.set(**{name: value})
    with pytest.raises(ValueError, match="applies only to a terminal node"):
        box.get(name)


def test_a_terminal_has_no_selection_fill_its_palette_is_the_highlight():
    window = Window(width=400, height=300)
    term = terminal(window)
    with pytest.raises(ValueError, match="selection_fill"):
        term.set(selection_fill=GREEN)
    term.set(palette={"selection": GREEN})
    assert term.get("palette")["selection"] == GREEN


def test_set_defaults_takes_them():
    window = Window(width=400, height=300)
    window.set_defaults("terminal", caret_shape="bar", dim_opacity=0.5)
    term = terminal(window)
    assert term.get("caret_shape") == "bar"
    assert term.get("dim_opacity") == pytest.approx(0.5)
