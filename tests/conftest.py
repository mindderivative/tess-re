"""Keeps the test run away from the real desktop session (D-019, D-021).

A test that opens a window, or writes the clipboard, acts on the session it can reach, and a
private compositor does not change which session `DISPLAY` or `WAYLAND_DISPLAY` name unless the
runner says so. So on Linux, unless the run is explicitly allowed to reach a session, this removes
both variables for the whole run (child processes inherit that), and every test that needs a display
skips with "no display reachable": the live tests, and the clipboard tests, which skip when no
clipboard service is reachable.

Allowed, any one of:
 - `TESS_RE_PRIVATE_COMPOSITOR=1`, which `tools/sway_live.sh` sets (the session is a private sway);
 - `TESS_RE_ALLOW_REAL_SESSION=1`, which the maintainer sets for a hand run on the desktop
   (docs/contributing.md, "Tests that need hardware or KWin").
CI runners have no display, so the guard changes nothing there.
"""

import os
import sys

REASON = (
    "no display reachable: this run is not allowed to reach a desktop session. Set "
    "TESS_RE_PRIVATE_COMPOSITOR=1 (tools/sway_live.sh does) or TESS_RE_ALLOW_REAL_SESSION=1 (a hand run)"
)


def _guarded() -> bool:
    return (
        sys.platform.startswith("linux")
        and not os.environ.get("TESS_RE_PRIVATE_COMPOSITOR")
        and not os.environ.get("TESS_RE_ALLOW_REAL_SESSION")
    )


#: What was named before the guard removed it, for the report header.
_removed = [name for name in ("DISPLAY", "WAYLAND_DISPLAY") if _guarded() and os.environ.pop(name, None)]


def pytest_report_header(config):
    if _guarded():
        return "real-session guard: ON" + (f" (removed {', '.join(_removed)})" if _removed else "") + f"; {REASON}"
    return "real-session guard: off (private compositor or TESS_RE_ALLOW_REAL_SESSION=1)"
