#!/usr/bin/env python3
"""Visualize the best profiles in runs/atlas/mapfine.

The corpus has many optimized initializations in each (n, lambda) cell.  best.csv has already
performed the within-cell argmax over dJ; this script applies post-hoc feasibility constraints
to those cell winners and produces the four requested boundary branches.  The interactive
browser instead assigns every certified profile an empirical implied lambda as documented in
docs/implied-lambda.md, and can display either the feasible or unconstrained population.

    python3 tools/plot_mapfine.py
    python3 tools/plot_mapfine.py RUN_ROOT OUT_DIR
"""

import base64
import csv
import glob
import gzip
import io
import json
import math
import os
import statistics
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.patheffects
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from load import load
from plot_field_replay import (FIELD_SCALE, FIELD_ZERO, HALO, VX_HI, VX_LO, VY_HI,
                               VY_LO, best_gain, field_rgb, replay)


ROOT = sys.argv[1] if len(sys.argv) > 1 else "runs/atlas/mapfine"
OUT = sys.argv[2] if len(sys.argv) > 2 else "runs/atlas/fig/mapfine"
N_LO, N_HI = 100, 200

BRANCHES = (
    ("min_dy_given_dy", "MINIMIZE dy\nconstraint: dy > 0", "dy",
     lambda r: r["dy"] > 0),
    ("min_dz_given_dz", "MINIMIZE dz\nconstraint: dz > 150", "dz",
     lambda r: r["dz"] > 150),
    ("min_dy_given_both", "MINIMIZE dy\nconstraints: dy > 0, dz > 150", "dy",
     lambda r: r["dy"] > 0 and r["dz"] > 150),
    ("min_dz_given_both", "MINIMIZE dz\nconstraints: dy > 0, dz > 150", "dz",
     lambda r: r["dy"] > 0 and r["dz"] > 150),
)


def theme():
    plt.style.use("dark_background")
    plt.rcParams.update({
        "figure.facecolor": "#14171a", "savefig.facecolor": "#14171a",
        "axes.facecolor": "#1b1f24", "axes.edgecolor": "#4a525c",
        "grid.color": "#5d6774", "text.color": "#e6e9ed",
        "axes.labelcolor": "#e6e9ed", "xtick.color": "#b9c0c9",
        "ytick.color": "#b9c0c9",
    })


def read_rows():
    rows = []
    with open(os.path.join(ROOT, "best.csv"), newline="") as fh:
        for raw in csv.DictReader(fh):
            row = dict(raw)
            row.update(n=int(raw["n"]), lam=float(raw["lam"]), dJ=float(raw["dJ"]),
                       dy=float(raw["dy"]), dz=float(raw["dz"]))
            if N_LO <= row["n"] <= N_HI:
                row["path"] = os.path.join(ROOT, "out", row["cell"], row["file"])
                rows.append(row)
    return rows


def select_branches(rows):
    ns = sorted({r["n"] for r in rows})
    selected = {}
    for key, _, metric, predicate in BRANCHES:
        branch = []
        for n in ns:
            candidates = [r for r in rows if r["n"] == n and predicate(r)]
            if candidates:
                branch.append(min(candidates, key=lambda r: (r[metric], r["lam"])))
        selected[key] = branch

        # Keep the plotted definition executable: every selected row must satisfy its branch,
        # and no other cell winner at that n may have a smaller target value.
        for chosen in branch:
            candidates = [r for r in rows if r["n"] == chosen["n"] and predicate(r)]
            expected = min(candidates, key=lambda r: (r[metric], r["lam"]))
            assert chosen["cell"] == expected["cell"], (key, chosen["n"], chosen["cell"],
                                                         expected["cell"])
    return selected


