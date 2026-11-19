"""TRE-REQ-011: a `call_soon` made before `App.run()` runs when the loop starts.

With a window the first frame ran what was queued. With none (`App.run(keep_alive=True)`, a signal
handler or a service), nothing woke the loop, so the callback waited until some other thread queued
something, however long that took. The loop is now woken once at start when no window opens.

The loop needs a display to start (a private sway under `tools/sway_live.sh`), so these skip without one.
"""

import json
import os
import subprocess
import sys
import textwrap

import pytest

CHILD = """
import json, threading, time
from tess_re import App, Window

app = App()
handle = app.thread_handle()
started = time.monotonic()
ran = []

def note(name):
    ran.append((name, round(time.monotonic() - started, 3)))

%(setup)s

def later():
    time.sleep(1.5)  # nothing else wakes the loop for a second and a half
    handle.call_soon(lambda: note("late"))
    time.sleep(0.3)
    handle.stop()

threading.Thread(target=later, daemon=True).start()
app.run(keep_alive=True)
print(json.dumps(ran))
"""


def run(setup):
    if not (os.environ.get("WAYLAND_DISPLAY") or os.environ.get("DISPLAY")):
        pytest.skip("the loop needs a display to start (tools/sway_live.sh)")
    done = subprocess.run(
        [sys.executable, "-c", textwrap.dedent(CHILD) % {"setup": textwrap.indent(textwrap.dedent(setup), "")}],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    line = next((line for line in done.stdout.splitlines() if line.startswith("[")), None)
    assert line, (done.stdout, done.stderr[-1500:])
    return dict(json.loads(line))


def test_a_callback_queued_before_run_runs_when_the_loop_starts_not_when_something_else_wakes_it():
    ran = run('handle.call_soon(lambda: note("early"))')
    assert ran["early"] < 0.5, ran
    assert ran["late"] > 1.4, ran


def test_several_run_in_the_order_they_were_queued():
    out = run(
        """
        for name in ("a", "b", "c"):
            handle.call_soon(lambda n=name: note(n))
        """
    )
    ordered = sorted((t, n) for n, t in out.items() if n in ("a", "b", "c"))
    assert [n for _, n in ordered] == ["a", "b", "c"], out
    assert ordered[-1][0] < 0.5, out


def test_a_stop_before_run_does_not_end_the_run_and_a_window_run_still_runs_the_callback_once():
    ran = run(
        """
        handle.stop()  # outside a run this queues nothing: it must not end a later run
        window = Window(width=100, height=100)
        app.add_window(window)
        handle.call_soon(lambda: note("early"))
        """
    )
    assert ran["early"] < 2.0, ran
    assert ran["late"] > 1.4, "the stop before the run did not end it"
