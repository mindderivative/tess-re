"""0.5.10 (#246): a window the compositor is not showing must not stall the others.

On a headless sway (`tools/sway_live.sh -- python -m pytest tests/test_hidden_window_live.py`) or a headless KWin
(`tools/kwin_live.sh`, the `[kwin]` variants: a window is hidden by minimizing it, covered by fullscreening another).
Before the fix a window moved off screen left a vsync present blocked, with the
Python GIL held, so the whole app froze: every other window and every Python thread.
"""

import os
import subprocess
import sys
import textwrap
import time

import live_compositor
import pytest

pytestmark = pytest.mark.usefixtures("compositor")

CHILD = textwrap.dedent(
    """
    import sys, threading, time
    from tess_re import App, Window

    app = App()
    for title in ("hide-me", "stay"):
        window = Window(width=100, height=100, title=title)
        node = window.create("box", width=40, height=40, position="absolute", x=10, y=10,
                             fill=(200, 50, 50, 255))
        window.root.add_child(node)
        node.animate("rotation_deg", 360000.0, 3_600_000, easing="linear")
        app.add_window(window)
    handle = app.thread_handle()

    def beat():
        n = 0
        while True:
            time.sleep(0.1)
            n += 1
            print(n, flush=True)
            if n > 600:
                handle.stop()
                return

    threading.Thread(target=beat, daemon=True).start()
    app.run()
    """
)


def _grab():
    """The top 200 x 30 pixels of the screen, where the bar is."""
    return live_compositor.region(0, 0, 200, 30)


def _beats(proc, seconds):
    """How many heartbeat lines the child's Python thread printed in `seconds`."""
    os.set_blocking(proc.stdout.fileno(), False)
    count, end = 0, time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            line = proc.stdout.readline()
        except BlockingIOError:
            line = None
        if line:
            count += 1
        else:
            time.sleep(0.02)
    return count


def test_a_hidden_window_does_not_freeze_the_other_window_or_python_threads():
    proc = subprocess.Popen(
        [sys.executable, "-c", CHILD],
        stdout=subprocess.PIPE,
        text=True,
        stderr=subprocess.DEVNULL,
    )
    try:
        assert _beats(proc, 3.0) > 15, (
            "the child must be running with both windows open"
        )
        live_compositor.window_action("hide-me", "hide")
        time.sleep(1.0)
        # 2 s at one beat per 0.1 s is 20; a frozen app prints none.
        assert _beats(proc, 2.0) >= 12
        live_compositor.window_action("hide-me", "show")
        assert _beats(proc, 2.0) >= 12, "and it goes on after the window comes back"
    finally:
        proc.kill()
        proc.wait()


LAYER_CHILD = textwrap.dedent(
    """
    import threading, time
    from tess_re import App, LayerWindow, Window

    app = App()
    bar = LayerWindow("probe-bar", anchor=("top", "left", "right"), size=(0, 30),
                      exclusive_zone=0, layer="top")
    spinner = bar.create("box", width=20, height=20, position="absolute", x=5, y=5,
                         fill=(50, 200, 50, 255))
    bar.root.add_child(spinner)
    spinner.animate("rotation_deg", 360000.0, 3_600_000, easing="linear")
    app.add_window(bar)
    window = Window(width=100, height=100, title="cover")
    window.root.add_child(window.create("box", width=40, height=40, fill=(200, 50, 50, 255)))
    app.add_window(window)
    handle = app.thread_handle()

    def beat():
        for n in range(1, 600):
            time.sleep(0.1)
            print(n, flush=True)
        handle.stop()

    threading.Thread(target=beat, daemon=True).start()
    app.run()
    """
)


def test_a_layer_surface_under_a_fullscreen_window_does_not_freeze_the_app():
    proc = subprocess.Popen(
        [sys.executable, "-c", LAYER_CHILD],
        stdout=subprocess.PIPE,
        text=True,
        stderr=subprocess.DEVNULL,
    )
    try:
        assert _beats(proc, 3.0) > 15, (
            "the child must be running with both windows open"
        )
        live_compositor.window_action("cover", "fullscreen")
        time.sleep(1.0)
        assert _beats(proc, 2.0) >= 12
        live_compositor.window_action("cover", "unfullscreen")
        time.sleep(1.0)
        assert _beats(proc, 2.0) >= 12
        first = _grab()
        time.sleep(0.4)
        assert first != _grab(), "the bar draws again once it is uncovered"
    finally:
        proc.kill()
        proc.wait()


# 0.6.10 (#247): an app whose only surface is covered still runs what other threads queue.
ONLY_BAR_CHILD = textwrap.dedent(
    """
    import threading, time
    from tess_re import App, LayerWindow

    app = App()
    bar = LayerWindow("only-bar", anchor=("top", "left", "right"), size=(0, 30),
                      exclusive_zone=0, layer="top")
    spinner = bar.create("box", width=20, height=20, position="absolute", x=5, y=5,
                         fill=(50, 200, 50, 255))
    bar.root.add_child(spinner)
    spinner.animate("rotation_deg", 360000.0, 3_600_000, easing="linear")
    app.add_window(bar)
    handle = app.thread_handle()
    bar.every(100, lambda: print("timer", flush=True))

    def post():
        for _ in range(1, 600):
            time.sleep(0.1)
            sent = time.monotonic()
            handle.call_soon(lambda sent=sent: print("lag %.2f" % (time.monotonic() - sent), flush=True))
        handle.stop()

    threading.Thread(target=post, daemon=True).start()
    app.run()
    """
)

COVER_CHILD = textwrap.dedent(
    """
    import time
    from tess_re import App, Window

    app = App()
    window = Window(width=100, height=100, title="cover-only")
    window.root.add_child(window.create("box", width=40, height=40, fill=(200, 50, 50, 255)))
    app.add_window(window)
    window.after(20000, app.thread_handle().stop)
    app.run()
    """
)


def _lines(proc, seconds):
    os.set_blocking(proc.stdout.fileno(), False)
    out, end = [], time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            line = proc.stdout.readline()
        except BlockingIOError:
            line = None
        if line:
            out.append(line.strip())
        else:
            time.sleep(0.02)
    return out


def test_call_soon_and_timers_run_while_the_only_surface_is_covered():
    proc = subprocess.Popen(
        [sys.executable, "-c", ONLY_BAR_CHILD],
        stdout=subprocess.PIPE,
        text=True,
        stderr=subprocess.DEVNULL,
    )
    cover = None
    try:
        before = _lines(proc, 3.0)
        assert sum(1 for x in before if x.startswith("lag")) > 15, before[:5]
        cover = subprocess.Popen(
            [sys.executable, "-c", COVER_CHILD],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        time.sleep(1.5)
        live_compositor.window_action("cover-only", "fullscreen")
        time.sleep(1.0)
        _lines(proc, 0.0)  # drop what arrived before the cover took effect
        covered = _lines(proc, 3.0)
        lags = [float(x.split()[1]) for x in covered if x.startswith("lag")]
        # 3 s at one post per 0.1 s is 30; a stalled app runs none until it is uncovered.
        assert len(lags) >= 26, f"call_soon stalled while covered: {covered[:6]}"
        assert max(lags) < 0.5, f"call_soon ran late while covered: max lag {max(lags)}"
        assert sum(1 for x in covered if x == "timer") >= 26, "the bar's own timer stalled"
    finally:
        for p in (cover, proc):
            if p:
                p.kill()
                p.wait()
