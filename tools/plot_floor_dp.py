#!/usr/bin/env python3
"""Build the backward-DP explorer (README-floor 6g): the DP's flown policy tick by tick, against
its gradient polish and the best earlier answer, with the value it compared at every pitch and
its policy over velocity at the probed state's exact height. Made to look for chatter.

The policy map is computed in the page (tools/floor_dp_policy.js, a port of floordp's backup)
from a crop of the saved V; the build checks the port against every traced DP tick.

Inputs: runs/floor/v16-dp/viz, written from the saved V by, for each mode (time, dist),

    floordp --mode <mode> --hmax 33 --nh 133 --gamma 1.5 --dv 0.025 \
        --load runs/floor/v16-dp/thresh/V_<mode>.bin --trace --out runs/floor/v16-dp/viz/<mode> \
        --y0 <Y0S> > runs/floor/v16-dp/viz/<mode>.txt

the V files themselves, the polishes in runs/floor/v16-dp/pol and thresh/pol, and
known-before-dp.tsv.
"""

import base64
import csv
import io
import json
import math
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plot_floor_profiles import TRACE, clipped, roughness, run_trace  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
DP = ROOT / "runs/floor/v16-dp"
VIZ = DP / "viz"
OUT = ROOT / "runs/floor/fig/floor-dp.html"
# Ticks shown per flight: flights that never land run 20000 ticks in the files.
TMAX = 3000
# The `--dp` grid the `.q` files are on, and the map's crop (the DP flights stay inside
# v_y -0.53..0.92, v_z 0..1.76).
QPITCH = np.arange(-85, 86)
MAP_VY, MAP_VZ = (-0.8, 1.1), (0.0, 2.0)
# The saved V's grid (its `.spec`), and the crop of it the page holds: every read from the map's
# cells lands inside (a tick moves v_y by -0.1..+0.3 and v_z by -0.3..+0.1).
GRID = {"hmax": 33.0, "gamma": 1.5, "nh": 133, "vymin": -3.0, "vymax": 2.0, "vzmax": 3.0, "dv": 0.025, "dp": 1}
V_VY, V_VZ = (-1.2, 1.6), (0.0, 2.4)
# The policy's pitch grid (floordp `--fine`).
FINE = 0.25
# Deficits and margins are coded on a log scale, CODE_PER_DECADE codes per decade from 10^LOG_LO.
LOG_LO, CODE_PER_DECADE = -6.0, 32.0
MODES = {"endurance": "time", "range": "dist"}
# The `--y0` list the viz files were made with.
Y0S = "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,27.5,27.75,28,28.25,28.5,28.75,29,29.25,29.5,30,31,32"


def log_code(x):
    x = np.maximum(np.asarray(x, dtype=np.float64), 10 ** LOG_LO)
    return np.clip(np.round((np.log10(x) - LOG_LO) * CODE_PER_DECADE), 0, 255).astype(np.uint8)


def png(a):
    buf = io.BytesIO()
    Image.fromarray(a, mode="L").save(buf, format="PNG", optimize=True)
    return base64.b64encode(buf.getvalue()).decode()


def b64(a):
    return base64.b64encode(np.ascontiguousarray(a).tobytes()).decode()


def y0_name(y):
    return f"{y:g}"


def replay(path, y0):
    """A pitch file's exact replay (floor_trace), clipped at its exit, as columns."""
    rows = run_trace(str(path))
    states, t_exit, z_exit = clipped(rows, y0)
    live = [s["pitch"] for s in states if s["pitch"] is not None]
    rough, chatter = roughness(live)
    states = states[:TMAX + 1]
    return {"t_exit": t_exit, "z_exit": z_exit, "chatter": chatter, "rough": round(rough, 1),
            "pitch": [None if s["pitch"] is None else round(s["pitch"], 3) for s in states],
            # Full precision: the page recomputes the DP's policy at these states.
            "h": [s["y"] + y0 for s in states], "vy": [s["vy"] for s in states],
            "vz": [s["vz"] for s in states]}


