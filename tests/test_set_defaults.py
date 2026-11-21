"""0.6.11 (P12): `Window.set_defaults` and `App.set_defaults` declare the baseline look of each node
kind once. Defaults merge into the props `create` and `build` parse, at creation only."""

from __future__ import annotations

import pytest

import tess_re

RED = (255, 0, 0, 255)
GREEN = (0, 255, 0, 255)
BLUE = (0, 0, 255, 255)


def window() -> tess_re.Window:
    return tess_re.Window(300, 200, "defaults")


def app_with(*windows: tess_re.Window) -> tess_re.App:
    app = tess_re.App()
    for w in windows:
        app.add_window(w)
    return app


def test_a_default_applies_to_nodes_created_afterwards() -> None:
    w = window()
    w.set_defaults("text", font_size=21, fill=RED)
    label = w.create("text", text="hi")
    assert label.get("font_size") == 21
    assert label.get("fill") == RED
    # another kind is untouched
    assert w.create("box").get("fill") != RED


def test_an_explicit_property_wins() -> None:
    w = window()
    w.set_defaults("text", font_size=21)
    assert w.create("text", text="a", font_size=9).get("font_size") == 9


def test_precedence_explicit_over_window_over_app_over_engine() -> None:
    w = window()
    app = app_with(w)
    engine_default = w.create("text", text="x").get("font_size")
    app.set_defaults("text", font_size=12, font_weight=700)
    assert w.create("text", text="x").get("font_size") == 12
    w.set_defaults("text", font_size=14)
    other = window()
    app.add_window(other)
    assert w.create("text", text="x").get("font_size") == 14  # the window's own wins
    assert w.create("text", text="x").get("font_weight") == 700  # the app's still fills the rest
    assert other.create("text", text="x").get("font_size") == 12  # another window: the app's
    assert w.create("text", text="x", font_size=30).get("font_size") == 30  # explicit wins
    app.clear_defaults()
    w.clear_defaults()
    assert w.create("text", text="x").get("font_size") == engine_default


def test_window_star_beats_the_apps_kind_default() -> None:
    w = window()
    app = app_with(w)
    app.set_defaults("text", font_size=12)
    w.set_defaults("*", font_size=15)
    assert w.create("text", text="x").get("font_size") == 15
    w.set_defaults("text", font_size=16)  # within a scope the kind beats "*"
    assert w.create("text", text="x").get("font_size") == 16


def test_star_applies_where_the_property_exists_and_skips_the_rest() -> None:
    w = window()
    w.set_defaults("*", font_size=18, opacity=0.5, corner_radius=4)
    assert w.create("text", text="x").get("font_size") == 18
    assert w.create("text_input").get("font_size") == 18
    box = w.create("box")  # a box has no font_size: skipped silently
    assert box.get("opacity") == 0.5
    with pytest.raises(ValueError):
        box.get("font_size")
    assert w.create("scroll_view").get("opacity") == 0.5


def test_a_star_property_no_kind_has_raises() -> None:
    w = window()
    with pytest.raises(ValueError, match="no_such_prop"):
        w.set_defaults("*", no_such_prop=1)
    assert w.get_defaults() == {}


def test_a_kind_specific_property_the_kind_lacks_raises_and_stores_nothing() -> None:
    w = window()
    with pytest.raises(ValueError, match="font_size"):
        w.set_defaults("box", font_size=12)
    with pytest.raises(ValueError, match="settable"):
        w.set_defaults("text", colour=RED)
    with pytest.raises(ValueError, match="opacity"):
        w.set_defaults("text", fill=RED, opacity="loud")  # a bad value
    assert w.get_defaults() == {}  # atomic: `fill` was not kept either


def test_an_unknown_kind_raises_listing_the_kinds() -> None:
    w = window()
    with pytest.raises(ValueError, match="scroll_view"):
        w.set_defaults("button", fill=RED)
    with pytest.raises(ValueError):
        w.get_defaults("button")
    with pytest.raises(ValueError):
        w.clear_defaults("button")


def test_none_removes_a_default() -> None:
    w = window()
    w.set_defaults("text", font_size=21, fill=RED)
    w.set_defaults("text", font_size=None)
    assert w.get_defaults("text") == {"fill": RED}
    assert w.create("text", text="x").get("font_size") != 21
    w.set_defaults("text", fill=None)
    assert w.get_defaults() == {}
    w.set_defaults("*", opacity=0.5)
    w.set_defaults("*", opacity=None)
    assert w.create("box").get("opacity") != 0.5


def test_get_and_clear() -> None:
    w = window()
    w.set_defaults("text", font_size=21)
    w.set_defaults("box", fill=BLUE)
    assert w.get_defaults() == {"text": {"font_size": 21}, "box": {"fill": BLUE}}
    assert w.get_defaults("box") == {"fill": BLUE}
    assert w.get_defaults("image") == {}
    copy = w.get_defaults("box")
    copy["fill"] = RED
    assert w.get_defaults("box") == {"fill": BLUE}  # a copy
    w.clear_defaults("box")
    assert w.get_defaults() == {"text": {"font_size": 21}}
    w.clear_defaults()
    assert w.get_defaults() == {}