def plot_branch_values(selected):
    fig, axes = plt.subplots(4, 3, figsize=(15, 14), sharex="col")
    metrics = (("lam", "selected lambda"), ("dy", "selected height dy, blocks"),
               ("dz", "selected distance dz, blocks"))
    colors = ("#55c1ff", "#52d273", "#f6b44b")
    for i, (key, label, target, _) in enumerate(BRANCHES):
        rows = selected[key]
        for j, ((metric, ylabel), color) in enumerate(zip(metrics, colors)):
            ax = axes[i, j]
            ax.plot([r["n"] for r in rows], [r[metric] for r in rows], "o-", ms=3,
                    lw=1.25, color=color)
            ax.grid(alpha=.25)
            ax.set_ylabel(ylabel)
            if i == 0:
                ax.set_title(ylabel)
            if i == len(BRANCHES) - 1:
                ax.set_xlabel("num_ticks")
            if j == 0:
                ax.text(.02, .96, label, transform=ax.transAxes, va="top", fontsize=9,
                        color="#e6e9ed")
            if metric in ("dy", "dz"):
                ax.axhline(0, color="0.6", lw=.8, zorder=0)
            if metric == target:
                ax.text(.98, .96, "minimized", transform=ax.transAxes, ha="right", va="top",
                        fontsize=8, color=color)
        if rows:
            missing = len({r["n"] for b in selected.values() for r in b}) - len(rows)
            if missing:
                absent = sorted(set(range(120, 201, 2)) - {r["n"] for r in rows})
                span = (f"n={absent[0]}-{absent[-1]}" if absent else f"{missing} n values")
                axes[i, 2].text(.98, .04, f"No cell winner satisfies this row at {span}",
                                transform=axes[i, 2].transAxes, ha="right", va="bottom",
                                fontsize=8, color="#b9c0c9")
    fig.suptitle("mapfine boundary selections after choosing the maximum-dJ profile in each cell",
                 fontsize=14)
    plt.tight_layout()
    path = os.path.join(OUT, "branches-values.png")
    plt.savefig(path, dpi=140)
    plt.close(fig)
    return path


def load_branch_profiles(selected):
    return {key: [(row, load(row["path"])) for row in rows] for key, rows in selected.items()}


def plot_branch_pitches(profiles):
    fig, axes = plt.subplots(2, 2, figsize=(15, 10), sharex=False, sharey=True)
    norm = matplotlib.colors.Normalize(N_LO, N_HI)
    cmap = plt.get_cmap("viridis")
    for ax, (key, label, _, _) in zip(axes.ravel(), BRANCHES):
        for row, profile in profiles[key]:
            ax.plot(profile.pitches, color=cmap(norm(row["n"])), lw=.85, alpha=.78)
        ax.axhline(0, color="0.6", lw=.7)
        ax.set_ylim(90, -90)
        ax.set_title(f"{label}  ({len(profiles[key])} profiles)", fontsize=10)
        ax.set_xlabel("tick")
        ax.set_ylabel("pitch, degrees (nose-up at top)")
        ax.grid(alpha=.22)
    sm = matplotlib.cm.ScalarMappable(norm=norm, cmap=cmap)
    fig.colorbar(sm, ax=axes.ravel().tolist(), label="num_ticks", pad=.012, fraction=.025)
    fig.suptitle("chosen boundary profiles: pitch through time", fontsize=14)
    fig.subplots_adjust(left=.07, right=.91, bottom=.07, top=.92, wspace=.16, hspace=.24)
    path = os.path.join(OUT, "branches-pitch.png")
    plt.savefig(path, dpi=140)
    plt.close(fig)
    return path


def field_grid(width=430, height=430):
    vz_grid, vy_grid = np.meshgrid(np.linspace(VX_LO, VX_HI, width),
                                   np.linspace(VY_HI, VY_LO, height))
    return vz_grid, vy_grid, best_gain(vy_grid, vz_grid)


def plot_branch_field(profiles, field):
    _, _, gains = field
    fig, axes = plt.subplots(2, 2, figsize=(13, 12), sharex=True, sharey=True)
    norm = matplotlib.colors.Normalize(N_LO, N_HI)
    cmap = plt.get_cmap("viridis")
    halo = [matplotlib.patheffects.withStroke(linewidth=2.0, foreground=HALO, alpha=.85)]
    for ax, (key, label, _, _) in zip(axes.ravel(), BRANCHES):
        ax.imshow(field_rgb(gains), extent=(VX_LO, VX_HI, VY_LO, VY_HI), origin="upper",
                  interpolation="nearest", aspect="equal", zorder=0)
        for row, profile in profiles[key]:
            vy, vz = replay(profile)
            ax.plot(vz, vy, color=cmap(norm(row["n"])), lw=.9, alpha=.82, zorder=2,
                    path_effects=halo)
        ax.set_xlim(VX_LO, VX_HI)
        ax.set_ylim(VY_LO, VY_HI)
        ax.set_xticks(np.arange(VX_LO, VX_HI + .001, .5))
        ax.set_yticks(np.arange(VY_LO, VY_HI + .001, .5))
        ax.set_title(f"{label}  ({len(profiles[key])} profiles)", fontsize=10)
        ax.set_xlabel("vz, blocks/tick")
        ax.set_ylabel("vy, blocks/tick")
        ax.grid(color="white", alpha=.14, lw=.6)
        ax.axvline(0, color="white", alpha=.30, lw=.9)
        ax.axhline(0, color="white", alpha=.30, lw=.9)
    sm = matplotlib.cm.ScalarMappable(norm=norm, cmap=cmap)
    fig.colorbar(sm, ax=axes.ravel().tolist(), label="num_ticks", pad=.012, fraction=.025)
    fig.suptitle("chosen boundary profiles replayed over the one-tick energy field", fontsize=14)
    fig.subplots_adjust(left=.07, right=.90, bottom=.06, top=.92, wspace=.15, hspace=.20)
    path = os.path.join(OUT, "branches-field.png")
    plt.savefig(path, dpi=140)
    plt.close(fig)
    return path


