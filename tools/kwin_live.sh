#!/usr/bin/env bash
# Run a command inside a private headless KWin (virtual output), then shut it down. The KWin twin of
# tools/sway_live.sh, for the live tests on the compositor the maintainer's desktop actually runs.
#
#     tools/kwin_live.sh -- python -m pytest -m live_kwin tests/test_popup_geometry_live.py -q
#     tools/kwin_live.sh -- tools/with_adapter.sh lvp -- python -m pytest -m live_kwin tests -q
#     tools/kwin_live.sh --width 1920 --height 1080 -- python -c 'import os; print(os.environ["WAYLAND_DISPLAY"])'
#
# What it does (tests/kwin_sandbox.py, standard library only): a private session bus (dbus-run-session
# with its own config, no service activation, no system bus) in a fresh XDG_RUNTIME_DIR (mode 700 under
# the temporary folder), private HOME and XDG_* folders, then `kwin_wayland --virtual --socket pkwin
# --no-lockscreen` in that bus. The command gets WAYLAND_DISPLAY (the private socket), XDG_RUNTIME_DIR
# and DBUS_SESSION_BUS_ADDRESS (both private), TESS_RE_PRIVATE_COMPOSITOR=1, TESS_RE_COMPOSITOR=kwin and
# no DISPLAY. It REFUSES to start when the bus or runtime directory it would hand out is the real one.
# At the end KWin and the bus are stopped by their process groups and any process that still carries the
# private runtime directory in its environment is killed (never a kill by process name); the folder is
# removed. The command's exit status is returned.
#
# Local only: CI has no KWin. Tests marked `live_kwin` run only here (TESS_RE_COMPOSITOR=kwin). No
# uinput, no TESS_RE_LIVE_INPUT. Build the extension into the venv first (tools/with_adapter.sh pins the GPU).
set -uo pipefail

case "${1:-}" in
    -h|--help) sed -n '2,21p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
esac
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
exec python3 -I "$here/../tests/kwin_sandbox.py" "$@"
