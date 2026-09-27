#!/usr/bin/env python3
"""Build the interactive floor-constraint profile explorer."""

import csv
import io
import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TRACE = ROOT / "target/release/floor_trace"
OUT = ROOT / "runs/floor/fig/floor-profiles.html"
HEIGHTS = range(1, 33)
# Optimizer variants. All are first-exit ascents without tail shifts, from the same minipump
# seed, mu 1e-4 and growing cap, 30 passes at the final cap. The first is the default selection
# and the reference the summary table's delta column is measured against; the color is used
# when the variant is a comparison line (the selected variant is always drawn white). A bubble
# type shares one hue; its jump-at-floor version is drawn dotted.
# "Jump at floor" (v8) is the bubble before the src fix: it charged states at h >= 0 but not
# the crossing interval, so the price jumped by the full weight as the crossing slid past a
# tick, and fixed-bubble optima parked their last state exactly on the floor. "Continuous"
# (v9) charges the crossing interval weight * f, the fraction t* uses. The header does not
# record which, so the directory is the only thing that tells them apart.
# v10 varies the curvature price's shape (`--penalty`) and the search's moves (`--moves`), no
# bubble. A family shares a hue, lighter for smaller d; box-move variants are dashed and share
# their base's color. Colors of the six default-visible lines were checked for all-pairs
# colorblind separation on the dark panel; within-family steps are ordinal, not categorical.
# v11 compares the optimizer: `--method grad` (L-BFGS on the exact gradient, README-floor 6c)
# against a fresh coordinate-ascent run of the l2 2 default, both from the same seed and cap
# logic. The gradient family is gold, lighter for the default step; grad+tick is dashed in its
# base's color, like the box variants. The coordinate rerun reuses v10's l2 2 color: it is the
# same configuration, rebuilt and rerun so its core time is comparable. The gold fails the
# palette validator's dark-mode lightness band (L 0.80, as the older #56b4e9 does at 0.74) but
# passes CVD separation and contrast against the panel.
# v12 gives `--method grad` the smooth bubble in place of the graze margin (README-floor 6d), all
# laptop runs. Teal is the bubble at margin 0.01, darker when it shrinks, dashed with the graze on
# too; the too-strong weight is red, the too-weak margin pale, and "graze off, no bubble" grey.
# The graze baseline is v11's gradient run again on the laptop, so it keeps v11's gold.
V7, V8, V9, V10, V11, V12 = (ROOT / f"runs/floor/{d}/out"
                             for d in ("v7-30pass", "v8-bubble", "v9-bubble-cont", "v10-pen", "v11-grad",
                                       "v12-bubble-grad"))
FIXED, SHRINK = ("0.5", "1", "1"), ("0.5", "1", "0.8")
BOX = "tick,box:3:9:17:33"
GROUPS = {"bubble": "bubble study (v7–v9)", "pen": "penalty/moves (v10)", "grad": "gradient ascent (v11)",
          "gradbub": "gradient + bubble (v12)"}


def variant(group, name, sweep, label, color, style="solid", bubble=None, pen=None, moves=None,
            method=None, shown=False):
    # bubble: expected "margin,weight shrink" from header line 1, None for no bubble.
    # pen, moves: expected `pen <spec> moves <spec>` from header line 1; None where the header
    # predates those fields (v7-v9 ran with the l1 price and per-tick moves only).
    # method: expected `method <m>` from header line 1; None for coordinate ascent, whose header
    # has no such field.
    return {"group": group, "name": name, "dir": sweep, "label": label, "color": color,
            "style": style, "bubble": bubble, "pen": pen, "moves": moves, "method": method,
            "shown": shown}


VARIANTS = [
    variant("bubble", "noshift30", V7, "no bubble", "#56b4e9", shown=True),
    variant("bubble", "bubble30", V8, "fixed bubble (jump at floor)", "#e69f00", "dotted", FIXED),
    variant("bubble", "shrink30", V8, "shrinking bubble (jump at floor)", "#cc79a7", "dotted", SHRINK),
    variant("bubble", "bubble30c", V9, "fixed bubble (continuous)", "#e69f00", bubble=FIXED),
    variant("bubble", "shrink30c", V9, "shrinking bubble (continuous)", "#cc79a7", bubble=SHRINK),
    variant("pen", "l1", V10, "l1 (control)", "#8fd6a3", pen="l1", moves="tick"),
    variant("pen", "l1box", V10, "l1 + box", "#2b9667", "dashed", pen="l1", moves=BOX, shown=True),
    variant("pen", "l1ramp", V10, "l1 ramp only", "#6f6de8", pen="l1", moves="ramp", shown=True),
    variant("pen", "hub0.5", V10, "Huber 0.5", "#f5a66e", pen="huber:0.5", moves="tick"),
    variant("pen", "hub2", V10, "Huber 2", "#d44f13", pen="huber:2", moves="tick", shown=True),
    variant("pen", "l2_0.125", V10, "l2 0.125", "#f0b3e0", pen="l2:0.125", moves="tick"),
    variant("pen", "l2_0.5", V10, "l2 0.5", "#d058ae", pen="l2:0.5", moves="tick", shown=True),
    variant("pen", "l2_2", V10, "l2 2", "#a44cc0", pen="l2:2", moves="tick"),
    variant("pen", "l2_0.5box", V10, "l2 0.5 + box", "#d058ae", "dashed", pen="l2:0.5", moves=BOX,
            shown=True),
    variant("grad", "l2_2", V11, "coordinate l2 2 (rerun)", "#a44cc0", pen="l2:2", moves="tick",
            shown=True),
    variant("grad", "grad", V11, "gradient", "#d9bf2b", pen="l2:2", moves="tick", method="grad",
            shown=True),
    variant("grad", "grad_s2", V11, "gradient, max-step 2", "#8f7d12", pen="l2:2", moves="tick",
            method="grad"),
    variant("grad", "gradtick", V11, "gradient + 30 tick passes", "#d9bf2b", "dashed", pen="l2:2",
            moves="tick", method="grad+tick"),
    variant("gradbub", "graze", V12, "graze margin (laptop rerun)", "#d9bf2b", pen="l2:2", moves="tick",
            method="grad", shown=True),
    variant("gradbub", "off", V12, "graze off, no bubble", "#9aa3ad", pen="l2:2", moves="tick", method="grad"),
    variant("gradbub", "b0.01_1e-2", V12, "bubble 0.01, 1e-2", "#3fc1c9", bubble=("0.01", "0.01", "1"),
            pen="l2:2", moves="tick", method="grad", shown=True),
    variant("gradbub", "b0.01_1e-2_shrink", V12, "bubble 0.01, 1e-2, ×0.3 twice", "#1f7f86",
            bubble=("0.01", "0.01", "0.3"), pen="l2:2", moves="tick", method="grad", shown=True),
    variant("gradbub", "b0.01_1e-2+graze", V12, "bubble 0.01, 1e-2 + graze", "#3fc1c9", "dashed",
            bubble=("0.01", "0.01", "1"), pen="l2:2", moves="tick", method="grad"),
    variant("gradbub", "b0.05_1e-2", V12, "bubble 0.05, 1e-2", "#9ae3e6", bubble=("0.05", "0.01", "1"),
            pen="l2:2", moves="tick", method="grad"),
    variant("gradbub", "b0.01_1e-1", V12, "bubble 0.01, 1e-1 (too strong)", "#e8575a",
            bubble=("0.01", "0.1", "1"), pen="l2:2", moves="tick", method="grad"),
]
METHOD = re.compile(r"\bmethod (\S+)")
BUBBLE = re.compile(r"bubble ([-+\d.eE]+),([-+\d.eE]+) shrink ([-+\d.eE]+)")
PEN = re.compile(r"\bpen(?:alty)? (\S+) moves (\S+)")
# Chatter: a first-difference sign flip where both steps exceed this many degrees.
CHATTER_STEP = 2.0
# Depth under the floor, in blocks, below which a replay's exit is a graze float noise can flip.
GRAZE = 1e-9
FIELD_VZ = (-0.5, 3.0)
FIELD_VY = (-1.5, 2.0)
COLUMNS = ("t", "pitch", "y", "z", "vy", "vz")
HEADER = re.compile(r"t\* ([-+\d.eE]+)\s+z\(t\*\) ([-+\d.eE]+)\s+survived \d+\s+"
                    r"exit KE ([-+\d.eE]+)\s+passes (\S+)")


