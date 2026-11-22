"""0.6.11 (P5): `Window(first_present=..., background=...)` and `window.ready()`, headless.

What a window does with them while it is open (no buffer is committed before `ready()`, the
colour of the first frame) needs a compositor: `test_first_present_live.py`. These are the API:
the arguments, their validation, `get`/`set`, and `ready()` on a window that is not open.
"""

import sys

import pytest

from tess_re import LayerWindow, PopupWindow, Window


def test_a_window_presents_immediately_and_clears_to_the_scheme_by_default():
    w = Window()
    assert w.get("first_present") == "immediate"
    assert w.get("background") is None
    assert w.get("first_presented") is False


@pytest.mark.parametrize("mode", ["immediate", "on_ready", 1, 250, 600_000])
def test_first_present_takes_a_name_or_milliseconds(mode):
    assert Window(first_present=mode).get("first_present") == mode


def test_none_is_the_default_mode():
    assert Window(first_present=None).get("first_present") == "immediate"


@pytest.mark.parametrize(
    "bad",
    ["later", "ON_READY", "", 0, -5, 600_001, 1.5, True, False, [], (1,), b"on_ready"],
)
def test_a_bad_first_present_names_the_choices(bad):
    with pytest.raises(
        ValueError, match="first_present must be 'immediate', 'on_ready' or"
    ):
        Window(first_present=bad)


def test_first_present_is_fixed_when_the_window_is_made():
    w = Window()
    with pytest.raises(ValueError, match="only be given when the window is made"):
        w.set(first_present="on_ready")
    assert w.get("first_present") == "immediate"


def test_first_presented_is_read_only():
    with pytest.raises(ValueError, match="read-only"):
        Window().set(first_presented=True)


def test_background_is_a_colour_or_none_in_the_constructor_and_in_set():
    w = Window(background=(10, 20, 30, 255))
    assert w.get("background") == (10, 20, 30, 255)
    w.set(background=(1, 2, 3, 128))
    assert w.get("background") == (1, 2, 3, 128)
    w.set(background=None)
    assert w.get("background") is None
    assert Window(background=None).get("background") is None


@pytest.mark.parametrize("bad", ["red", (1, 2, 3), (1, 2, 3, 256), 5, [1, 2]])
def test_a_bad_background_is_refused(bad):
    with pytest.raises(ValueError, match="background"):
        Window(background=bad)
    w = Window()
    with pytest.raises(ValueError, match="background"):
        w.set(background=bad)
    assert w.get("background") is None


def test_ready_before_the_window_is_open_is_harmless_and_repeatable():
    for mode in ("immediate", "on_ready", 100):
        w = Window(first_present=mode)
        assert w.ready() is None
        assert w.ready() is None
        # Nothing was presented: the window is not open.
        assert w.get("first_presented") is False


def test_a_headless_window_still_snapshots_while_its_first_present_is_held():
    """The hold is a live-loop matter: layout and the headless pipeline are untouched."""
    w = Window(
        width=100, height=60, first_present="on_ready", background=(0, 0, 0, 255)
    )
    w.root.add_child(w.create("box", width=40, height=20, fill=(200, 0, 0, 255)))
    w.advance(0.0)
    assert w.snapshot() is not None


@pytest.mark.skipif(
    not sys.platform.startswith("linux"), reason="layer surfaces are a Linux feature"
)
class TestLayerAndPopup:
    @pytest.fixture(autouse=True)
    def headless(self, monkeypatch):
        monkeypatch.delenv("WAYLAND_DISPLAY", raising=False)
        monkeypatch.delenv("DISPLAY", raising=False)

    def bar(self, **kw):
        return LayerWindow(
            "p5.bar", anchor=("top", "left", "right"), size=(0, 24), **kw
        )

    @pytest.mark.parametrize("mode", [None, "immediate"])
    def test_a_layer_window_accepts_immediate(self, mode):
        assert self.bar(first_present=mode).get("first_present") == "immediate"

    @pytest.mark.parametrize("mode", ["on_ready", 1, 100, 600_000])
    def test_a_layer_window_takes_on_ready_and_milliseconds(self, mode):
        assert self.bar(first_present=mode).get("first_present") == mode

    @pytest.mark.parametrize(
        "bad",
        ["soon", "ON_READY", "", 0, -5, 600_001, 1.5, True, False, [], b"on_ready"],
    )
    def test_a_bad_first_present_is_refused_on_a_layer_window(self, bad):
        with pytest.raises(
            ValueError, match="first_present must be 'immediate', 'on_ready' or"
        ):
            self.bar(first_present=bad)

    def test_a_popup_takes_every_mode_and_refuses_a_bad_one(self):
        parent = self.bar()
        kw = dict(size=(40, 40), anchor_rect=(0, 0, 10, 10))
        for mode, want in (
            (None, "immediate"),
            ("immediate", "immediate"),
            ("on_ready", "on_ready"),
            (250, 250),
        ):
            assert (
                PopupWindow(parent, first_present=mode, **kw).get("first_present")
                == want
            )
        for bad in ("soon", 0, 600_001, True):
            with pytest.raises(ValueError, match="first_present must be"):
                PopupWindow(parent, first_present=bad, **kw)

    def test_first_present_cannot_be_set_later_on_either(self):
        bar = self.bar(first_present="on_ready")
        with pytest.raises(ValueError, match="first_present"):
            bar.set(first_present="immediate")
        assert bar.get("first_present") == "on_ready"

    def test_ready_is_harmless_before_during_and_after_opening_and_repeatable(self):
        for mode in ("immediate", "on_ready", 100):
            bar = self.bar(first_present=mode)
            popup = PopupWindow(
                bar, size=(40, 40), anchor_rect=(0, 0, 10, 10), first_present=mode
            )
            for w in (bar, popup):
                assert w.ready() is None
                assert w.ready() is None
                assert w.get("first_presented") is False
                assert w.get("first_present") == mode

    def test_a_held_layer_window_still_lays_out_and_snapshots(self):
        bar = self.bar(first_present="on_ready", background=(5, 6, 7, 255))
        bar.root.add_child(
            bar.create("box", width=20, height=10, fill=(200, 0, 0, 255))
        )
        assert bar.snapshot() is not None

    @pytest.mark.parametrize("make", ["bar", "popup"])
    def test_background_is_a_constructor_argument_of_layer_and_popup(self, make):
        def build(**kw):
            bar = self.bar()
            if make == "bar":
                return self.bar(**kw)
            return PopupWindow(bar, size=(40, 40), anchor_rect=(0, 0, 10, 10), **kw)

        assert build().get("background") is None
        assert build(background=(1, 2, 3, 255)).get("background") == (1, 2, 3, 255)
        assert build(background=None).get("background") is None
        with pytest.raises(ValueError, match="background"):
            build(background=(1, 2, 3))

    def test_ready_on_a_layer_window_is_a_no_op_when_it_presents_immediately(self):
        assert self.bar().ready() is None

    def test_a_layer_window_background_can_be_set(self):
        w = self.bar()
        w.set(background=(1, 2, 3, 255))
        assert w.get("background") == (1, 2, 3, 255)
