"""TRE-REQ-026: the app is told the compositor went away, and `supports("layer_shell", refresh=True)` asks again.

The test starts two sways of its own (headless, in a private runtime dir) and never touches the shared one
a CI job or `tools/sway_live.sh` provides: killing that would take every later test's compositor with it.
It runs only where `TESS_RE_PRIVATE_COMPOSITOR=1` says the environment is a private one.
"""

import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import textwrap
import threading
import time

import pytest

CHILD = """
import json, time
import tess_re
from tess_re import App, Window, LayerWindow

def say(**kw):
    print(json.dumps(kw), flush=True)

app = App()
window = Window(width=200, height=100)
window.root.set(fill=(200, 0, 0, 255))
bar = LayerWindow("lost-bar", anchor=("top", "left", "right"), size=(0, 30))
bar.root.set(fill=(0, 0, 200, 255))
window.on("closed", lambda e: say(kind="closed", who="window", reason=e.reason))
bar.on("closed", lambda e: say(kind="closed", who="bar", reason=e.reason))
calls = []
app.on("connection_lost", lambda: (calls.append(1), say(kind="lost", running=True, calls=len(calls))))
app.add_window(window)
app.add_window(bar)
handle = app.thread_handle()
handle.call_soon(lambda: say(kind="ready"))
app.run()
say(kind="returned", calls=len(calls), cached=tess_re.supports("layer_shell"),
    refreshed=tess_re.supports("layer_shell", refresh=True))
end = time.time() + 40
while time.time() < end and not tess_re.supports("layer_shell", refresh=True):
    time.sleep(0.2)
say(kind="back", supported=tess_re.supports("layer_shell"))
"""


def need_private_sway():
    if os.environ.get("TESS_RE_PRIVATE_COMPOSITOR") != "1" or not shutil.which("sway"):
        pytest.skip("needs sway and a private environment (tools/sway_live.sh)")


def start_sway(runtime):
    """A headless sway in `runtime`; returns (process, the Wayland display name it chose)."""
    config = runtime / "sway.config"
    display = runtime / f"display.{time.time_ns()}"
    config.write_text(f"exec sh -c 'echo $WAYLAND_DISPLAY > {display}'\n")
    process = subprocess.Popen(
        ["sway", "-c", str(config)],
        env=dict(os.environ, XDG_RUNTIME_DIR=str(runtime), WLR_BACKENDS="headless",
                 WLR_LIBINPUT_NO_DEVICES="1", WLR_RENDERER="pixman"),
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    end = time.time() + 15
    while time.time() < end and not (display.exists() and display.read_text().strip()):
        assert process.poll() is None, "sway exited early"
        time.sleep(0.1)
    return process, display.read_text().strip()


def read_until(process, kind, timeout):
    got, end = [], time.time() + timeout
    while time.time() < end:
        line = process.stdout.readline()
        if not line:
            break
        if line.startswith("{"):
            got.append(json.loads(line))
            if got[-1].get("kind") == kind:
                break
    return got


def test_a_killed_compositor_is_announced_once_and_layer_shell_is_found_again_when_it_returns():
    need_private_sway()
    runtime = pathlib.Path(tempfile.mkdtemp(prefix="sway-lost.", dir=os.environ.get("XDG_RUNTIME_DIR")))
    runtime.chmod(0o700)
    sway, display = start_sway(runtime)
    env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), WAYLAND_DISPLAY=display)
    env.pop("DISPLAY", None)
    child = subprocess.Popen(
        [sys.executable, "-I", "-c", textwrap.dedent(CHILD)], stdout=subprocess.PIPE, text=True, env=env
    )
    watchdog = threading.Timer(120, child.kill)
    watchdog.start()
    again = None
    try:
        assert read_until(child, "ready", 40)[-1]["kind"] == "ready"
        time.sleep(1.5)  # both surfaces are up
        sway.terminate()
        sway.wait(timeout=15)
        events = read_until(child, "returned", 30)
        kinds = [e["kind"] for e in events]
        assert kinds.count("lost") == 1, events
        lost = next(e for e in events if e["kind"] == "lost")
        assert lost["running"] and lost["calls"] == 1
        assert kinds.index("lost") < min(i for i, k in enumerate(kinds) if k == "closed"), events
        assert {(e["who"], e["reason"]) for e in events if e["kind"] == "closed"} == {
            ("window", "connection_lost"),
            ("bar", "connection_lost"),
        }, events
        returned = events[-1]
        assert returned["calls"] == 1
        # The first answer is remembered; a refresh asks the (now absent) compositor.
        assert returned["cached"] is True and returned["refreshed"] is False, returned

        again, display_again = start_sway(runtime)
        if display_again != display:
            pytest.skip(f"the new sway chose {display_again}, not {display}: the child's address is fixed")
        back = read_until(child, "back", 45)[-1]
        assert back == {"kind": "back", "supported": True}, back
    finally:
        watchdog.cancel()
        child.kill()
        for process in (sway, again):
            if process and process.poll() is None:
                process.kill()
                process.wait()
        shutil.rmtree(runtime, ignore_errors=True)