def field_data_url(gains):
    buf = io.BytesIO()
    plt.imsave(buf, field_rgb(gains), format="png", origin="upper")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")


LAMBDA_LO, LAMBDA_HI = 0.0, 6.0
LAMBDA_SCALE = 21.5 / 330.0


def compact_profile(profile, cell, implied_lambda, rank, all_rank, utility, regret, feasible):
    return {
        "lambda": round(implied_lambda, 8),
        "statedLambda": profile.lam,
        "dy": round(float(profile.header["dy"].split()[0]), 5),
        "dz": round(float(profile.header["dz"].split()[0]), 5),
        "utility": round(utility, 5),
        "regret": round(regret, 5),
        "cell": cell,
        "profile": os.path.basename(profile.path),
        "rank": rank,
        "allRank": all_rank,
        "feasible": feasible,
        "pitch": [round(v, 2) for v in profile.pitches],
    }


def upper_envelope(lines):
    """Upper envelope of intercept+slope*lambda as (slope, intercept), with start points."""
    by_slope = {}
    for slope, intercept, *_ in lines:
        by_slope[slope] = max(intercept, by_slope.get(slope, -math.inf))
    hull, starts = [], []
    for slope, intercept in sorted(by_slope.items()):
        start = -math.inf
        while hull:
            prev_slope, prev_intercept = hull[-1]
            start = (prev_intercept - intercept) / (slope - prev_slope)
            if start <= starts[-1]:
                hull.pop()
                starts.pop()
            else:
                break
        if not hull:
            start = -math.inf
        hull.append((slope, intercept))
        starts.append(start)
    return hull, starts


def implied_lambda(slope, hull, starts):
    """Continuous lambda in [0, 6] minimizing envelope regret for a line of this slope."""
    active = []
    for i, (hull_slope, _) in enumerate(hull):
        lo = max(LAMBDA_LO, starts[i])
        hi = min(LAMBDA_HI, starts[i + 1] if i + 1 < len(starts) else math.inf)
        if lo <= hi:
            active.append((hull_slope, lo, hi))
    if slope < active[0][0]:
        return LAMBDA_LO
    for i, (hull_slope, lo, hi) in enumerate(active):
        if math.isclose(slope, hull_slope, rel_tol=0, abs_tol=1e-12):
            return (lo + hi) / 2
        if i + 1 < len(active) and hull_slope < slope < active[i + 1][0]:
            return active[i + 1][1]
    return LAMBDA_HI