def dp_flight(mode, y0):
    base = VIZ / mode / f"{mode}_y{y0_name(y0)}"
    rows = list(csv.DictReader(open(f"{base}.trace")))
    col = {k: np.array([float(r[k]) for r in rows]) for k in rows[0]}
    head = open(f"{base}.pitches").read().splitlines()[1].split()
    t_exit, z_exit = float(head[head.index("t*") + 1]), float(head[head.index("z(t*)") + 1])
    rough, chatter = roughness(list(col["pitch"]))
    n = min(len(rows), TMAX)
    q = np.fromfile(f"{base}.q", dtype="<f4").reshape(-1, len(QPITCH))[:n]
    # Deficit of every pitch against the one flown (the best on the finer 0.25-degree grid);
    # image rows run pitch -85 (top) to +85, columns are ticks.
    deficit = np.maximum(col["q"][:n, None] - q, 0.0)
    return {"t_exit": None if np.isnan(t_exit) else t_exit, "z_exit": None if np.isnan(z_exit) else z_exit,
            "chatter": chatter, "rough": round(rough, 1), "ticks": len(rows),
            "pitch": [round(x, 2) for x in col["pitch"][:n]],
            "h": [float(x) for x in col["h"][:n]],
            "vy": [float(x) for x in col["vy"][:n]], "vz": [float(x) for x in col["vz"][:n]],
            "qflown": [float(x) for x in col["q"][:n]],
            "alt": [None if np.isnan(x) else round(x, 2) for x in col["alt_pitch"][:n]],
            "margin": [None if not np.isfinite(x) else float(f"{x:.3g}") for x in col["margin"][:n]],
            "q": png(log_code(deficit).T.copy())}


def known():
    out = {}
    for line in open(DP / "known-before-dp.tsv"):
        mode, y0, _, path = line.split("\t")
        out[(mode, float(y0))] = DP.parent / path.strip()
    return out


def polished(mode, y0):
    tag = {"time": "t", "dist": "d"}[mode]
    for path in (DP / f"pol/{tag}_hi_v025_{mode}_y{y0_name(y0)}.pitches",
                 DP / f"thresh/pol/{mode}_y{y0_name(y0)}.pitches"):
        if path.exists():
            return path
    return None


def pitch_trig():
    """`PitchTrig::new(p as f32)` for every policy pitch under mth_lut: f32 lean, double cos
    squared, and Mth.sin/cos from the 65536-entry f32 table."""
    table = np.array([math.sin(i * math.pi * 2 / 65536) for i in range(65536)], dtype=np.float32)
    lut = lambda x: float(table[int(x) & 0xFFFF])  # noqa: E731 -- int() truncates, as Java's cast
    pitches = [min(-85 + FINE * i, 85.0) for i in range(round(170 / FINE) + 1)]
    trig = []
    for p in pitches:
        lean = np.float32(p) * np.float32(math.pi / 180)
        x = np.float32(lean * np.float32(10430.378))
        trig.append([math.cos(float(lean)) ** 2, lut(x), lut(np.float32(x + np.float32(16384.0)))])
    return pitches, trig


def policy_grid(mode):
    """What makePolicy needs: the grid (as floordp builds it), a crop of the saved V, the trig."""
    g, dv = GRID, GRID["dv"]
    jy0 = round(-g["vymin"] / dv)
    nvy, nvz = jy0 + round(g["vymax"] / dv) + 1, round(g["vzmax"] / dv) + 1
    vy0 = -float(jy0) * dv
    spec = (f"{mode} grid {g['nh']}x{nvy}x{nvz} hmax {g['hmax']:g} gamma {g['gamma']:g} vy {vy0:g} "
            f"dv {dv:g} dp {g['dp']:g}")
    f = DP / f"thresh/V_{mode}.bin"
    saved = open(f"{f}.spec").read().strip()
    if saved != spec:
        raise RuntimeError(f"{f}: spec {saved!r}, expected {spec!r}")
    V = np.fromfile(f, dtype="<f4").reshape(g["nh"], nvy, nvz)
    j0, j1 = round((V_VY[0] - vy0) / dv), round((V_VY[1] - vy0) / dv)
    k0, k1 = round(V_VZ[0] / dv), round(V_VZ[1] / dv)
    crop = V[:, j0:j1 + 1, k0:k1 + 1]
    hs = [g["hmax"] * (i / (g["nh"] - 1)) ** g["gamma"] for i in range(g["nh"])]
    pitches, trig = pitch_trig()
    return {"mode": mode, "hs": hs, "hmax": g["hmax"], "vy0": vy0, "dv": dv, "nvy": nvy, "nvz": nvz,
            "j0": j0, "k0": k0, "cnvy": crop.shape[1], "cnvz": crop.shape[2], "V": b64(crop),
            "pitches": pitches, "trig": trig,
            # The map's cells: full-grid node indices.
            "map": {"j": [round((MAP_VY[0] - vy0) / dv), round((MAP_VY[1] - vy0) / dv)],
                    "k": [round(MAP_VZ[0] / dv), round(MAP_VZ[1] / dv)]}}


