"""A focused terminal sends every key to its shell, so a keyboard user could
never leave it (WCAG 2.1.2). `escape_chord` leaves it: focus moves on, the key
is not sent, and `terminal_escape` fires. Headless; the shell is a script that
logs what it receives, so `window.simulate` keys can be checked end to end.
"""

import os
import stat
import sys
import time

import pytest

from tess_re import Window
from helpers import add

pytestmark = pytest.mark.skipif(sys.platform == "win32", reason="Terminal is POSIX-only in this v1")

BG = {"background": (0, 0, 0, 255)}


def make_window(tmp_path, **props):
    """A terminal whose shell appends what it reads (`cat -v`) to a file, then two buttons."""
    log = tmp_path / "log"
    script = tmp_path / "sh.sh"
    script.write_text(f"#!/bin/sh\nexec cat -v > {log}\n")
    script.chmod(script.stat().st_mode | stat.S_IEXEC)
    window = Window(width=400, height=300)
    before = add(window, "box", width=50, height=20, focusable=True)
    term = add(window, "terminal", shell=str(script), cols=20, rows=4, palette=BG, **props)
    after = add(window, "box", width=50, height=20, focusable=True)
    return window, before, term, after, log


def logged(log, want, timeout=5.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        text = log.read_text() if log.exists() else ""
        if want in text:
            return text
        time.sleep(0.05)
    return log.read_text() if log.exists() else ""


def test_default_and_getter(tmp_path):
    _, _, term, _, _ = make_window(tmp_path)
    assert term.get("escape_chord") == "ctrl+shift+escape"
    assert term.get("escape_chord_reverse") is None
    term.set(escape_chord="Alt+Ctrl+Tab", escape_chord_reverse="ctrl+alt+shift+tab")
    assert term.get("escape_chord") == "ctrl+alt+tab"
    term.set(escape_chord=None)
    assert term.get("escape_chord") is None


def test_invalid_chords_raise_listing_the_format(tmp_path):
    _, _, term, _, _ = make_window(tmp_path)
    for bad in ("ctrl+shift+nope", "", "banana+escape", "k", 5):
        with pytest.raises((ValueError,)) as err:
            term.set(escape_chord=bad)
        assert "ctrl+shift+escape" in str(err.value) or "chord" in str(err.value)
    with pytest.raises(ValueError, match="format"):
        term.set(escape_chord="ctrl+")


def test_chord_moves_focus_forward_and_is_not_sent(tmp_path):
    window, before, term, after, log = make_window(tmp_path)
    seen = []
    term.on("terminal_escape", lambda e: seen.append((e.key, e.related_target)))
    focus = []
    after.on("focus", lambda e: focus.append(e.focus_visible))
    window.simulate("click", node=term)
    assert term.get("focused") is True
    window.simulate("key_down", key="escape", ctrl=True, shift=True)
    assert after.get("focused") is True
    assert term.get("focused") is False
    assert len(seen) == 1 and seen[0][0] == "ctrl+shift+escape" and seen[0][1] == after
    assert focus == [True]
    # Now back in the terminal, type; the log holds the text and no escape byte.
    window.simulate("click", node=term)
    window.simulate("input", text="hello")
    window.simulate("key_down", key="enter")
    text = logged(log, "hello")
    assert "hello" in text and "^[" not in text


def test_plain_escape_and_ctrl_escape_still_reach_the_shell(tmp_path):
    window, before, term, after, log = make_window(tmp_path)
    window.simulate("click", node=term)
    window.simulate("key_down", key="escape")
    window.simulate("key_down", key="escape", ctrl=True)
    window.simulate("key_down", key="escape", ctrl=True, shift=True, alt=True)
    assert term.get("focused") is True
    window.simulate("input", text="x")
    window.simulate("key_down", key="enter")
    assert logged(log, "^[^[^[x").count("^[") == 3


def test_reverse_chord_and_disabled(tmp_path):
    window, before, term, after, log = make_window(tmp_path, escape_chord_reverse="ctrl+alt+tab")
    window.simulate("click", node=term)
    window.simulate("key_down", key="tab", ctrl=True, alt=True)
    assert before.get("focused") is True
    term.set(escape_chord=None)
    window.simulate("click", node=term)
    window.simulate("key_down", key="escape", ctrl=True, shift=True)
    assert term.get("focused") is True


def test_dismiss_on_escape_overlay_still_closes_on_plain_escape(tmp_path):
    window, before, term, after, log = make_window(tmp_path)
    menu = window.create("box", width=60, height=60, x=0, y=200)
    window.show_layer(menu)
    dismissed = []
    menu.on("dismiss", lambda: dismissed.append(1))
    # The chord leaves the terminal and does not dismiss the overlay.
    window.simulate("focus", node=term)
    window.simulate("key_down", key="escape", ctrl=True, shift=True)
    assert after.get("focused") is True and dismissed == []
    # A plain Escape outside a terminal still dismisses it.
    window.simulate("focus", node=before)
    window.simulate("key_down", key="escape")
    assert dismissed == [1]