def compact_population(rows):
    """Assign implied lambda from all profiles; rank both populations at that lambda."""
    profiles_by_n = {}
    for row in rows:
        for path in glob.glob(os.path.join(ROOT, "out", row["cell"], "tight_t*.pitches")):
            profile = load(path)
            if "certified" in profile.header:
                dte = float(profile.header["dte"].split()[0])
                dz = float(profile.header["dz"].split()[0])
                profiles_by_n.setdefault(row["n"], []).append(
                    (LAMBDA_SCALE * dz, dte, profile, row["cell"]))

    out = {}
    for n, lines in profiles_by_n.items():
        hull, starts = upper_envelope(lines)
        envelope_values = {}
        all_groups = {}
        feasible_groups = {}
        for slope, intercept, profile, cell in lines:
            lam = implied_lambda(slope, hull, starts)
            key = round(lam, 8)
            if key not in envelope_values:
                envelope_values[key] = max(a + m * lam for m, a in hull)
            utility = intercept + slope * lam
            dy = float(profile.header["dy"].split()[0])
            dz = float(profile.header["dz"].split()[0])
            entry = {
                "utility": utility, "profile": profile, "cell": cell,
                "regret": envelope_values[key] - utility,
                "feasible": dy > 0 and dz > 150,
            }
            all_groups.setdefault(key, []).append(entry)
            if entry["feasible"]:
                feasible_groups.setdefault(key, []).append(entry)

        for group in all_groups.values():
            group.sort(key=lambda item: item["utility"], reverse=True)
            for rank, entry in enumerate(group, 1):
                entry["all_rank"] = rank
        for group in feasible_groups.values():
            group.sort(key=lambda item: item["utility"], reverse=True)
            for rank, entry in enumerate(group, 1):
                entry["rank"] = rank

        compact = []
        for key, group in all_groups.items():
            for entry in group:
                compact.append(compact_profile(
                    entry["profile"], entry["cell"], key, entry.get("rank"),
                    entry["all_rank"], entry["utility"], entry["regret"], entry["feasible"]))
        compact.sort(key=lambda r: (r["lambda"], r["allRank"]))
        out[str(n)] = compact
    return out


def reference_profile(data):
    """Best profile at the lower-median implied lambda of the earliest feasible horizon."""
    n = min(int(key) for key, rows in data.items() if any(r["feasible"] for r in rows))
    horizon = data[str(n)]
    feasible = [r for r in horizon if r["feasible"]]
    lam = statistics.median_low(sorted({r["lambda"] for r in feasible}))
    row = next(r for r in feasible if r["lambda"] == lam and r["rank"] == 1)
    return {"n": n, **row}