def run_trace(path):
    result = subprocess.run([str(TRACE), "--file", path], cwd=ROOT, check=True,
                            capture_output=True, text=True)
    return [{key: (None if value == "NaN" else float(value)) for key, value in row.items()}
            for row in csv.DictReader(io.StringIO(result.stdout))]


def clipped(rows, y0):
    # The optimizer's rule (`exit_score` in src/bin/floor.rs): the exit is the first interval
    # whose end is strictly below the floor. A state at exactly h = 0 mid-flight is a graze, not
    # an exit -- no-bubble optima do this. But fixed-bubble optima land exactly on the floor at
    # a tick and their file ends there; the optimizer then exits in the next interval at
    # fraction 0, i.e. at that last state.
    for k in range(1, len(rows)):
        if rows[k]["y"] < -y0:
            a, b = rows[k - 1], rows[k]
            fraction = (a["y"] + y0) / (a["y"] - b["y"])
            end = {key: a[key] + fraction * (b[key] - a[key]) for key in ("t", "y", "z", "vy", "vz")}
            end["y"] = -float(y0)
            end["pitch"] = None
            return rows[:k] + [end], end["t"], end["z"]
    if rows[-1]["y"] == -y0:
        end = dict(rows[-1], pitch=None)
        return rows[:-1] + [end], end["t"], end["z"]
    return rows, None, rows[-1]["z"]


def columns(rows):
    return {key: [None if row[key] is None else round(row[key], 4) for row in rows] for key in COLUMNS}


def load_field():
    candidates = [
        ROOT / "runs/atlas/fig/mapfine/feasible-explorer.html",
        ROOT.parent / "myopic-metrics/runs/steady/fig/steady-explorer.html",
    ]
    for path in candidates:
        if path.exists():
            match = re.search(r'const FIELD = "(data:image/png;base64,[^"]+)";', path.read_text())
            if match:
                return match.group(1)
    raise FileNotFoundError("could not find the embedded one-tick energy field")


def roughness(pitches):
    # Over the live pitches only (the clipped replay's), so the flat dead tail adds nothing.
    d1 = [b - a for a, b in zip(pitches, pitches[1:])]
    rough = sum(abs(b - a) for a, b in zip(d1, d1[1:]))
    chatter = sum(1 for a, b in zip(d1, d1[1:])
                  if a * b < 0 and abs(a) > CHATTER_STEP and abs(b) > CHATTER_STEP)
    return rough, chatter


def header_bubble(line):
    # A zero weight is no bubble: v10 headers print the default `bubble 1,0 shrink 1`.
    found = BUBBLE.search(line)
    return None if not found or float(found.group(2)) == 0 else found.groups()


def load_run(v, path, y0):
    """One file's replay and summary, or a string saying why it is skipped."""
    lines = path.read_text().splitlines()
    if len(lines) < 3:
        return "fewer than three lines"
    if (bubble := header_bubble(lines[0])) != v["bubble"]:
        return f"header bubble {bubble}, expected {v['bubble']}"
    found = PEN.search(lines[0])
    if (found.groups() if found else (None, None)) != (v["pen"], v["moves"]):
        return f"header pen/moves {found and found.groups()}, expected {(v['pen'], v['moves'])}"
    found = METHOD.search(lines[0])
    if (found and found.group(1)) != v["method"]:
        return f"header method {found and found.group(1)}, expected {v['method']}"
    match = HEADER.search(lines[1])
    if not match:
        return "could not read utility, exit KE and passes from line 2"
    t_star, z_star, ke = map(float, match.groups()[:3])
    rows = run_trace(path)
    states, t_exit, z_exit = clipped(rows, y0)
    if t_exit is None:
        return "never reaches the floor on replay"
    # The replay must land where the optimizer said it did; a gap means the physics settings
    # drifted from the ones the sweep ran with -- except at a touch-and-go graze. Optima without
    # a bubble can park a mid-flight state on the floor to within ~1e-13 blocks, and then whether
    # it counts as the exit is a sign decided by the last few ulps: the laptop's replay and the
    # cluster's optimizer (different machine and libm) can disagree. Such a file is skipped and
    # reported rather than drawn with the wrong exit; any other gap is still an error.
    gap = max(abs(t_exit - t_star), abs(z_exit - z_star))
    if gap > 1e-3:
        k = len(states) - 1
        h = rows[k]["y"] + y0 if k < len(rows) else None
        if h is not None and -GRAZE < h < 0:
            return (f"touch-and-go graze: replay is under the floor by {-h:.1e} at tick {k} "
                    f"(t* {t_exit:.2f}), the header's t* is {t_star:.2f}")
    rough, chatter = roughness([s["pitch"] for s in states if s["pitch"] is not None])
    return {"t_exit": t_star, "z_exit": z_star, "ke": ke, "passes": match.group(4),
            "rough": round(rough, 3), "chatter": chatter, "gap": gap, "states": columns(states)}


