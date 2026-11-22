#!/usr/bin/env bash
# The CI's cheap, non-test gates, run locally before a push (a red gate costs a ~15 minute CI cycle):
#   stubtest, mypy --strict, check_agent_docs, python.md and rust.md regenerated (no diff allowed), check_version.
#
#     tools/pre_push_gates.sh [--venv DIR] [--expect VERSION]
#
# DIR is a venv with maturin and mypy installed (default: $VIRTUAL_ENV if it has them, else /home/phil/.cache/tre-gate-venv). The tess_re
# extension is rebuilt from THIS checkout first. Every heavy step runs through the machine's CPU cap and the shared heavy-job
# lock when they exist (/home/phil/.orca/bots/orchestrator/bin/capped, /home/phil/.cache/tre-heavy.lock).
#
# Why the touch and the unset: ~/.cargo/config.toml may share ONE target dir between all worktrees, and cargo's workspace-crate
# fingerprints are the same in each (same relative paths, freshness by mtime), so a build or rustdoc here can silently reuse another
# branch's artifacts: docs/api/rust.md then lists items this branch does not have. A stale exported CARGO_TARGET_DIR does the same
# (it overrides the config). So: unset it, touch crates/*.rs, delete the old rustdoc JSON, and only then build.
# The regenerated pages must leave no diff: commit them, then run this again.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
venv=""; expect=""
while [ $# -gt 0 ]; do
    case "$1" in
        --venv) venv="${2:-}"; shift 2 ;;
        --expect) expect="${2:-}"; shift 2 ;;
        -h|--help) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "pre_push_gates.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done
if [ -z "$venv" ]; then
    for d in "${VIRTUAL_ENV:-}" /home/phil/.cache/tre-gate-venv; do
        [ -n "$d" ] && [ -x "$d/bin/maturin" ] && [ -x "$d/bin/mypy" ] && { venv="$d"; break; }
    done
fi
[ -n "$venv" ] && [ -x "$venv/bin/maturin" ] && [ -x "$venv/bin/mypy" ] || { echo "pre_push_gates.sh: $venv needs maturin and mypy (uv pip install maturin mypy)" >&2; exit 2; }
unset CARGO_TARGET_DIR
heavy=()
cap=/home/phil/.orca/bots/orchestrator/bin/capped; lock=/home/phil/.cache/tre-heavy.lock
[ -x "$cap" ] && heavy=(flock "$lock" "$cap")
fail=0
step() { echo "== $*"; }
check() { "$@" || { echo "FAILED: $*" >&2; fail=1; }; }

find crates -name '*.rs' -exec touch {} +
target=$(cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
[ -n "$target" ] && rm -f "$target"/doc/engine_*.json
step "build the extension from this checkout"
VIRTUAL_ENV="$venv" "${heavy[@]}" "$venv/bin/maturin" develop --release -m crates/engine-py/Cargo.toml >/tmp/pre_push_build.log 2>&1 \
    || { echo "build failed (see /tmp/pre_push_build.log):" >&2; grep -n '^error' /tmp/pre_push_build.log | head >&2; exit 1; }
step "stubtest";           check "${heavy[@]}" "$venv/bin/python" -m mypy.stubtest tess_re --allowlist tools/stubtest_allowlist.txt
step "mypy --strict";      check "${heavy[@]}" "$venv/bin/mypy" --strict python/tess_re
step "check_agent_docs";   check python3 -I tools/check_agent_docs.py
step "python.md";          check "$venv/bin/python" tools/gen_python_api.py
step "rust.md (rustdoc JSON from this checkout)"
[ -L target ] || [ -e target ] || ln -s "$target" target
for c in engine-core engine-render engine-platform; do
    RUSTC_BOOTSTRAP=1 check "${heavy[@]}" cargo rustdoc -p "$c" --lib -- -Z unstable-options --output-format json >/dev/null 2>&1 || true
done
check python3 tools/gen_rust_api.py
if [ -n "$expect" ]; then step "check_version"; check python3 -I tools/check_version.py --expect "$expect" --pypi; fi
step "the generated pages leave no diff"
if ! git diff --quiet -- docs/api/python.md docs/api/rust.md; then
    echo "docs/api/*.md differ from what this checkout generates: commit the regenerated pages (git diff --stat docs/api)" >&2
    git diff --stat -- docs/api >&2; fail=1
fi
[ "$fail" = 0 ] && echo "all gates clean" || { echo "gates FAILED" >&2; exit 1; }
