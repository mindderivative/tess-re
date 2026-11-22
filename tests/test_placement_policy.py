"""0.6.12 (P10): the placement policy of layers (`show_layer`) and popups as properties.

Headless: layers are placed by the layout, so `layout_x`/`layout_y`/`layer_placement` read the result;
a popup's `edge_margin` is validated and read back here, and what a compositor does with it is in
`test_popup_edge_margin_live.py`.
"""

from __future__ import annotations

import sys
from typing import Any

import pytest

import tess_re

W, H = 400, 300


def window() -> tess_re.Window:
    w = tess_re.Window(W, H, "placement")
    w.root.set(padding=0, gap=0)
    return w


def trigger(w: tess_re.Window, x: float, y: float) -> tess_re.Node:
    node = w.create("box", width=60, height=30, position="absolute", x=x, y=y)
    w.root.add_child(node)
    return node


def layer(w: tess_re.Window, **props: Any) -> tess_re.Node:
    return w.create("box", **{"width": 120, "height": 90, **props})


def at(node: tess_re.Node) -> tuple[float, float]:
    return node.get("layout_x"), node.get("layout_y")


def test_the_defaults_are_what_layers_always_did() -> None:
    w = window()
    assert w.get("layer_viewport_margin") == (0, 0, 0, 0)
    assert w.get("layer_flip_threshold") == 0
    menu = layer(w)
    w.show_layer(menu, anchor=trigger(w, 350, 100))
    assert menu.get("layer_placement") == "below"
    assert at(menu) == (W - 120, 130), "shifted into the window, flush with its edge"
    assert menu.get("layer_viewport_margin") == (0, 0, 0, 0)
    assert menu.get("layer_flip_threshold") == 0
    assert menu.get("layer_align") == "start"
    assert menu.get("layer_flip") is True
    assert menu.get("layer_shift") is True


def test_the_window_margin_keeps_every_layer_off_the_edge() -> None:
    w = window()
    w.set(layer_viewport_margin=8)
    assert w.get("layer_viewport_margin") == (8, 8, 8, 8)
    menu = layer(w)
    w.show_layer(menu, anchor=trigger(w, 350, 100))
    assert at(menu) == (W - 120 - 8, 130)
    w.set(layer_viewport_margin=(1, 2, 3, 4))
    assert w.get("layer_viewport_margin") == (1, 2, 3, 4), "(top, right, bottom, left)"
    assert at(menu) == (W - 120 - 2, 130), "placed again when the window's value changes"
    w.set(layer_viewport_margin=None)
    assert at(menu) == (W - 120, 130)


def test_a_margin_makes_the_layer_flip_before_it_would_cover_its_anchor() -> None:
    w = window()
    anchor = trigger(w, 100, 170)  # 100 px below it: room for the 90 px layer with 10 to spare
    menu = layer(w)
    w.show_layer(menu, anchor=anchor)
    assert menu.get("layer_placement") == "below"
    w.set(layer_viewport_margin=20)
    assert menu.get("layer_placement") == "above", "10 px short once the margin is kept"
    assert menu.get("layout_y") == 170 - 90


def test_a_flip_threshold_asks_for_room_beyond_the_layer() -> None:
    w = window()
    anchor = trigger(w, 100, 170)
    menu = layer(w)
    w.show_layer(menu, anchor=anchor)
    assert menu.get("layer_placement") == "below"
    w.set(layer_flip_threshold=15)
    assert menu.get("layer_placement") == "above"
    w.set(layer_flip_threshold=None)
    assert menu.get("layer_placement") == "below"


def test_a_layer_can_override_the_window_and_follow_it_again() -> None:
    w = window()
    w.set(layer_viewport_margin=8)
    menu = layer(w)
    w.show_layer(menu, anchor=trigger(w, 350, 100), viewport_margin=(0, 0, 0, 0), flip_threshold=0)
    assert at(menu)[0] == W - 120
    menu.set(layer_viewport_margin=2)
    assert at(menu)[0] == W - 120 - 2
    assert menu.get("layer_viewport_margin") == (2, 2, 2, 2)
    menu.set(layer_viewport_margin=None, layer_flip_threshold=None)
    assert at(menu)[0] == W - 120 - 8, "None follows the window again"


def test_align_lines_the_layer_up_with_the_anchor() -> None:
    w = window()
    anchor = trigger(w, 100, 50)  # 60 wide: a 120 px layer
    menu = layer(w)
    w.show_layer(menu, anchor=anchor)
    assert menu.get("layout_x") == 100
    menu.set(layer_align="center")
    assert menu.get("layout_x") == 100 - 30
    menu.set(layer_align="end")
    assert menu.get("layout_x") == 100 + 60 - 120
    side = layer(w)
    lower = trigger(w, 100, 100)
    w.show_layer(side, anchor=lower, placement="end", align="end")
    assert side.get("layout_x") == 160
    assert side.get("layout_y") == 100 + 30 - 90, "its bottom edge on the anchor's"
    assert side.get("layer_align") == "end"