def load_data():
    if not TRACE.exists():
        subprocess.run(["cargo", "build", "--release", "--bin", "floor_trace"], cwd=ROOT, check=True)
    utilities = {"range": [], "endurance": []}
    missing, skipped, gaps = [], [], []
    for y0 in HEIGHTS:
        for key, mode in [("range", "dist"), ("endurance", "time")]:
            runs = []
            for v in VARIANTS:
                path = v["dir"] / f"{mode}_y{y0}_{v['name']}.pitches"
                run = load_run(v, path, y0) if path.exists() else None
                if run is None:
                    missing.append(path)
                elif isinstance(run, str):
                    skipped.append(f"{path.relative_to(ROOT)}: {run}")
                    run = None
                elif (gap := run.pop("gap")) > 1e-3:
                    gaps.append(f"{path.relative_to(ROOT)}: {gap:.3g}")
                runs.append(run)
            utilities[key].append({"y0": y0, "runs": runs})
    # A variant with no file at all (a sweep not pulled yet) is left out of the figure.
    keep = [vi for vi in range(len(VARIANTS))
            if any(g["runs"][vi] for u in utilities.values() for g in u)]
    for u in utilities.values():
        for g in u:
            g["runs"] = [g["runs"][vi] for vi in keep]
    kept = [VARIANTS[vi] for vi in keep]
    loaded = sum(r is not None for u in utilities.values() for g in u for r in g["runs"])
    by_dir = {}
    for path in missing:
        by_dir[path.parent] = by_dir.get(path.parent, 0) + 1
    for folder, count in by_dir.items():
        print(f"missing {count} files in {folder.relative_to(ROOT)}")
    absent = [v["name"] for v in VARIANTS if v not in kept]
    if absent:
        print(f"no files at all, left out: {', '.join(absent)}")
    for line in skipped:
        print(f"skipped {line}")
    print(f"{loaded} files loaded; {len(gaps)} with a header-vs-replay gap over 1e-3")
    if gaps:
        raise RuntimeError("replayed exits differ from the file headers:\n  " + "\n  ".join(gaps))
    if VARIANTS[0] not in kept:
        raise RuntimeError(f"the reference variant {VARIANTS[0]['name']} has no files")
    return {"heights": list(HEIGHTS),
            "groups": GROUPS,
            "variants": [{"group": v["group"], "name": v["name"], "label": v["label"],
                          "color": v["color"], "style": v["style"], "shown": v["shown"],
                          "bubble": None if v["bubble"] is None else list(map(float, v["bubble"])),
                          "pen": v["pen"], "moves": v["moves"], "method": v["method"]} for v in kept],
            "chatterStep": CHATTER_STEP,
            "utilities": utilities}


