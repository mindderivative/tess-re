#!/usr/bin/env bash
# Frame-cost and VRAM baseline of two tre revisions, old against new, on the same adapter: medians over repeats.
#
#     tools/baseline_compare.sh prepare [--old v0.6.13] [--new origin/main]   # builds both (heavy, capped; no timing)
#     tools/baseline_compare.sh run     [--repeats 5] [--adapters "radv lvp"] # the measurement: ONLY on a quiet machine
#     tools/baseline_compare.sh summary                                       # re-print the last run's table
#
# `prepare` checks out both revisions in worktrees under /home/phil/.cache/tre-base/{old,new}, builds the engine-render
# `baseline` and `scene_cost` test binaries (release) for each, copies them aside, and builds the Python extension of
# each into its own venv (for tools/vram_probe.py). Nothing is timed. `run` refuses unless the machine is quiet (load average
# 1 minute below 0.5, on AC power; override with QUIET=force for a dry run, never for a published number). It runs
# old and new ALTERNATELY (old new, new old, ...) per adapter, REPEATS times, each binary alone, never in parallel, then runs
# vram_probe for both in a private sway (`tools/sway_live.sh`, RADV only: the fdinfo memory keys), and prints the medians
# and the change in percent; a change above 5% is marked. Results land in /home/phil/.cache/tre-base/results/<timestamp>/.
# Never touches the real session. Every build goes through the CPU cap and the heavy lock; the timed runs do NOT (a
# measurement must not share the machine, and must not be capped).
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
base=/home/phil/.cache/tre-base
cap=/home/phil/.orca/bots/orchestrator/bin/capped; lock=/home/phil/.cache/tre-heavy.lock
heavy=(); [ -x "$cap" ] && heavy=(flock "$lock" "$cap")
cmd=${1:-}; shift || true
old=v0.6.13; new=origin/main; repeats=5; adapters="radv lvp"
while [ $# -gt 0 ]; do
    case "$1" in
        --old) old=$2; shift 2 ;; --new) new=$2; shift 2 ;;
        --repeats) repeats=$2; shift 2 ;; --adapters) adapters=$2; shift 2 ;;
        *) echo "baseline_compare.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done
unset CARGO_TARGET_DIR
git_main() { git -C "$here" "$@"; }

build_side() {  # side ref
    local side=$1 ref=$2; local wt=$base/$side
    mkdir -p "$base/bin/$side"
    git_main fetch -q origin "+refs/heads/*:refs/remotes/origin/*" --tags 2>/dev/null || true
    local sha; sha=$(git_main rev-parse --verify -q "$ref^{commit}") || { echo "unknown ref $ref" >&2; exit 1; }
    [ -e "$wt/.git" ] || git_main worktree add -q --detach "$wt" "$sha"
    git -C "$wt" checkout -q --detach "$sha"
    echo "$sha" > "$base/bin/$side/sha"
    ( cd "$wt" && find crates -name '*.rs' -exec touch {} + \
      && "${heavy[@]}" cargo test -p engine-render --test baseline --test scene_cost --release --no-run --message-format=json 2>/dev/null \
         | python3 -c '
import json,sys,shutil,os
out=sys.argv[1]
for line in sys.stdin:
    try: m=json.loads(line)
    except ValueError: continue
    if m.get("reason")=="compiler-artifact" and m.get("executable") and m["target"]["name"] in ("baseline","scene_cost"):
        shutil.copy(m["executable"], os.path.join(out, m["target"]["name"]))
' "$base/bin/$side" ) || { echo "build of $side failed" >&2; exit 1; }
    ls "$base/bin/$side"
    [ -d "$base/venv-$side" ] || uv venv -q "$base/venv-$side"
    ( cd "$wt" && "${heavy[@]}" uv pip install -q --python "$base/venv-$side/bin/python" maturin \
      && VIRTUAL_ENV="$base/venv-$side" "${heavy[@]}" "$base/venv-$side/bin/maturin" develop --release -m crates/engine-py/Cargo.toml >/dev/null 2>&1 ) \
      || { echo "extension build of $side failed" >&2; exit 1; }
}

quiet_check() {
    local load; load=$(cut -d' ' -f1 /proc/loadavg)
    local ac=1; for f in /sys/class/power_supply/*/online; do [ -r "$f" ] && [ "$(cat "$f")" = 0 ] && [ "$(cat "${f%online}type" 2>/dev/null)" = Mains ] && ac=0; done
    if awk "BEGIN{exit !($load >= 0.5)}" || [ "$ac" = 0 ]; then
        echo "machine not quiet (load $load, AC online: $ac): refusing to measure (QUIET=force for a dry run)" >&2
        [ "${QUIET:-}" = force ] || exit 3
    fi
}

case "$cmd" in
prepare)
    mkdir -p "$base"
    build_side old "$old"; build_side new "$new"
    echo "prepared: old $(cat "$base/bin/old/sha") new $(cat "$base/bin/new/sha")"
    ;;
run)
    [ -x "$base/bin/old/baseline" ] && [ -x "$base/bin/new/baseline" ] || { echo "run 'prepare' first" >&2; exit 2; }
    quiet_check
    out=$base/results/$(date +%Y%m%d-%H%M%S); mkdir -p "$out"
    icd_for() { case $1 in radv) echo radeon_icd.json ;; lvp) echo lvp_icd.json ;; esac; }
    for adapter in $adapters; do
        for i in $(seq 1 "$repeats"); do
            order="old new"; [ $((i % 2)) = 0 ] && order="new old"
            for side in $order; do
                for bin in baseline scene_cost; do
                    f=$out/$adapter-$side-$bin-$i.txt
                    ( cd "$base/$side/crates/engine-render" \
                      && "$here/tools/with_adapter.sh" "$adapter" -- "$base/bin/$side/$bin" --ignored --nocapture --test-threads 1 >"$f" 2>&1 )
                done
            done
        done
    done
    # VRAM: each side's extension, a private sway, RADV only (a software adapter has no fdinfo memory keys).
    for side in old new; do
        ( cd "$here" && PATH="$base/venv-$side/bin:$PATH" "$here/tools/with_adapter.sh" radv -- "$here/tools/sway_live.sh" --renderer pixman -- \
          python "$here/tools/vram_probe.py" >"$out/vram-$side.txt" 2>&1 )
    done
    echo "$out" > "$base/results/latest"
    python3 "$here/tools/baseline_summarize.py" "$out"
    ;;
summary)
    python3 "$here/tools/baseline_summarize.py" "$(cat "$base/results/latest")"
    ;;
*) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