def test_a_dict_sets_a_whole_theme_in_one_call() -> None:
    w = window()
    w.set_defaults({"text": {"font_size": 20}, "*": {"opacity": 0.25}, "box": {"fill": GREEN}})
    assert w.create("text", text="x").get("font_size") == 20
    assert w.create("box").get("fill") == GREEN
    assert w.create("box").get("opacity") == 0.25
    with pytest.raises(ValueError, match="not both"):
        w.set_defaults({"text": {"font_size": 1}}, font_size=2)
    with pytest.raises(ValueError, match="dict"):
        w.set_defaults({"text": 3})
    # one bad entry stores none of the call
    with pytest.raises(ValueError):
        w.set_defaults({"text": {"font_size": 99}, "box": {"font_size": 1}})
    assert w.get_defaults("text") == {"font_size": 20}


def test_defaults_do_not_change_existing_nodes() -> None:
    w = window()
    before = w.create("text", text="x")
    size = before.get("font_size")
    w.set_defaults("text", font_size=33)
    assert before.get("font_size") == size
    assert w.create("text", text="x").get("font_size") == 33
    w.clear_defaults()
    assert before.get("font_size") == size


def test_build_honours_defaults_for_every_node_it_makes() -> None:
    w = window()
    app = app_with(w)
    app.set_defaults("text", font_weight=700)
    w.set_defaults("text", font_size=19)
    w.set_defaults("box", gap=7)
    root, ids = w.build(
        {
            "kind": "box",
            "id": "root",
            "children": [
                {"kind": "text", "id": "a", "props": {"text": "a"}},
                {"kind": "text", "id": "b", "props": {"text": "b", "font_size": 11}},
            ],
        }
    )
    assert root.get("gap") == 7
    assert ids["a"].get("font_size") == 19 and ids["a"].get("font_weight") == 700
    assert ids["b"].get("font_size") == 11  # explicit in the spec wins


def test_layout_props_are_allowed() -> None:
    w = window()
    w.set_defaults("box", padding=5, gap=3, width=40)
    box = w.create("box")
    assert (box.get("padding"), box.get("gap"), box.get("width")) == (5, 3, 40)


def test_scrollbar_defaults_on_scroll_view() -> None:
    w = window()
    w.set_defaults("scroll_view", scrollbar_fill=RED, scrollbar_width=3)
    view = w.create("scroll_view", width=100, height=100)
    assert (view.get("scrollbar_fill"), view.get("scrollbar_width")) == (RED, 3.0)
    assert w.create("scroll_view", scrollbar_width=9).get("scrollbar_width") == 9.0
    with pytest.raises(ValueError, match="scroll_view"):
        w.set_defaults("box", scrollbar_width=3)
    # "*" reaches the scroll views only
    w.clear_defaults()
    w.set_defaults("*", scrollbar_width=5)
    assert w.create("scroll_view").get("scrollbar_width") == 5.0
    w.create("box")


def test_a_terminals_scrollback_default() -> None:
    w = window()
    w.set_defaults("terminal", scrollback_lines=50, cols=20, rows=4, shell="/bin/sh")
    terminal = w.create("terminal")
    assert terminal is not None
    with pytest.raises(ValueError, match="scrollback_lines"):
        w.set_defaults("terminal", scrollback_lines=-1)
    with pytest.raises(ValueError, match="terminal"):
        w.set_defaults("box", scrollback_lines=5)


def test_a_required_property_may_come_from_a_default() -> None:
    w = window()
    with pytest.raises(ValueError, match="needs `text`"):
        w.create("text")
    w.set_defaults("text", text="placeholder")
    assert w.create("text").get("text") == "placeholder"


def test_app_defaults_reach_windows_added_to_the_app() -> None:
    app = tess_re.App()
    app.set_defaults({"text": {"font_size": 13}})
    w = window()
    assert w.create("text", text="x").get("font_size") != 13  # not added yet
    app.add_window(w)
    assert w.create("text", text="x").get("font_size") == 13
    app.set_defaults("text", font_size=None)
    assert app.get_defaults() == {}
    assert w.create("text", text="x").get("font_size") != 13


def test_app_defaults_validate_and_refuse_nodes() -> None:
    app = tess_re.App()
    w = window()
    with pytest.raises(ValueError, match="font_size"):
        app.set_defaults("box", font_size=3)
    with pytest.raises(ValueError, match="Node"):
        app.set_defaults("box", anchor=w.create("box"))
    app.set_defaults("*", opacity=0.5)
    assert app.get_defaults("*") == {"opacity": 0.5}
    app.clear_defaults("*")
    assert app.get_defaults() == {}



def test_a_default_that_refers_to_its_window_is_collected() -> None:
    import gc
    import weakref

    class Probe:
        pass

    w = window()
    probe = Probe()
    # a cycle: window -> defaults -> draw -> window and probe
    w.set_defaults("canvas", draw=lambda painter, w=w, probe=probe: None)
    ref = weakref.ref(probe)
    del w, probe
    gc.collect()
    assert ref() is None


def test_a_text_made_under_defaults_still_follows_the_window_scheme() -> None:
    # The kind constructors moved into defaults::blank_kind: the scheme flags must come with them.
    w = tess_re.Window(100, 60, "scheme", scheme="dark")
    w.set_defaults("text", font_size=20)
    label = w.create("text", text="x")
    field = w.create("text_input")
    assert label.get("fill") == tess_re.scheme_colors("dark")["on_surface"]
    w.set(scheme="light")
    assert label.get("fill") == tess_re.scheme_colors("light")["on_surface"], "unset = scheme"
    assert field.get("font_size") == 16
