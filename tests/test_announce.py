"""TRE-REQ-021: `Window.announce(text, politeness)` takes its arguments strictly.

What a screen reader hears is tested in `test_layer_window_live.py` (a real AT-SPI bus); here only the
call itself, which needs no display and no reader: nothing is heard unless one is connected.
"""

import pytest

from tess_re import LayerWindow, PopupWindow, Window


def test_announce_accepts_both_politenesses_and_returns_nothing():
    window = Window()
    assert window.announce("Connected") is None
    assert window.announce("Battery low", "assertive") is None
    assert window.announce("Same again", politeness="polite") is None


@pytest.mark.parametrize("politeness", ["loud", "", "POLITE", None])
def test_another_politeness_is_refused_and_says_which(politeness):
    with pytest.raises((ValueError, TypeError)):
        Window().announce("x", politeness)


@pytest.mark.parametrize("text", ["", "   ", "\n"])
def test_an_empty_text_is_refused(text):
    with pytest.raises(ValueError, match="must not be empty"):
        Window().announce(text)


def test_it_is_on_a_layer_window_and_a_popup_too():
    assert callable(LayerWindow.announce) and callable(PopupWindow.announce)