def test_shift_and_flip_are_separate_switches() -> None:
    w = window()
    anchor = trigger(w, 350, 270)
    pinned = layer(w)
    w.show_layer(pinned, anchor=anchor, flip=False)
    assert pinned.get("layer_placement") == "below"
    assert (pinned.get("layer_flip"), pinned.get("layer_shift")) == (False, False), "flip=False pins both, as before"
    assert at(pinned) == (350, 300)
    shifted = layer(w)
    w.show_layer(shifted, anchor=anchor, flip=False, shift=True)
    assert shifted.get("layer_placement") == "below"
    assert at(shifted) == (W - 120, H - 90), "not flipped, still moved inside the window"
    flipped = layer(w)
    w.show_layer(flipped, anchor=anchor, shift=False)
    assert flipped.get("layer_placement") == "above"
    assert at(flipped) == (350, 270 - 90), "flipped, left where it is"
    flipped.set(layer_shift=True)
    assert at(flipped)[0] == W - 120


def test_a_point_placement_ignores_the_policy() -> None:
    w = window()
    w.set(layer_viewport_margin=40, layer_flip_threshold=500)
    badge = w.create("box", width=16, height=16)
    w.show_layer(badge, anchor=trigger(w, 0, 0), placement="top_start")
    assert at(badge) == (-8, -8)


@pytest.mark.parametrize(
    "props",
    [
        {"layer_viewport_margin": -1},
        {"layer_viewport_margin": "wide"},
        {"layer_viewport_margin": (1, 2, 3)},
        {"layer_viewport_margin": (1, 2, 3, -4)},
        {"layer_viewport_margin": float("nan")},
        {"layer_viewport_margin": True},
        {"layer_flip_threshold": -0.5},
        {"layer_flip_threshold": "x"},
        {"layer_flip_threshold": float("inf")},
        {"ime_offset": 5},
        {"ime_offset": (1.0,)},
        {"ime_offset": (float("nan"), 0)},
    ],
)
def test_a_bad_window_value_raises_and_changes_nothing(props: dict[str, Any]) -> None:
    w = window()
    w.set(layer_viewport_margin=3, layer_flip_threshold=2)
    with pytest.raises(ValueError, match=next(iter(props))):
        w.set(**props)
    assert w.get("layer_viewport_margin") == (3, 3, 3, 3)
    assert w.get("layer_flip_threshold") == 2


@pytest.mark.parametrize(
    "args",
    [
        {"viewport_margin": -1},
        {"viewport_margin": (1, 2)},
        {"flip_threshold": -1},
        {"flip_threshold": "soon"},
        {"align": "middle"},
        {"align": 3},
    ],
)
def test_a_bad_show_layer_argument_raises(args: dict[str, Any]) -> None:
    w = window()
    menu = layer(w)
    with pytest.raises(ValueError, match=next(iter(args))):
        w.show_layer(menu, anchor=trigger(w, 10, 10), **args)
    assert menu.parent() is None and not menu.get("layer_placement"), "nothing was shown"


def test_the_layer_properties_need_a_shown_layer() -> None:
    w = window()
    plain = w.create("box", width=10, height=10)
    w.root.add_child(plain)
    assert plain.get("layer_viewport_margin") is None
    assert plain.get("layer_align") is None
    with pytest.raises(ValueError, match="show_layer"):
        plain.set(layer_align="end")
    with pytest.raises(ValueError, match="show_layer"):
        w.create("box", layer_flip=False)
    menu = layer(w)
    w.show_layer(menu, anchor=plain)
    with pytest.raises(ValueError, match="layer_align"):
        menu.set(layer_align="middle")
    with pytest.raises(ValueError, match="layer_flip"):
        menu.set(layer_flip="no")
    w.hide_layer(menu)
    with pytest.raises(ValueError, match="show_layer"):
        menu.set(layer_shift=False)


def test_a_bad_value_in_a_mixed_set_changes_nothing() -> None:
    w = window()
    menu = layer(w)
    w.show_layer(menu, anchor=trigger(w, 10, 10))
    with pytest.raises(ValueError):
        menu.set(layer_align="end", layer_viewport_margin=-3)
    assert menu.get("layer_align") == "start"


def test_ime_offset_defaults_to_nothing_and_reads_back() -> None:
    w = window()
    assert w.get("ime_offset") == (0.0, 0.0)
    w.set(ime_offset=(3, -4.5))
    assert w.get("ime_offset") == (3.0, -4.5)
    w.set(ime_offset=None)
    assert w.get("ime_offset") == (0.0, 0.0)


@pytest.mark.skipif(not sys.platform.startswith("linux"), reason="layer surfaces are a Linux feature")
def test_a_layer_window_and_a_popup_take_the_same_window_keys(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
    monkeypatch.delenv("DISPLAY", raising=False)
    bar = tess_re.LayerWindow("test.bar", anchor=("top", "left", "right"), size=(0, 32))
    bar.set(layer_viewport_margin=6, ime_offset=(0, 4))
    assert bar.get("layer_viewport_margin") == (6, 6, 6, 6)
    assert bar.get("ime_offset") == (0.0, 4.0)
    popup = tess_re.PopupWindow(bar, size=(160, 90), anchor_rect=(100, 0, 60, 32))
    popup.set(layer_flip_threshold=5)
    assert popup.get("layer_flip_threshold") == 5
