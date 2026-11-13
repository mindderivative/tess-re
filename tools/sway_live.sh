#!/usr/bin/env bash
# Run a command inside a private headless sway, then shut sway down. For the live LayerWindow tests.
#
#     tools/sway_live.sh -- python -m pytest tests/test_layer_window_live.py -q
#     tools/sway_live.sh -- swaymsg -t get_outputs
#     tools/sway_live.sh --renderer pixman -- ...    # compositor on the CPU, no GPU for sway itself
#
# What it does, so it is not done by hand: starts `sway -c <config>` with WLR_BACKENDS=headless
# beside the real session (its own wayland-N socket, so KWin is left alone), waits until sway answers
# on its IPC socket, exports WAYLAND_DISPLAY and SWAYSOCK for the command, runs it, and on the way
# out asks sway to exit, then TERM, then KILL, by the pid it started (never by a name pattern).
# The command's exit status is returned. It gets a private XDG_RUNTIME_DIR, a private Xwayland (DISPLAY), and no
# session bus, so nothing it starts reaches the real session (D-019).
#
# --renderer gles2 (the default) matches the earlier live runs and uses the real GPU for the
# compositor; pixman keeps the compositor off the GPU. The tests' own windows still render with
# wgpu on whatever adapter they pick (see tools/with_adapter.sh).
# Not for KWin-only checks; those need TESS_RE_LIVE_INPUT=1 (or TESS_RE_LIVE_CLIPBOARD=1) and a real session.
# It exports TESS_RE_PRIVATE_COMPOSITOR=1 for the command, which tests read as "this clipboard is private".
set -uo pipefail

renderer=gles2
while [ $# -gt 0 ]; do
    case "$1" in
        --renderer) renderer="${2:-}"; shift 2 ;;
        --) shift; break ;;
        -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) break ;;
    esac
done
[ "$#" -gt 0 ] || { echo "sway_live.sh: no command given (put it after --)" >&2; exit 2; }
command -v sway >/dev/null || { echo "sway_live.sh: sway is not installed" >&2; exit 2; }
command -v swaymsg >/dev/null || { echo "sway_live.sh: swaymsg is not installed" >&2; exit 2; }
work=$(mktemp -d "${TMPDIR:-/tmp}/sway-live.XXXXXX")
# Its own runtime dir (D-019): sway's sockets, and everything that looks in XDG_RUNTIME_DIR, stay apart
# from the real session's (its wayland-0, bus, pipewire and pulse sockets).
runtime="$work/run"
mkdir -m 700 "$runtime"
export XDG_RUNTIME_DIR="$runtime"
pid=""
xpid=""
cleanup() {
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        SWAYSOCK="$sock" swaymsg exit >/dev/null 2>&1
        for _ in $(seq 1 50); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
        kill -0 "$pid" 2>/dev/null && { kill -TERM "$pid" 2>/dev/null; sleep 1; }
        kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
    fi
    if [ -n "$xpid" ] && kill -0 "$xpid" 2>/dev/null; then
        kill -TERM "$xpid" 2>/dev/null
        wait "$xpid" 2>/dev/null
    fi
    rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# sway runs this on start-up and tells us which wayland socket it chose.
printf 'exec sh -c %q\n' "echo \"\$WAYLAND_DISPLAY\" > '$work/display'" > "$work/config"

WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER="$renderer" \
    sway -c "$work/config" >"$work/sway.log" 2>&1 &
pid=$!
sock="$runtime/sway-ipc.$(id -u).$pid.sock"

for _ in $(seq 1 100); do
    [ -S "$sock" ] && [ -s "$work/display" ] && SWAYSOCK="$sock" swaymsg -t get_version >/dev/null 2>&1 && break
    kill -0 "$pid" 2>/dev/null || { echo "sway_live.sh: sway exited early:" >&2; tail -20 "$work/sway.log" >&2; exit 1; }
    sleep 0.1
done
if ! [ -s "$work/display" ]; then
    echo "sway_live.sh: sway did not come up in 10 s:" >&2; tail -20 "$work/sway.log" >&2; exit 1
fi

# This compositor is private, with a clipboard of its own: tests that would overwrite a real desktop's
# clipboard (tests/test_clipboard_layer_live.py) may run without being asked.
export SWAYSOCK="$sock" WAYLAND_DISPLAY TESS_RE_PRIVATE_COMPOSITOR=1
# A private compositor isolates nothing else of the session (D-019): without these two lines the command
# still sees the real X server (DISPLAY, an XWayland of the real desktop: windows and the clipboard
# fall back to it) and the real session bus. Tests that need a bus start their own throwaway one.
unset DISPLAY
export DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent"
WAYLAND_DISPLAY=$(cat "$work/display")
# A private X server for the tests that need X11: Xwayland as a client of this sway (rootless), on a
# display number it picks itself. Without it those tests skip with "needs an X display".
if command -v Xwayland >/dev/null; then
    Xwayland -rootless -noreset -displayfd 3 3>"$work/xdisplay" >"$work/xwayland.log" 2>&1 &
    xpid=$!
    for _ in $(seq 1 100); do [ -s "$work/xdisplay" ] && break; kill -0 "$xpid" 2>/dev/null || break; sleep 0.1; done
    if [ -s "$work/xdisplay" ]; then
        export DISPLAY=":$(head -1 "$work/xdisplay")"
    else
        echo "sway_live.sh: Xwayland did not start; X11 tests will skip" >&2; xpid=""
    fi
fi
echo "sway_live: pid $pid, WAYLAND_DISPLAY=$WAYLAND_DISPLAY, DISPLAY=${DISPLAY:-none}, renderer $renderer" >&2
"$@"
status=$?
exit "$status"