def write_interactive(rows, gains):
    data = compact_population(rows)
    ns = sorted(map(int, data))
    feasible_ns = sorted(int(key) for key, group in data.items()
                         if any(r["feasible"] for r in group))
    reference = reference_profile(data)
    max_depth = max(max(r["allRank"] for r in group) for group in data.values())
    payload = json.dumps({"data": data, "reference": reference}, separators=(",", ":"))
    packed = base64.b64encode(gzip.compress(payload.encode("utf-8"), compresslevel=9)).decode("ascii")
    template = r'''<div id="mapfine-explorer">
  <style>
    html, body { margin: 0; background: #14171a; }
    #mapfine-explorer { box-sizing: border-box; color: #e6e9ed; background: #14171a; font: 14px system-ui, sans-serif; padding: 16px; width: 100%; height: 100vh; overflow: hidden; display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; }
    #mapfine-explorer * { box-sizing: border-box; }
    #mapfine-explorer h1 { font-size: 20px; font-weight: 500; margin: 0 0 12px; }
    #mapfine-explorer .control-stack { margin-bottom: 10px; }
    #mapfine-explorer .controls { display: grid; grid-template-columns: auto minmax(180px, 1fr) auto; gap: 8px 12px; align-items: center; }
    #mapfine-explorer .controls.secondary { grid-template-columns: auto minmax(180px, 1fr) auto auto auto; margin-top: 5px; }
    #mapfine-explorer input[type="range"] { width: 100%; }
    #mapfine-explorer .toggle { display: flex; align-items: center; gap: 6px; white-space: nowrap; }
    #mapfine-explorer .readout { font-variant-numeric: tabular-nums; min-width: 190px; text-align: right; }
    #mapfine-explorer .grid { min-height: 0; display: grid; grid-template-columns: minmax(0, 1.25fr) minmax(360px, .9fr); grid-template-rows: repeat(3, minmax(0, 1fr)); grid-template-areas: "field pitch" "field dy" "field dz"; gap: 12px; }
    #mapfine-explorer figure { margin: 0; min-width: 0; min-height: 0; display: grid; grid-template-rows: auto minmax(0, 1fr); }
    #mapfine-explorer figure.field { grid-area: field; }
    #mapfine-explorer figure.pitch { grid-area: pitch; }
    #mapfine-explorer figure.dy { grid-area: dy; }
    #mapfine-explorer figure.dz { grid-area: dz; }
    #mapfine-explorer figcaption { margin: 0 0 4px; color: #b9c0c9; }
    #mapfine-explorer canvas { display: block; width: 100%; height: 100%; min-height: 0; background: #1b1f24; border: 1px solid #4a525c; }
    #mapfine-explorer .detail { min-height: 20px; margin-top: 10px; color: #b9c0c9; font-variant-numeric: tabular-nums; }
    @media (max-width: 900px) { #mapfine-explorer { height: auto; min-height: 0; overflow: visible; display: block; } #mapfine-explorer .controls.secondary { grid-template-columns: auto minmax(160px, 1fr) auto; } #mapfine-explorer .toggle { grid-column: 1 / -1; } #mapfine-explorer .grid { display: flex; flex-direction: column; gap: 16px; } #mapfine-explorer figure, #mapfine-explorer figure.field, #mapfine-explorer figure.pitch, #mapfine-explorer figure.dy, #mapfine-explorer figure.dz { grid-area: auto; display: block; } #mapfine-explorer canvas { height: auto; aspect-ratio: 1.55; } #mapfine-explorer figure.field canvas { aspect-ratio: 1; } }
    @media (max-width: 700px) { #mapfine-explorer .controls, #mapfine-explorer .controls.secondary { grid-template-columns: 1fr; } #mapfine-explorer .toggle { grid-column: auto; } #mapfine-explorer .readout { text-align: left; } }
  </style>
  <h1>mapfine profiles</h1>
  <div class="control-stack">
    <div class="controls">
      <label for="mapfine-n">num_ticks</label>
      <input id="mapfine-n" type="range" min="0" max="__MAX__" value="__START__" step="1">
      <output id="mapfine-readout" class="readout" aria-live="polite"></output>
    </div>
    <div class="controls secondary">
      <label for="mapfine-depth">profiles per implied lambda</label>
      <input id="mapfine-depth" type="range" min="1" max="__DEPTH__" value="1" step="1">
      <output id="mapfine-depth-readout" class="readout" aria-live="polite">best 1</output>
      <label class="toggle"><input id="mapfine-constrained" type="checkbox"> show only feasible profiles</label>
      <label class="toggle"><input id="mapfine-reference" type="checkbox"> show n=__REFN__, lambda=__REFLAM__ reference</label>
    </div>
  </div>
  <div class="grid">
    <figure class="field"><figcaption>replay on the energy field</figcaption><canvas id="mf-field" aria-label="velocity replay over the one-tick energy field"></canvas></figure>
    <figure class="pitch"><figcaption>pitch vs time</figcaption><canvas id="mf-pitch" aria-label="pitch versus tick for selected profiles"></canvas></figure>
    <figure class="dy"><figcaption>implied lambda vs dy</figcaption><canvas id="mf-dy" aria-label="implied lambda versus dy for selected profiles"></canvas></figure>
    <figure class="dz"><figcaption>implied lambda vs dz</figcaption><canvas id="mf-dz" aria-label="implied lambda versus dz for selected profiles"></canvas></figure>
  </div>
  <div id="mf-detail" class="detail" aria-live="polite"></div>
  <script>
  (async () => {
    const PACKED = "__PACKED__";
    const NS = __NS__;
    const FIELD = "__FIELD__";
    const bytes = Uint8Array.from(atob(PACKED), c => c.charCodeAt(0));
    const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
    const payload = JSON.parse(await new Response(stream).text());
    const DATA = payload.data, REFERENCE = payload.reference;
    const root = document.getElementById("mapfine-explorer");
    const slider = root.querySelector("#mapfine-n");
    const depthSlider = root.querySelector("#mapfine-depth");
    const depthReadout = root.querySelector("#mapfine-depth-readout");
    const constrainedToggle = root.querySelector("#mapfine-constrained");
    const referenceToggle = root.querySelector("#mapfine-reference");
    const readout = root.querySelector("#mapfine-readout");
    const detail = root.querySelector("#mf-detail");
    const fieldImage = new Image(); fieldImage.src = FIELD;
    let selected = null;
    const charts = {};
    const colors = [
      [68,1,84],[72,35,116],[64,67,135],[52,94,141],[41,120,142],
      [32,144,140],[34,168,132],[68,190,112],[121,209,81],[189,223,38],[253,231,37]
    ];
    function color(t, a=1) {
      t = Math.max(0, Math.min(.999, t)); const q=t*(colors.length-1), i=Math.floor(q), f=q-i;
      const p=colors[i], n=colors[Math.min(i+1,colors.length-1)];
      return `rgba(${p.map((v,j)=>Math.round(v+(n[j]-v)*f)).join(",")},${a})`;
    }
    function replay(pitches) {
      let vy=0, vz=.4; const ys=[vy], zs=[vz];
      for (const pitch of pitches) {
        const lean=pitch*Math.PI/180, lookZ=Math.cos(lean), lookHor=Math.abs(lookZ), moveHor=Math.abs(vz), lift=lookZ*lookZ;
        vy += .08*(-1+lift*.75);
        if (lookHor>0) {
          const down=vy<0 ? vy*-.1*lift : 0; vy+=down; vz+=lookZ*down/lookHor;
          if (lean<0) { const up=moveHor*-Math.sin(lean)*.04; vy+=up*3.2; vz-=lookZ*up/lookHor; }
          vz += (lookZ/lookHor*moveHor-vz)*.1;
        }
        vy*=.9800000190734863; vz*=.9900000095367432; ys.push(vy); zs.push(vz);
      }
      return {vy:ys,vz:zs};
    }
    function setup(canvas) {
      const rect=canvas.getBoundingClientRect(), dpr=Math.min(devicePixelRatio||1,2);
      canvas.width=Math.round(rect.width*dpr); canvas.height=Math.round(rect.height*dpr);
      const ctx=canvas.getContext("2d"); ctx.setTransform(dpr,0,0,dpr,0,0);
      return {ctx,w:rect.width,h:rect.height,m:{l:58,r:14,t:14,b:42}};
    }
    function extent(values) { let lo=Math.min(...values), hi=Math.max(...values); if(lo===hi){lo-=.5;hi+=.5;} const p=(hi-lo)*.06; return [lo-p,hi+p]; }
    function axes(s, xd, yd, xl, yl, prepaint, tickStep=null) {
      const {ctx,w,h,m}=s, X=x=>m.l+(x-xd[0])/(xd[1]-xd[0])*(w-m.l-m.r), Y=y=>h-m.b-(y-yd[0])/(yd[1]-yd[0])*(h-m.t-m.b);
      if(prepaint) prepaint({X,Y});
      const ticks=(domain,step)=>step===null
        ? Array.from({length:5},(_,i)=>domain[0]+i*(domain[1]-domain[0])/4)
        : Array.from({length:Math.round((domain[1]-domain[0])/step)+1},(_,i)=>domain[0]+i*step);
      const gridStyle=value=>{if(tickStep===null){ctx.strokeStyle="#5d6774";ctx.lineWidth=1;return;}const zero=Math.abs(value)<1e-9;ctx.strokeStyle=zero?"rgba(230,233,237,.48)":"rgba(185,192,201,.24)";ctx.lineWidth=zero?1.25:1;};
      ctx.fillStyle="#b9c0c9";ctx.font="12px system-ui";
      for(const x of ticks(xd,tickStep)){const px=X(x);gridStyle(x);ctx.beginPath();ctx.moveTo(px,m.t);ctx.lineTo(px,h-m.b);ctx.stroke();ctx.textAlign="center";ctx.fillText(x.toFixed(tickStep===null?(xl.includes("lambda")?2:0):1),px,h-m.b+18);}
      for(const y of ticks(yd,tickStep)){const py=Y(y);gridStyle(y);ctx.beginPath();ctx.moveTo(m.l,py);ctx.lineTo(w-m.r,py);ctx.stroke();ctx.textAlign="right";ctx.fillText(y.toFixed(tickStep===null?(Math.abs(y)<10?1:0):1),m.l-7,py+4);}
      ctx.fillStyle="#e6e9ed";ctx.textAlign="center";ctx.fillText(xl,(m.l+w-m.r)/2,h-7);ctx.save();ctx.translate(14,(m.t+h-m.b)/2);ctx.rotate(-Math.PI/2);ctx.fillText(yl,0,0);ctx.restore();
      return {X,Y};
    }
    function colorFor(row, rows, alpha) {
      const values=rows.map(r=>r.lambda), lo=Math.min(...values), hi=Math.max(...values);
      return color(hi===lo?.5:(row.lambda-lo)/(hi-lo),alpha);
    }
    function plotScatter(id, rows, metric) {
      const canvas=root.querySelector("#"+id), s=setup(canvas), ctx=s.ctx; ctx.clearRect(0,0,s.w,s.h);
      const xd=rows.length?extent(rows.map(r=>r.lambda)):[0,6], yd=rows.length?extent(rows.map(r=>r[metric])):(metric==="dy"?[0,20]:[150,250]), sc=axes(s,xd,yd,"implied lambda",metric+", blocks");
      const isTop=r=>constrainedToggle.checked?r.rank===1:r.allRank===1;
      const best=rows.filter(isTop);
      if(best.length){ctx.strokeStyle="#55c1ff";ctx.lineWidth=1.2;ctx.beginPath();best.forEach((r,i)=>{const x=sc.X(r.lambda),y=sc.Y(r[metric]);i?ctx.lineTo(x,y):ctx.moveTo(x,y);});ctx.stroke();}
      rows.forEach((r,i)=>{ctx.beginPath();ctx.arc(sc.X(r.lambda),sc.Y(r[metric]),selected===i?5:(isTop(r)?3:2),0,Math.PI*2);ctx.fillStyle=selected===i?"#ffffff":colorFor(r,rows,isTop(r)?.95:.36);ctx.fill();});
      if(!rows.length){ctx.fillStyle="#b9c0c9";ctx.textAlign="center";ctx.fillText("no profiles at this depth",s.w/2,s.h/2);}
      charts[id]={canvas,rows,metric,sc};
    }
    function plotPitch(rows, selectedRow) {
      const canvas=root.querySelector("#mf-pitch"),s=setup(canvas),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const n=NS[+slider.value], yd=[90,-90], xd=[0,Math.max(n,referenceToggle.checked?REFERENCE.n:0)], sc=axes(s,xd,yd,"tick","pitch, degrees");
      function draw(row, stroke, width, dash=[]){ctx.beginPath();row.pitch.forEach((p,t)=>{const x=sc.X(t),y=sc.Y(p);t?ctx.lineTo(x,y):ctx.moveTo(x,y);});ctx.setLineDash(dash);ctx.strokeStyle=stroke;ctx.lineWidth=width;ctx.stroke();ctx.setLineDash([]);}
      if(referenceToggle.checked)draw(REFERENCE,"rgba(230,233,237,.48)",1.5,[6,4]);
      rows.forEach(r=>{if(r!==selectedRow)draw(r,colorFor(r,rows,(r.rank===1||r.allRank===1)?.62:.22),(r.rank===1||r.allRank===1)?.9:.65);});
      if(selectedRow)draw(selectedRow,"#ffffff",2.2);
    }
    function plotField(rows, selectedRow) {
      const canvas=root.querySelector("#mf-field"),s=setup(canvas),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const aw=s.w-s.m.l-s.m.r, ah=s.h-s.m.t-s.m.b;
      if(aw>ah){const d=(aw-ah)/2;s.m.l+=d;s.m.r+=d;}else{const d=(ah-aw)/2;s.m.t+=d;s.m.b+=d;}
      const xd=[-.5,3],yd=[-1.5,2],sc=axes(s,xd,yd,"vz, blocks/tick","vy, blocks/tick",q=>{const x0=q.X(xd[0]),y0=q.Y(yd[1]),x1=q.X(xd[1]),y1=q.Y(yd[0]);ctx.drawImage(fieldImage,x0,y0,x1-x0,y1-y0);},.5);
      function draw(row,stroke,width,dash=[]){const v=replay(row.pitch);ctx.beginPath();v.vy.forEach((y,t)=>{const x=sc.X(v.vz[t]),py=sc.Y(y);t?ctx.lineTo(x,py):ctx.moveTo(x,py);});ctx.setLineDash(dash);ctx.strokeStyle=stroke;ctx.lineWidth=width;ctx.stroke();ctx.setLineDash([]);}
      if(referenceToggle.checked)draw(REFERENCE,"rgba(230,233,237,.52)",1.6,[6,4]);
      rows.forEach(r=>{if(r!==selectedRow)draw(r,colorFor(r,rows,(r.rank===1||r.allRank===1)?.76:.24),(r.rank===1||r.allRank===1)?1:.7);});
      if(selectedRow)draw(selectedRow,"#ffffff",2.4);
    }
    function render() {
      const n=NS[+slider.value], depth=+depthSlider.value, all=DATA[n];
      const rows=constrainedToggle.checked
        ? all.filter(r=>r.feasible&&r.rank<=depth)
        : all.filter(r=>r.allRank<=depth);
      if(selected!==null&&selected>=rows.length)selected=null;
      const selectedRow=selected===null?null:rows[selected];
      const groups=new Set(rows.map(r=>r.lambda)).size;
      const population=constrainedToggle.checked?"feasible":"unconstrained";
      readout.textContent=`n = ${n}  \u00b7  ${rows.length} ${population} profiles across ${groups} implied lambdas`;
      depthReadout.textContent=depth===1?"top 1":"top "+depth;
      plotScatter("mf-dy",rows,"dy");plotScatter("mf-dz",rows,"dz");plotPitch(rows,selectedRow);plotField(rows,selectedRow);
      detail.textContent=selected===null?"Select a point in either lambda plot to highlight its schedule and replay.":(()=>{const r=rows[selected],rank=constrainedToggle.checked?r.rank:r.allRank,status=r.feasible?"feasible":"infeasible";return `implied lambda ${r.lambda.toFixed(3)}  \u00b7  stated lambda ${r.statedLambda.toFixed(2)}  \u00b7  rank ${rank}  \u00b7  ${status}  \u00b7  dy ${r.dy.toFixed(2)}  \u00b7  dz ${r.dz.toFixed(2)}  \u00b7  utility ${r.utility.toFixed(3)}  \u00b7  regret ${r.regret.toFixed(3)}  \u00b7  ${r.cell}/${r.profile}`;})();
    }
    function pick(event, chart) { const rect=chart.canvas.getBoundingClientRect(), x=event.clientX-rect.left,y=event.clientY-rect.top;let best=-1,dist=Infinity;chart.rows.forEach((r,i)=>{const dx=chart.sc.X(r.lambda)-x,dy=chart.sc.Y(r[chart.metric])-y,d=dx*dx+dy*dy;if(d<dist){dist=d;best=i;}});selected=dist<400?best:null;render(); }
    root.querySelector("#mf-dy").addEventListener("click",e=>pick(e,charts["mf-dy"]));
    root.querySelector("#mf-dz").addEventListener("click",e=>pick(e,charts["mf-dz"]));
    slider.addEventListener("input",()=>{selected=null;render();});
    depthSlider.addEventListener("input",()=>{selected=null;render();});
    constrainedToggle.addEventListener("change",()=>{selected=null;render();});
    referenceToggle.addEventListener("change",render);
    let timer;new ResizeObserver(()=>{clearTimeout(timer);timer=setTimeout(render,80);}).observe(root);
    fieldImage.onload=render; render();
  })();
  </script>
</div>'''
    html = (template.replace("__MAX__", str(len(ns) - 1))
            .replace("__START__", str(len(ns) // 2))
            .replace("__DEPTH__", str(max_depth))
            .replace("__REFN__", str(reference["n"]))
            .replace("__REFLAM__", f'{reference["lambda"]:.2f}')
            .replace("__PACKED__", packed)
            .replace("__NS__", json.dumps(ns))
            .replace("__FIELD__", field_data_url(gains)))
    path = os.path.join(OUT, "feasible-explorer.html")
    with open(path, "w") as fh:
        fh.write(html)
    feasible_count = sum(r["feasible"] for group in data.values() for r in group)
    return path, feasible_count, feasible_ns, max_depth, reference


def write_selection_csv(selected):
    path = os.path.join(OUT, "branch-selections.csv")
    fields = ["branch", "constraint", "metric", "cell", "n", "lam", "dJ", "dy", "dz", "file"]
    with open(path, "w", newline="") as fh:
        writer = csv.DictWriter(fh, fields)
        writer.writeheader()
        for key, label, metric, _ in BRANCHES:
            for row in selected[key]:
                writer.writerow({"branch": key, "constraint": label, "metric": metric,
                                 **{k: row[k] for k in fields if k in row}})
    return path


def main():
    os.makedirs(OUT, exist_ok=True)
    theme()
    rows = read_rows()
    selected = select_branches(rows)
    profiles = load_branch_profiles(selected)
    field = field_grid()
    paths = [write_selection_csv(selected), plot_branch_values(selected),
             plot_branch_pitches(profiles), plot_branch_field(profiles, field)]
    interactive, feasible_count, feasible_ns, max_depth, reference = write_interactive(rows,
                                                                                       field[2])
    paths.append(interactive)
    print(f"{len(rows)} cell winners in n=[{N_LO}, {N_HI}]")
    for key, label, _, _ in BRANCHES:
        print(f"  {key}: {len(selected[key])} num_ticks values")
    print(f"{feasible_count} profiles satisfy dy > 0 and dz > 150 across "
          f"{len(feasible_ns)} num_ticks values ({feasible_ns[0]}..{feasible_ns[-1]})")
    print(f"interactive depth 1..{max_depth}; reference n={reference['n']}, "
          f"lambda={reference['lambda']:.2f}")
    for path in paths:
        print(f"wrote {path}")


if __name__ == "__main__":
    main()
