#!/usr/bin/env python3
"""Medians over the repeats of tools/baseline_compare.sh's run, old against new (change above 5 % is marked).

    python3 tools/baseline_summarize.py /home/phil/.cache/tre-base/results/<timestamp>
"""

import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

ROW = re.compile(
    r"^(?P<name>.+?)\s{2,}(?P<median>[\d.]+)\s+(?P<p95>[\d.]+)\s+\|\s+(?P<update>[\d.]+)\s+(?P<tick>[\d.]+)\s+(?P<layout>[\d.]+)"
    r"\s+(?P<prepare>[\d.]+)\s+(?P<draw>[\d.]+)\s+(?P<gpu>[\d.]+)\s+\|"
)
SCENE = re.compile(r"^(?P<name>.+?)\s+(?P<count>\d+) nodes\s+build\s+\S+\s+\(\s*(?P<build>[\d.]+) ns/node\)\s+render_into\+submit\s+\S+\s+\(\s*(?P<encode>[\d.]+) ns/node\)")
ALLOC = re.compile(r"^Scene::new\(1920x1080\): ([\d.]+)(ns|µs|ms|s)")
UNIT = {"ns": 1.0, "µs": 1e3, "ms": 1e6, "s": 1e9}
FIELDS = ("median", "p95", "update", "tick", "layout", "prepare", "draw", "gpu")


def parse(path):
    rows = {}
    for line in path.read_text(errors="replace").splitlines():
        if m := ROW.match(line):
            rows[("frame", m["name"].strip())] = {f: float(m[f]) for f in FIELDS}
        elif m := SCENE.match(line):
            rows[("scene", m["name"].strip())] = {"build ns/node": float(m["build"]), "encode ns/node": float(m["encode"])}
        elif m := ALLOC.match(line):
            rows[("alloc", "Scene::new 1920x1080")] = {"ns": float(m[1]) * UNIT[m[2]]}
    return rows


def main(out):
    out = Path(out)
    data = defaultdict(lambda: defaultdict(list))  # (adapter, side, kind, name, field) -> [values]
    for path in sorted(out.glob("*-*-*-*.txt")):
        adapter, side, _bin, _i = path.stem.split("-", 3)
        if side not in ("old", "new"):
            continue
        for (kind, name), fields in parse(path).items():
            for field, value in fields.items():
                data[(adapter, kind, name, field)][side].append(value)
    worst = []
    for adapter in sorted({k[0] for k in data}):
        print(f"\n== {adapter}: median over repeats (old -> new, change)")
        for (a, kind, name, field), sides in sorted(data.items()):
            if a != adapter or not sides["old"] or not sides["new"]:
                continue
            o, n = statistics.median(sides["old"]), statistics.median(sides["new"])
            change = (n - o) / o * 100 if o else 0.0
            mark = "  <-- >5%" if abs(change) > 5 and max(o, n) > 0.05 else ""
            if kind == "frame" and field not in ("median", "p95") and not mark:
                continue  # the per-stage columns only when they moved
            print(f"  {kind:5} {name:34} {field:15} {o:10.3f} -> {n:10.3f}  {change:+6.1f}%{mark}  (n={len(sides['old'])}/{len(sides['new'])})")
            if mark:
                worst.append((abs(change), adapter, name, field, change))
    for side in ("old", "new"):
        v = out / f"vram-{side}.txt"
        if v.exists():
            print(f"\n== vram_probe ({side}):\n{v.read_text(errors='replace').strip()}")
    print("\nOver 5 %:" if worst else "\nNothing moved by more than 5 %.")
    for _, adapter, name, field, change in sorted(worst, reverse=True):
        print(f"  {adapter} {name} {field}: {change:+.1f}%")


if __name__ == "__main__":
    main(sys.argv[1])
