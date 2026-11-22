"""0.6.12 (P11): the window's title-bar, resize-border and shadow behaviour are properties.
Headless: the OS moves, resizes and shades the window, but the decision -- which action a
double-click on the drag region takes, which cursor a border zone asks for -- runs under
`simulate`."""

import pytest

from tess_re import Window

DIRECTIONS = (
    "north",
    "south",
    "east",
    "west",
    "north_west",
    "north_east",
    "south_west",
    "south_east",
)
STOCK = {
    "north": "ns_resize",
    "south": "ns_resize",
    "east": "ew_resize",
    "west": "ew_resize",
    "north_west": "nwse_resize",
    "south_east": "nwse_resize",
    "north_east": "nesw_resize",
    "south_west": "nesw_resize",
}


def title_bar(window):
    bar = window.create("box", width=300, height=40, window_region="drag")
    window.root.add_child(bar)
    window.advance(0)
    return bar


def double_press(window, bar):
    for _ in range(2):
        window.simulate("pointer_down", node=bar, x=200, y=20)
        window.simulate("pointer_up", node=bar, x=200, y=20)
        window.advance(100)


def test_defaults_are_how_it_always_behaved():
    window = Window(width=400, height=300)
    assert window.get("titlebar_double_click") is None
    assert window.get("resize_cursors") == STOCK
    assert window.get("undecorated_shadow") is True


@pytest.mark.parametrize("value", ["maximize", "minimize", "none", "event", None])
def test_titlebar_double_click_reads_back(value):
    window = Window()
    window.set(titlebar_double_click=value)
    assert window.get("titlebar_double_click") == value


@pytest.mark.parametrize("value", ["toggle", "", 1, True, ["maximize"]])
def test_titlebar_double_click_is_validated(value):
    window = Window()
    with pytest.raises(ValueError, match="titlebar_double_click"):
        window.set(titlebar_double_click=value)
    assert window.get("titlebar_double_click") is None


def test_maximize_is_the_default_action_and_toggles():
    window = Window(width=400, height=300)
    bar = title_bar(window)
    double_press(window, bar)
    assert window.get("maximized") is True


def test_minimize_action():
    window = Window(width=400, height=300)
    window.set(titlebar_double_click="minimize")
    bar = title_bar(window)
    double_press(window, bar)
    assert window.get("minimized") is True
    assert window.get("maximized") is False


def test_none_action_does_nothing():
    window = Window(width=400, height=300)
    window.set(titlebar_double_click="none")
    bar = title_bar(window)
    double_press(window, bar)
    assert window.get("maximized") is False
    assert window.get("minimized") is False


def test_event_action_fires_an_event_and_does_nothing_itself():
    window = Window(width=400, height=300)
    heard = []
    window.on("titlebar_double_click", lambda e: heard.append((e.type, e.x, e.y)))
    window.set(titlebar_double_click="event")
    bar = title_bar(window)
    double_press(window, bar)
    assert [kind for kind, _, _ in heard] == ["titlebar_double_click"]
    assert all(isinstance(v, float) for v in heard[0][1:]), "x and y of the second press"
    assert window.get("maximized") is False
    # The press that made it a double-click is spent: a third is a fresh first press.
    window.simulate("pointer_down", node=bar, x=200, y=20)
    window.simulate("pointer_up", node=bar, x=200, y=20)
    assert len(heard) == 1


def test_no_event_unless_asked():
    window = Window(width=400, height=300)
    heard = []
    window.on("titlebar_double_click", lambda: heard.append(1))
    bar = title_bar(window)
    double_press(window, bar)
    assert heard == []


def test_resize_cursors_override_and_reset():
    window = Window()
    window.set(resize_cursors={"north": "move", "south_east": "crosshair"})
    cursors = window.get("resize_cursors")
    assert cursors["north"] == "move"
    assert cursors["south_east"] == "crosshair"
    assert cursors["east"] == "ew_resize", "directions left out keep the stock cursor"
    window.set(resize_cursors={"west": "col_resize"})
    assert window.get("resize_cursors")["north"] == "ns_resize", "a dict replaces the last"
    window.set(resize_cursors=None)
    assert window.get("resize_cursors") == STOCK


@pytest.mark.parametrize(
    "value, match",
    [
        ("move", "dict"),
        ({"up": "move"}, "no direction"),
        ({"north": "hand"}, "cursor names"),
        ({"north": 3}, "cursor name"),
        ({1: "move"}, "direction names"),
    ],
)
def test_resize_cursors_are_validated(value, match):
    window = Window()
    with pytest.raises(ValueError, match=match):
        window.set(resize_cursors=value)
    assert window.get("resize_cursors") == STOCK


def test_undecorated_shadow_reads_back_and_is_validated():
    window = Window()
    window.set(undecorated_shadow=False)
    assert window.get("undecorated_shadow") is False
    for bad in ("no", 0, None):
        with pytest.raises(ValueError, match="`undecorated_shadow` must be a bool"):
            window.set(undecorated_shadow=bad)
    assert window.get("undecorated_shadow") is False


def test_a_failed_set_changes_nothing():
    window = Window()
    with pytest.raises(ValueError):
        window.set(titlebar_double_click="event", resize_cursors={"north": "nope"})
    assert window.get("titlebar_double_click") is None