HTML = r'''<!doctype html>
<meta charset="utf-8">
<title>floor-constraint profiles</title>
<div id="floor-explorer">
  <style>
    html, body { margin: 0; background: #14171a; }
    #floor-explorer { box-sizing: border-box; color: #e6e9ed; background: #14171a; font: 14px system-ui, sans-serif; padding: 16px; width: 100%; height: 100vh; overflow: hidden; display: grid; grid-template-columns: minmax(0, 1.12fr) minmax(440px, .88fr); gap: 12px; }
    #floor-explorer * { box-sizing: border-box; }
    #floor-explorer h1 { font-size: 20px; font-weight: 500; margin: 0 0 10px; }
    #floor-explorer .left { min-width: 0; min-height: 0; display: grid; grid-template-rows: auto auto auto auto auto auto minmax(0, 1fr); }
    #floor-explorer .right { min-width: 0; min-height: 0; overflow-y: auto; padding-right: 4px; display: grid; grid-template-rows: 160px 160px 150px 160px 160px 210px 150px 150px auto; gap: 10px; }
    #floor-explorer select { font: inherit; color: #e6e9ed; background: #1b1f24; border: 1px solid #4a525c; border-radius: 6px; padding: 3px 6px; }
    #floor-explorer .shown { margin-top: 7px; display: grid; gap: 5px; }
    #floor-explorer .shown fieldset { margin: 0; padding: 0; border: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 6px; }
    #floor-explorer .shown legend { float: left; margin-right: 6px; color: #b9c0c9; font-size: 12.5px; }
    #floor-explorer .shown .group-actions button, #floor-explorer .reset { font: 12px system-ui, sans-serif; color: #b9c0c9; background: none; border: 1px solid #4a525c; border-radius: 4px; padding: 1px 6px; cursor: pointer; }
    #floor-explorer .shown .group-actions { display: inline-flex; gap: 4px; margin-right: 4px; }
    #floor-explorer .chip { position: relative; }
    #floor-explorer .chip input { position: absolute; opacity: 0; pointer-events: none; }
    #floor-explorer .chip label { display: inline-flex; align-items: center; gap: 6px; padding: 2px 8px; border: 1px solid #4a525c; border-radius: 12px; cursor: pointer; color: #7d8691; font-size: 12.5px; }
    #floor-explorer .chip label .swatch { width: 16px; opacity: .35; }
    #floor-explorer .chip input:checked + label { color: #e6e9ed; border-color: #8a939e; background: #242a31; }
    #floor-explorer .chip input:checked + label .swatch { opacity: 1; }
    #floor-explorer .chip input:focus-visible + label { outline: 2px solid #ffffff; outline-offset: 1px; }
    #floor-explorer .chip.selected label { box-shadow: inset 0 0 0 1px #ffffff; }
    #floor-explorer .notes { margin-top: 5px; }
    #floor-explorer .notes summary { cursor: pointer; font-size: 12.5px; }
    #floor-explorer .notes p { margin: 4px 0 0; }
    #floor-explorer .summary { min-width: 0; }
    #floor-explorer .summary .scroll { overflow-x: auto; }
    #floor-explorer table { border-collapse: collapse; font-size: 12px; font-variant-numeric: tabular-nums; white-space: nowrap; }
    #floor-explorer th, #floor-explorer td { padding: 2px 6px; text-align: right; }
    #floor-explorer th { color: #b9c0c9; font-weight: 500; }
    #floor-explorer td:first-child, #floor-explorer th:first-child { text-align: left; }
    #floor-explorer tr.group td { color: #b9c0c9; padding-top: 6px; border-bottom: 1px solid #4a525c; }
    #floor-explorer tr.hidden td { color: #7d8691; }
    #floor-explorer tr.selected td { background: #242a31; }
    #floor-explorer td.best { font-weight: 700; color: #ffffff; }
    #floor-explorer td.split, #floor-explorer th.split { border-left: 1px solid #4a525c; }
    #floor-explorer tbody tr:not(.group) { cursor: pointer; }
    #floor-explorer .controls { display: grid; grid-template-columns: auto minmax(180px, 1fr) auto; gap: 12px; align-items: center; }
    #floor-explorer .controls.secondary { margin-top: 7px; }
    #floor-explorer input[type="range"] { width: 100%; }
    #floor-explorer .toggle { display: flex; align-items: center; gap: 6px; white-space: nowrap; }
    #floor-explorer .readout { min-width: 0; grid-column: 1 / -1; text-align: left; font-variant-numeric: tabular-nums; }
    #floor-explorer .legend { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 8px 18px; color: #b9c0c9; }
    #floor-explorer .key { display: inline-flex; gap: 7px; align-items: center; }
    #floor-explorer .swatch { width: 22px; height: 3px; background: var(--color); }
    #floor-explorer .variant-row { margin-top: 9px; display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; }
    #floor-explorer .segmented { display: inline-flex; flex-wrap: wrap; border: 1px solid #4a525c; border-radius: 6px; overflow: hidden; }
    #floor-explorer .segmented input { position: absolute; opacity: 0; pointer-events: none; }
    #floor-explorer .segmented label { display: inline-flex; align-items: center; gap: 6px; padding: 4px 10px; cursor: pointer; color: #b9c0c9; font-variant-numeric: tabular-nums; border-left: 1px solid #4a525c; }
    #floor-explorer .segmented label:first-of-type { border-left: 0; }
    #floor-explorer .segmented label .swatch { width: 14px; }
    #floor-explorer .segmented input:checked + label { background: #2b3139; color: #ffffff; }
    #floor-explorer .segmented input:checked + label .swatch { background: #ffffff; }
    #floor-explorer .segmented input:focus-visible + label { outline: 2px solid #ffffff; outline-offset: -2px; }
    #floor-explorer .note { color: #b9c0c9; }
    #floor-explorer .bubble-note { margin: 5px 0 0; font-size: 12.5px; line-height: 1.35; }
    #floor-explorer .legend-others { display: contents; }
    #floor-explorer .swatch.dotted { background: repeating-linear-gradient(90deg, var(--color) 0 3px, transparent 3px 6px); }
    #floor-explorer .segmented input:checked + label .swatch.dotted { background: #ffffff; }
    #floor-explorer .swatch.dashed { background: repeating-linear-gradient(90deg, var(--color) 0 5px, transparent 5px 9px); }
    #floor-explorer figure { margin: 0; min-width: 0; min-height: 0; display: grid; grid-template-rows: auto minmax(0, 1fr); }
    #floor-explorer figcaption { margin: 0 0 4px; color: #b9c0c9; }
    #floor-explorer canvas { display: block; width: 100%; height: 100%; min-height: 0; background: #1b1f24; border: 1px solid #4a525c; }
    @media (max-width: 850px) {
      #floor-explorer { height: auto; overflow: visible; display: block; }
      #floor-explorer .controls { grid-template-columns: 1fr; }
      #floor-explorer .readout { min-width: 0; text-align: left; }
      #floor-explorer .left, #floor-explorer .right { display: block; }
      #floor-explorer figure { margin-bottom: 16px; }
      #floor-explorer canvas { height: auto; aspect-ratio: 1.55; }
    }
  </style>
  <section class="left">
    <h1>floor-constraint profiles</h1>
    <div class="controls">
      <label for="floor-y0">initial height y0</label>
      <input id="floor-y0" type="range" min="1" max="32" value="8" step="1">
      <output id="floor-readout" class="readout" aria-live="polite"></output>
    </div>
    <div class="controls secondary">
      <label class="toggle"><input id="floor-utility" type="checkbox"> highlight endurance <span>(off = range)</span></label>
      <div class="legend" aria-label="profile legend">
        <span class="key"><span class="swatch" style="--color:#ffffff"></span><span id="floor-legend-selected"></span></span>
        <span id="floor-legend-others" class="legend-others"></span>
        <span class="key"><span class="swatch dashed" style="--color:rgba(230,233,237,.55)"></span>fixed bubble margin, h = 0.5 (near-floor panel)</span>
      </div>
      <span></span>
    </div>
    <div class="variant-row">
      <label for="floor-variant">selected variant (white, with faint curves for every y0)</label>
      <select id="floor-variant"></select>
    </div>
    <div class="shown" id="floor-shown" aria-label="variants drawn as comparison lines"></div>
    <details class="note bubble-note notes" open>
      <summary>about the variants</summary>
      <p>All: first-exit ascents, same minipump seed, mu 1e-4, growing cap, 30 passes at the final cap. Bubble = per-tick price weight·(max(0, margin − h)/margin)² on each state before the floor crossing (h = height above floor); fixed: margin 0.5 blocks, weight 1 block of energy per tick at contact; shrinking: starts there, both ×0.8 after every pass. Jump at floor (dotted): the crossing tick went unpriced, so the price dropped by the full weight as the crossing slid past a tick; continuous: the crossing interval pays weight × f, the same fraction t* uses.</p>
      <p>v10, no bubble: the curvature price on each live second difference x (degrees) is l1 mu·|x|, Huber d mu·x²/(2d) inside |x| ≤ d and mu·(|x| − d/2) outside, or l2 d mu·x²/(2d). Moves: a per-tick pitch search, except "ramp only", which searches second differences alone (adds d·(s − t) to every later live pitch); "+ box" (dashed) adds a search that shifts k = 3, 9, 17, 33 consecutive pitches by d. "l1 (control)" is the same run as "no bubble". Table: rough = Σ|second difference| over live pitches; chatter = sign flips of the first difference with both steps over __CHATTER__°.</p>
      <p>v11, l2 2 price, no bubble: "gradient" is L-BFGS ascent on the exact reverse-mode gradient of the exit score (every pitch at once, Armijo line search on the true score), with pre-exit dips held 0.01 → 0.003 → 0.001 blocks off the floor; "max-step 2" caps any pitch's change per step at 2° instead of 5°; "+ 30 tick passes" (dashed) runs the per-tick coordinate search from the gradient's answer at every cap. "coordinate l2 2 (rerun)" is v10's l2 2 run again on the same build.</p>
      <p>v12, gradient ascent, l2 2 price, laptop: "graze margin" is v11's gradient run again. The others turn the graze off and let a smooth bubble (margin, weight: blocks, blocks of energy per tick at contact) keep dips off the floor instead; "×0.3 twice" shrinks both after each of the first two stalls; "+ graze" (dashed) keeps both. The weight 1e-1 bubble charges more for the crossing's last fraction of a tick than that fraction is worth, so its flights end early.</p>
    </details>
    <figure><figcaption>replay on the one-tick energy field · exact mth_lut/reference trajectory</figcaption><canvas id="floor-field" aria-label="velocity replay over the one-tick energy field"></canvas></figure>
  </section>
  <section class="right">
    <figure class="pitch"><figcaption>pitch vs time · faint: every y0 for the selected variant; colored: the other variants at selected y0</figcaption><canvas id="floor-pitch" aria-label="pitch versus time"></canvas></figure>
    <figure><figcaption>height vs time · absolute height above the shared floor</figcaption><canvas id="floor-height" aria-label="height above floor versus time"></canvas></figure>
    <figure><figcaption>height near the floor · 0 to 2 blocks · dotted rule: fixed bubble margin h = 0.5</figcaption><canvas id="floor-near" aria-label="height above floor versus time, zoomed to the bottom two blocks"></canvas></figure>
    <figure><figcaption>forward position vs time</figcaption><canvas id="floor-ztime" aria-label="forward position versus time"></canvas></figure>
    <figure><figcaption>velocity vs time · vy solid, vz dashed (jump-at-floor variants: vy dotted, vz dash-dot; box variants: vy long dash, vz dash-dot-dot)</figcaption><canvas id="floor-velocity" aria-label="vertical and forward velocity versus time"></canvas></figure>
    <figure><figcaption>actual path · height above the shared y = 0 floor</figcaption><canvas id="floor-path" aria-label="height above floor versus forward distance"></canvas></figure>
    <figure><figcaption>range utility z(t*) vs initial height · one line per variant · points mark selected y0</figcaption><canvas id="floor-range" aria-label="range utility versus initial height"></canvas></figure>
    <figure><figcaption>endurance utility t* vs initial height · one line per variant · points mark selected y0</figcaption><canvas id="floor-endurance" aria-label="endurance utility versus initial height"></canvas></figure>
    <div class="summary"><div id="floor-summary-caption" class="note"></div><div class="scroll"><table id="floor-summary"></table></div></div>
  </section>
  <script>
  (() => {
    const D = __DATA__, FIELD = "__FIELD__";
    const FIELD_VZ = __FIELD_VZ__, FIELD_VY = __FIELD_VY__;
    const root = document.querySelector("#floor-explorer"), slider = root.querySelector("#floor-y0");
    const utilityBox = root.querySelector("#floor-utility"), readout = root.querySelector("#floor-readout");
    const variantSelect = root.querySelector("#floor-variant"), shownBox = root.querySelector("#floor-shown");
    const summary = root.querySelector("#floor-summary"), summaryCaption = root.querySelector("#floor-summary-caption");
    const legendSelected = root.querySelector("#floor-legend-selected"), legendOthers = root.querySelector("#floor-legend-others");
    const fieldImage = new Image(); fieldImage.src = FIELD;
    const V = D.variants, SELECTED = "#ffffff", V_COLORS = V.map(v=>v.color), DOT = [1,3.5];
    // Line style per variant: vy (and every other panel) and vz on the velocity panel.
    const DASH = {solid:[], dotted:DOT, dashed:[7,4]}, VZ_DASH = {solid:[5,3], dotted:[7,3,1,3], dashed:[9,3,1,3,1,3]};
    const swatch = vi => `<span class="swatch${V[vi].style==="solid"?"":" "+V[vi].style}" style="--color:${V_COLORS[vi]}"></span>`;
    const C = {fg:"#e6e9ed", dim:"#b9c0c9", bg:"#14171a"};
    // Comparison lines drawn; the selected variant is drawn whether or not it is in here.
    const DEFAULT_SHOWN = V.map((v,i)=>v.shown?i:-1).filter(i=>i>=0), shown = new Set(DEFAULT_SHOWN);
    for(const [group,title] of Object.entries(D.groups)) {
      const members = V.map((v,i)=>i).filter(i=>V[i].group===group);
      if(!members.length) continue;
      variantSelect.insertAdjacentHTML("beforeend",`<optgroup label="${title}">${members.map(i=>`<option value="${i}">${V[i].label}</option>`).join("")}</optgroup>`);
      shownBox.insertAdjacentHTML("beforeend",`<fieldset data-group="${group}"><legend>${title}</legend><span class="group-actions"><button type="button" data-set="all">all</button><button type="button" data-set="none">none</button></span>${
        members.map(i=>`<span class="chip" data-vi="${i}"><input type="checkbox" id="floor-show-${i}" value="${i}"${shown.has(i)?" checked":""}><label for="floor-show-${i}">${swatch(i)}${V[i].label}</label></span>`).join("")}</fieldset>`);
    }
    shownBox.insertAdjacentHTML("beforeend",`<div><button type="button" class="reset">default lines</button></div>`);
    variantSelect.value = "0";
    const syncShown = () => { shown.clear(); shownBox.querySelectorAll("input:checked").forEach(b=>shown.add(+b.value)); };
    shownBox.addEventListener("change",()=>{ syncShown(); update(); });
    shownBox.addEventListener("click",event=>{
      const button=event.target.closest("button"); if(!button) return;
      const boxes=button.classList.contains("reset")?[...shownBox.querySelectorAll("input")]:[...button.closest("fieldset").querySelectorAll("input")];
      for(const b of boxes) b.checked=button.classList.contains("reset")?DEFAULT_SHOWN.includes(+b.value):button.dataset.set==="all";
      syncShown(); update();
    });
    // Columnar states from Python -> row objects the drawing code indexes by field. A run is
    // null where its file is missing or was skipped.
    for(const utility of ["range","endurance"]) for(const group of D.utilities[utility]) for(const run of group.runs) {
      if(!run) continue;
      const cols=run.states; run.states=cols.t.map((t,i)=>({t,pitch:cols.pitch[i],y:cols.y[i],z:cols.z[i],vy:cols.vy[i],vz:cols.vz[i]}));
    }

    // Spread into Math.max/min overflows the call stack on the ~10^5 states embedded here.
    const maxOf=a=>a.reduce((m,v)=>v>m?v:m,-Infinity), minOf=a=>a.reduce((m,v)=>v<m?v:m,Infinity);
    function setup(canvas, margins={l:58,r:15,t:14,b:38}) {
      const rect=canvas.getBoundingClientRect(), dpr=Math.min(devicePixelRatio||1,2);
      canvas.width=Math.round(rect.width*dpr); canvas.height=Math.round(rect.height*dpr);
      const ctx=canvas.getContext("2d"); ctx.setTransform(dpr,0,0,dpr,0,0);
      return {ctx,w:rect.width,h:rect.height,m:margins};
    }
    function domain(values, fraction=.06) {
      let lo=minOf(values), hi=maxOf(values); if(lo===hi){lo-=.5;hi+=.5;}
      const pad=(hi-lo)*fraction; return [lo-pad,hi+pad];
    }
    function steppedDomain(values, step, includeZero=false) {
      let lo=minOf(values), hi=maxOf(values);
      if(includeZero){lo=Math.min(lo,0);hi=Math.max(hi,0);}
      if(lo===hi){lo-=step;hi+=step;}
      return [Math.floor(lo/step)*step,Math.ceil(hi/step)*step];
    }
    function niceAxis(values, intervals=4, includeZero=false) {
      let lo=minOf(values), hi=maxOf(values);
      if(includeZero){lo=Math.min(lo,0);hi=Math.max(hi,0);}
      const raw=(hi-lo)/intervals, magnitude=10**Math.floor(Math.log10(raw)), q=raw/magnitude;
      const step=(q<=1?1:q<=2?2:q<=2.5?2.5:q<=5?5:10)*magnitude;
      return {domain:steppedDomain(values,step,includeZero),step};
    }
    function tickDigits(step) {
      for(let digits=0;digits<=6;digits++) if(Math.abs(step*10**digits-Math.round(step*10**digits))<1e-9) return digits;
      return 6;
    }
    function fmt(v, digits) { return Math.abs(v)<1e-12 ? "0" : v.toFixed(digits); }
    function ticks(d, step) {
      if(!step) return Array.from({length:5},(_,i)=>d[0]+i*(d[1]-d[0])/4);
      const values=[], direction=d[0]<=d[1]?1:-1, lo=minOf(d), hi=maxOf(d);
      for(let v=Math.ceil(lo/step)*step;v<=hi+step*1e-9;v+=step) values.push(+v.toFixed(10));
      return direction>0?values:values.reverse();
    }
    function axes(s, xd, yd, xl, yl, xdigits=0, ydigits=1, prepaint=null, xstep=null, ystep=null) {
      const {ctx,w,h,m}=s, X=x=>m.l+(x-xd[0])/(xd[1]-xd[0])*(w-m.l-m.r), Y=y=>h-m.b-(y-yd[0])/(yd[1]-yd[0])*(h-m.t-m.b);
      if(prepaint) prepaint({X,Y}); ctx.font="12px system-ui";
      for(const x of ticks(xd,xstep)) {
        const px=X(x);
        ctx.strokeStyle="rgba(185,192,201,.24)"; ctx.lineWidth=1; ctx.beginPath();ctx.moveTo(px,m.t);ctx.lineTo(px,h-m.b);ctx.stroke();
        ctx.fillStyle=C.dim;ctx.textAlign="center";ctx.fillText(fmt(x,xdigits),px,h-m.b+18);
      }
      for(const y of ticks(yd,ystep)) {
        const py=Y(y);
        ctx.strokeStyle="rgba(185,192,201,.24)";ctx.lineWidth=1;ctx.beginPath();ctx.moveTo(m.l,py);ctx.lineTo(w-m.r,py);ctx.stroke();
        ctx.fillStyle=C.dim;ctx.textAlign="right";ctx.fillText(fmt(y,ydigits),m.l-7,py+4);
      }
      ctx.fillStyle=C.fg;ctx.textAlign="center";ctx.fillText(xl,(m.l+w-m.r)/2,h-6);
      ctx.save();ctx.translate(14,(m.t+h-m.b)/2);ctx.rotate(-Math.PI/2);ctx.fillText(yl,0,0);ctx.restore();
      return {ctx,X,Y};
    }
    function line(ctx, points, color, width, alpha, dash=[]) {
      if(points.length<2)return;ctx.beginPath();points.forEach(([x,y],i)=>i?ctx.lineTo(x,y):ctx.moveTo(x,y));
      ctx.globalAlpha=alpha;ctx.strokeStyle=color;ctx.lineWidth=width;ctx.lineJoin="round";ctx.lineCap="round";ctx.setLineDash(dash);ctx.stroke();ctx.setLineDash([]);ctx.globalAlpha=1;
    }
    const ALL=[]; for(const utility of ["range","endurance"]) for(const group of D.utilities[utility])
      group.runs.forEach((run,vi)=>{ if(run) ALL.push({utility,y0:group.y0,vi,...run}); });
    const FIELD_VIEW={x:[-.5,3],y:[-1.5,2]};
    const PATH_Y=[0,niceMax(maxOf(ALL.flatMap(p=>p.states.map(s=>s.y+p.y0))))];
    const Z_TIME_Y=[0,niceMax(maxOf(ALL.flatMap(p=>p.states.map(s=>s.z))))];
    const VELOCITY_Y=steppedDomain(ALL.flatMap(p=>p.states.flatMap(s=>[s.vy,s.vz])),.5,true);

    function niceMax(value) {
      const raw=value/4, magnitude=10**Math.floor(Math.log10(raw)), q=raw/magnitude;
      const step=(q<=1?1:q<=2?2:q<=2.5?2.5:q<=5?5:10)*magnitude;
      return Math.ceil(value/step)*step;
    }
    function selected() {
      const utility=utilityBox.checked?"endurance":"range", y0=+slider.value, group=D.utilities[utility][y0-1];
      const vi=+variantSelect.value, run=group.runs[vi];
      // Variants drawn at this y0: the shown ones and the selected one, where a file exists.
      // The time and distance axes fit those, so a hidden long flight does not squash them.
      const drawn=V.map((_,i)=>i).filter(i=>group.runs[i]&&(i===vi||shown.has(i)));
      const pool=(drawn.length?drawn:V.map((_,i)=>i).filter(i=>group.runs[i])).map(i=>group.runs[i]);
      const timeMax=pool.length?niceMax(maxOf(pool.map(p=>p.states.at(-1).t))):100;
      const distanceMax=pool.length?niceMax(maxOf(pool.map(p=>p.states.at(-1).z))):50;
      return {utility,y0,group,vi,run,drawn,timeMax,distanceMax};
    }
    // Draw order: every y0 for the selected variant (faint), the shown variants at the
    // selected y0 (their accent colors), then the selected run (white) on top.
    function layers(sel) {
      const out=ALL.filter(p=>p.vi===sel.vi).map(p=>({p,y0:p.y0,color:SELECTED,width:.65,alpha:.05,dash:[],vz:VZ_DASH.solid,faint:true}));
      for(const vi of sel.drawn) if(vi!==sel.vi) out.push({p:sel.group.runs[vi],y0:sel.y0,color:V_COLORS[vi],width:1.8,alpha:.95,dash:DASH[V[vi].style],vz:VZ_DASH[V[vi].style]});
      if(sel.run) out.push({p:sel.run,y0:sel.y0,color:SELECTED,width:2.6,alpha:1,dash:[],vz:VZ_DASH.solid});
      return out;
    }
    const throughTime=(p,max)=>p.states.filter(s=>s.t<=max);
    const throughDistance=(p,max)=>p.states.filter(s=>s.z<=max);
    // Floor (h = 0); with `margin`, also the fixed bubble's margin (h = 0.5), which is only
    // visible on the near-floor panel's scale.
    function floorLine(s, margin=false) { return ({Y})=>{
      for(const [h,style] of [[0,"rgba(230,233,237,.65)"],...(margin?[[.5,"rgba(230,233,237,.55)"]]:[])]) {
        s.ctx.strokeStyle=style;s.ctx.lineWidth=1;s.ctx.setLineDash(h?[2,4]:[5,4]);s.ctx.beginPath();s.ctx.moveTo(s.m.l,Y(h));s.ctx.lineTo(s.w-s.m.r,Y(h));s.ctx.stroke();
      }
      s.ctx.setLineDash([]);
    }; }
    function drawPitch(sel) {
      const s=setup(root.querySelector("#floor-pitch")), sc=axes(s,[0,sel.timeMax],[90,-90],"tick","pitch, degrees",0,0);
      for(const L of layers(sel)) line(sc.ctx,throughTime(L.p,sel.timeMax).filter(v=>v.pitch!==null).map(v=>[sc.X(v.t),sc.Y(v.pitch)]),L.color,L.width,L.alpha,L.dash);
    }
    function drawHeight(sel) {
      const s=setup(root.querySelector("#floor-height"));
      const sc=axes(s,[0,sel.timeMax],PATH_Y,"tick","height above floor, blocks",0,0,floorLine(s));
      for(const L of layers(sel)) line(sc.ctx,throughTime(L.p,sel.timeMax).map(v=>[sc.X(v.t),sc.Y(v.y+L.y0)]),L.color,L.width,L.alpha,L.dash);
    }
    function drawNear(sel) {
      const s=setup(root.querySelector("#floor-near"));
      const sc=axes(s,[0,sel.timeMax],[-.25,2],"tick","h, blocks",0,1,floorLine(s,true),null,.5);
      s.ctx.save();s.ctx.beginPath();s.ctx.rect(s.m.l,s.m.t,s.w-s.m.l-s.m.r,s.h-s.m.t-s.m.b);s.ctx.clip();
      for(const L of layers(sel)) line(sc.ctx,throughTime(L.p,sel.timeMax).map(v=>[sc.X(v.t),sc.Y(v.y+L.y0)]),L.color,L.width,L.alpha,L.dash);
      s.ctx.restore();
    }
    function drawZTime(sel) {
      const s=setup(root.querySelector("#floor-ztime")), sc=axes(s,[0,sel.timeMax],Z_TIME_Y,"tick","z, blocks",0,0);
      for(const L of layers(sel)) line(sc.ctx,throughTime(L.p,sel.timeMax).map(v=>[sc.X(v.t),sc.Y(v.z)]),L.color,L.width,L.alpha,L.dash);
    }
    function drawVelocity(sel) {
      const s=setup(root.querySelector("#floor-velocity")), sc=axes(s,[0,sel.timeMax],VELOCITY_Y,"tick","velocity, blocks/tick",0,1,null,null,.5);
      for(const L of layers(sel)) {
        const states=throughTime(L.p,sel.timeMax), alpha=L.faint?.035:L.alpha;
        line(sc.ctx,states.map(v=>[sc.X(v.t),sc.Y(v.vy)]),L.color,L.width,alpha,L.dash);
        line(sc.ctx,states.map(v=>[sc.X(v.t),sc.Y(v.vz)]),L.color,L.width,alpha,L.faint?[4,3]:L.vz);
      }
    }
    function drawField(sel) {
      const s=setup(root.querySelector("#floor-field"));
      const sc=axes(s,FIELD_VIEW.x,FIELD_VIEW.y,"vz, blocks/tick","vy, blocks/tick",1,1,({X,Y})=>{
        if(fieldImage.complete&&fieldImage.naturalWidth) s.ctx.drawImage(fieldImage,X(FIELD_VZ[0]),Y(FIELD_VY[1]),X(FIELD_VZ[1])-X(FIELD_VZ[0]),Y(FIELD_VY[0])-Y(FIELD_VY[1]));
      },.5,.5);
      s.ctx.save();s.ctx.beginPath();s.ctx.rect(s.m.l,s.m.t,s.w-s.m.l-s.m.r,s.h-s.m.t-s.m.b);s.ctx.clip();
      for(const L of layers(sel)) line(sc.ctx,L.p.states.filter(v=>v.pitch!==null).map(v=>[sc.X(v.vz),sc.Y(v.vy)]),L.color,L.width,L.alpha,L.dash);
      if(sel.run){const a=sel.run.states[0];s.ctx.beginPath();s.ctx.arc(sc.X(a.vz),sc.Y(a.vy),3.5,0,Math.PI*2);s.ctx.fillStyle=SELECTED;s.ctx.fill();}
      s.ctx.restore();
    }
    function drawPath(sel) {
      const s=setup(root.querySelector("#floor-path"));
      const sc=axes(s,[0,sel.distanceMax],PATH_Y,"z, blocks","height above floor, blocks",0,0,floorLine(s));
      for(const L of layers(sel)) line(sc.ctx,throughDistance(L.p,sel.distanceMax).map(v=>[sc.X(v.z),sc.Y(v.y+L.y0)]),L.color,L.width,L.alpha,L.dash);
    }
    function drawUtility(sel, utility) {
      const canvas=root.querySelector(utility==="range"?"#floor-range":"#floor-endurance"), s=setup(canvas,{l:64,r:15,t:12,b:36});
      const value=p=>utility==="range"?p.z_exit:p.t_exit;
      const groups=D.utilities[utility], order=[...shown].filter(vi=>vi!==sel.vi).sort((a,b)=>a-b).concat([sel.vi]);
      const values=groups.flatMap(g=>order.map(vi=>g.runs[vi]).filter(Boolean).map(value));
      if(!values.length) values.push(0,1);
      const yAxis=niceAxis(values,4,true);
      const sc=axes(s,[1,32],yAxis.domain,"initial height y0",utility==="range"?"range, blocks":"endurance, ticks",0,tickDigits(yAxis.step),({X})=>{s.ctx.strokeStyle="rgba(230,233,237,.42)";s.ctx.setLineDash([4,3]);s.ctx.beginPath();s.ctx.moveTo(X(sel.y0),s.m.t);s.ctx.lineTo(X(sel.y0),s.h-s.m.b);s.ctx.stroke();s.ctx.setLineDash([]);},null,yAxis.step);
      const active=sel.utility===utility;
      for(const vi of order) {
        const chosen=vi===sel.vi, color=chosen?SELECTED:V_COLORS[vi];
        // A missing y0 breaks the line rather than bridging it.
        let segment=[];
        const flush=()=>{line(sc.ctx,segment,color,chosen?(active?2.4:1.6):(active?1.5:1.1),active?(chosen?1:.9):(chosen?.7:.5),chosen?[]:DASH[V[vi].style]);segment=[];};
        for(const g of groups) { if(g.runs[vi]) segment.push([sc.X(g.y0),sc.Y(value(g.runs[vi]))]); else flush(); }
        flush();
        const here=groups[sel.y0-1].runs[vi]; if(!here) continue;
        const x=sc.X(sel.y0), y=sc.Y(value(here));
        s.ctx.globalAlpha=active?1:.55;s.ctx.beginPath();s.ctx.arc(x,y,chosen?(active?4.5:3.5):2.5,0,Math.PI*2);s.ctx.fillStyle=color;s.ctx.fill();s.ctx.globalAlpha=1;
      }
    }
    // Per-variant summary for the highlighted utility's files: values at the selected y0, and
    // sums over the y0 that have a file. Δ is the utility minus the reference variant's (row 0),
    // summed over the y0 both have. Bold: best in column (sums only among rows with every y0).
    function drawSummary(sel) {
      const u=sel.utility, groups=D.utilities[u], value=p=>u==="range"?p.z_exit:p.t_exit;
      const rows=V.map((v,vi)=>{
        const here=sel.group.runs[vi], runs=groups.map(g=>g.runs[vi]).filter(Boolean);
        const sum=f=>runs.reduce((a,p)=>a+f(p),0);
        const both=groups.filter(g=>g.runs[vi]&&g.runs[0]);
        return {vi,here,n:runs.length,t:sum(p=>p.t_exit),z:sum(p=>p.z_exit),rough:sum(p=>p.rough),chatter:sum(p=>p.chatter),
                delta:vi===0?null:both.length?both.reduce((a,g)=>a+value(g.runs[vi])-value(g.runs[0]),0):null};
      });
      const full=Math.max(...rows.map(r=>r.n));
      const best=(list,f,dir)=>{const vals=list.map(f).filter(x=>x!==null&&x!==undefined);return vals.length?(dir>0?Math.max(...vals):Math.min(...vals)):null;};
      const withHere=rows.filter(r=>r.here), complete=rows.filter(r=>r.n===full);
      const B={ht:best(withHere,r=>r.here.t_exit,1),hz:best(withHere,r=>r.here.z_exit,1),hr:best(withHere,r=>r.here.rough,-1),hc:best(withHere,r=>r.here.chatter,-1),
               t:best(complete,r=>r.t,1),z:best(complete,r=>r.z,1),r:best(complete,r=>r.rough,-1),c:best(complete,r=>r.chatter,-1)};
      const cell=(x,digits,b,cls="")=>`<td class="${cls}${x!==null&&b!==null&&Math.abs(x-b)<1e-9?" best":""}">${x===null?"–":x.toFixed(digits)}</td>`;
      const isComplete=r=>r.n===full;
      summaryCaption.textContent=`summary · ${u==="range"?"range: dist files":"endurance: time files"} (toggle "highlight endurance") · bold = best in column · click a row to select it`;
      let html=`<thead><tr><th></th><th colspan="4">at y0 = ${sel.y0}</th><th class="split" colspan="6">over y0 1–32</th></tr>
        <tr><th>variant</th><th>t*</th><th>z(t*)</th><th>rough</th><th>chatter</th><th class="split">Σ t*</th><th>Σ z(t*)</th><th>Σ rough</th><th>Σ chatter</th><th title="Σ (${u==="range"?"z(t*)":"t*"} − ${V[0].label}'s) over the y0 both have">Δ ${u==="range"?"z":"t*"} vs ref</th><th>files</th></tr></thead><tbody>`;
      let lastGroup=null;
      for(const r of rows) {
        const v=V[r.vi];
        if(v.group!==lastGroup){ lastGroup=v.group; html+=`<tr class="group"><td colspan="11">${D.groups[v.group]}</td></tr>`; }
        const h=r.here, c=isComplete(r), cls=[r.vi===sel.vi?"selected":"",r.vi!==sel.vi&&!shown.has(r.vi)?"hidden":""].join(" ");
        html+=`<tr class="${cls}" data-vi="${r.vi}"><td>${swatch(r.vi)} ${v.label}${r.vi===0?" (ref)":""}</td>`+
          cell(h?h.t_exit:null,2,B.ht)+cell(h?h.z_exit:null,2,B.hz)+cell(h?h.rough:null,1,B.hr)+cell(h?h.chatter:null,0,B.hc)+
          cell(r.n?r.t:null,1,c?B.t:null,"split")+cell(r.n?r.z:null,1,c?B.z:null)+cell(r.n?r.rough:null,0,c?B.r:null)+cell(r.n?r.chatter:null,0,c?B.c:null)+
          `<td>${r.delta===null?"–":(r.delta>=0?"+":"")+r.delta.toFixed(2)}</td><td>${r.n}/${D.heights.length}</td></tr>`;
      }
      summary.innerHTML=html+"</tbody>";
    }
    summary.addEventListener("click",event=>{
      const row=event.target.closest("tr[data-vi]"); if(!row) return;
      variantSelect.value=row.dataset.vi; update();
    });
    function update() {
      const sel=selected(), opt=sel.run, v=V[sel.vi], b=v.bubble;
      // No-bubble's label already says so; the bubble variants add their settings.
      const bubble=b?` · margin ${b[0]}, weight ${b[1]}, ${b[2]===1?"fixed":`×${b[2]} per pass`}`:"";
      const pen=v.pen?` · pen ${v.pen} · ${v.method?`method ${v.method}`:`moves ${v.moves}`}`:"";
      const score=!opt?"no file for this y0":`${sel.utility==="range"?`z(t*) ${opt.z_exit.toFixed(3)} blocks`:`t* ${opt.t_exit.toFixed(3)} ticks`} · exit KE ${opt.ke.toFixed(4)} · passes ${opt.passes} · rough ${opt.rough.toFixed(1)} · chatter ${opt.chatter}`;
      readout.textContent=`y0 = ${sel.y0} · ${sel.utility} · ${v.label}${bubble}${pen} · ${score}`;
      legendSelected.textContent=`${v.label} (selected)`;
      legendOthers.innerHTML=[...shown].filter(vi=>vi!==sel.vi).sort((a,b)=>a-b)
        .map(vi=>`<span class="key">${swatch(vi)}${V[vi].label}${sel.group.runs[vi]?"":" (no file)"}</span>`).join("");
      shownBox.querySelectorAll(".chip").forEach(chip=>{ const sel_=+chip.dataset.vi===sel.vi; chip.classList.toggle("selected",sel_); chip.title=sel_?"selected: drawn in white regardless":""; });
      try { drawPitch(sel); drawHeight(sel); drawNear(sel); drawZTime(sel); drawVelocity(sel); drawField(sel); drawPath(sel); drawUtility(sel,"range"); drawUtility(sel,"endurance"); drawSummary(sel); }
      catch(error) { window.__floorError = String(error.stack || error); throw error; }
    }
    slider.addEventListener("input",update);variantSelect.addEventListener("change",update);utilityBox.addEventListener("change",update);window.addEventListener("resize",update);fieldImage.addEventListener("load",update);update();
  })();
  </script>
</div>
'''


def main():
    data = json.dumps(load_data(), separators=(",", ":"), allow_nan=False)
    html = (HTML.replace("__DATA__", data).replace("__FIELD__", load_field())
            .replace("__FIELD_VZ__", json.dumps(FIELD_VZ))
            .replace("__FIELD_VY__", json.dumps(FIELD_VY))
            .replace("__CHATTER__", f"{CHATTER_STEP:g}"))
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(html)
    print(f"{OUT}  {OUT.stat().st_size / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