CHECK = r"""
const {makePolicy} = require(process.argv[1]);
const D = JSON.parse(require("fs").readFileSync(0, "utf8"));
let n = 0, bad = 0, outside = 0, worst = 0;
for (const [key, G] of Object.entries(D.grids)) {
  const b = Buffer.from(G.V, "base64");
  G.V = new Float32Array(b.buffer, b.byteOffset, b.length / 4);
  const P = makePolicy(G);
  for (const F of D.flights[key]) {
    const d = F.dp;
    for (let t = 0; t < d.pitch.length; t++) {
      const r = P.ranked(d.h[t], d.vy[t], d.vz[t]);
      if (!r.ok) { outside++; continue; }
      n++;
      worst = Math.max(worst, Math.abs(r.q - d.qflown[t]));
      if (r.pitch !== d.pitch[t]) { if (bad++ < 5) console.error(`${key} y0 ${F.y0} t ${t}: page ${r.pitch}, floordp ${d.pitch[t]}`); }
    }
  }
}
console.log(JSON.stringify({n, bad, outside, worst}));
"""


def check_port(data):
    """Recompute every traced DP tick with the page's policy; it must pick floordp's pitch."""
    out = subprocess.run(["node", "-e", CHECK, "--", str(Path(__file__).resolve().parent / "floor_dp_policy.js")],
                         input=json.dumps({"grids": data["grids"], "flights": data["flights"]}),
                         capture_output=True, text=True, check=True)
    print(out.stderr, end="")
    r = json.loads(out.stdout)
    print(f"policy port: {r['n']} DP ticks recomputed, {r['bad']} different choices, {r['outside']} read V "
          f"outside the crop, largest value difference {r['worst']:.2e}")
    if r["bad"]:
        raise RuntimeError("the page's policy disagrees with floordp")


def load_data():
    if not TRACE.exists():
        raise FileNotFoundError(f"{TRACE}: cargo build --release --bin floor_trace")
    best = known()
    y0s = sorted({float(p.name.split("_y")[1][:-len(".trace")]) for p in (VIZ / "time").glob("*.trace")})
    flights = {}
    for key, mode in MODES.items():
        flights[key] = []
        for y0 in y0s:
            pol = polished(mode, y0)
            prev = best.get((mode, y0))
            flights[key].append({"y0": y0, "dp": dp_flight(mode, y0),
                                 "pol": replay(pol, y0) if pol else None,
                                 "best": replay(prev, y0) if prev and prev.exists() else None})
        print(f"{key}: {len(y0s)} heights")
    return {"y0s": y0s, "flights": flights, "grids": {key: policy_grid(mode) for key, mode in MODES.items()},
            "tmax": TMAX, "qpitch": [int(QPITCH[0]), int(QPITCH[-1])],
            "code": {"lo": LOG_LO, "perDecade": CODE_PER_DECADE}}


HTML = (Path(__file__).resolve().parent / "floor_dp.html").read_text


def main():
    loaded = load_data()
    check_port(loaded)
    data = json.dumps(loaded, separators=(",", ":"), allow_nan=False)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    policy = (Path(__file__).resolve().parent / "floor_dp_policy.js").read_text()
    OUT.write_text(HTML().replace("__TMAX__", str(TMAX)).replace("__POLICY__", policy).replace("__DATA__", data))
    print(f"{OUT}  {OUT.stat().st_size / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
