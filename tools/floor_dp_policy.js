// The DP's policy at any state, as `floordp` computes it: one flight tick per candidate pitch
// (`update_fall_flying_movement_reference_cached`, yaw zero, so v_x stays 0), then
// `r + V(s')` read from V by `Grid::at`'s trilinear interpolation, or the exit's fraction of r.
// The operations are in floordp's order so the doubles come out the same; the pitch terms come
// precomputed (`PitchTrig`, mth_lut) from plot_floor_dp.py. Shared by floor_dp.html and the
// check in plot_floor_dp.py. Written without per-call allocation: a field is ~4M backups.
function makePolicy(G) {
  // G: {mode: "time"|"dist", hs: [nh], hmax, vy0, dv, nvy, nvz (full grid), j0, k0, cnvy, cnvz
  //     (the crop of V held), V: Float32Array (crop, h-major then v_y then v_z),
  //     pitches: [n], trig: [[cos_sq, sin, cos], ...]}
  const GRAVITY = 0.08, DRAG_Y = Math.fround(0.98), DRAG_Z = Math.fround(0.99);
  const nh = G.hs.length, hs = Float64Array.from(G.hs), V = G.V, time = G.mode === "time";
  const n = G.pitches.length, pitches = Float64Array.from(G.pitches);
  const CS = Float64Array.from(G.trig, t => t[0]), SN = Float64Array.from(G.trig, t => t[1]), CO = Float64Array.from(G.trig, t => t[2]);
  const hmax = G.hmax, vy0 = G.vy0, dv = G.dv, nvy = G.nvy, nvz = G.nvz, j0 = G.j0, k0 = G.k0, cnvy = G.cnvy, cnvz = G.cnvz;
  // V at (h, v_y, v_z), or NaN where the read leaves the crop of V this page holds.
  function at(h, vy, vz) {
    const hc = Math.min(Math.max(h, 0), hmax);
    let lo = 0, hi = nh - 2;  // largest i <= nh - 2 with hs[i] <= hc
    while (lo < hi) { const m = (lo + hi + 1) >> 1; if (hs[m] <= hc) lo = m; else hi = m - 1; }
    const i = lo, a = Math.min(Math.max((hc - hs[i]) / (hs[i + 1] - hs[i]), 0), 1);
    // Grid::at's split(): clamp to the grid, then the cell and the fraction in it.
    let x = Math.min(Math.max((vy - vy0) / dv, 0), nvy - 1);
    const jf = Math.min(Math.floor(x), Math.max(nvy - 2, 0)), b = x - jf;
    x = Math.min(Math.max(vz / dv, 0), nvz - 1);
    const kf = Math.min(Math.floor(x), Math.max(nvz - 2, 0)), c = x - kf;
    const j = jf - j0, k = kf - k0;
    if (j < 0 || k < 0 || j + 1 >= cnvy || k + 1 >= cnvz) return NaN;
    const p0 = (i * cnvy + j) * cnvz + k, p1 = p0 + cnvy * cnvz, dj = cnvz;
    const l00 = V[p0] * (1 - c) + V[p0 + 1] * c;
    const l01 = V[p0 + dj] * (1 - c) + V[p0 + dj + 1] * c;
    const l10 = V[p1] * (1 - c) + V[p1 + 1] * c;
    const l11 = V[p1 + dj] * (1 - c) + V[p1 + dj + 1] * c;
    const l0 = l00 * (1 - b) + l01 * b, l1 = l10 * (1 - b) + l11 * b;
    return l0 * (1 - a) + l1 * a;
  }
  function backup(h, vy, vz, t) {
    const cs = CS[t], s = SN[t], co = CO[t];
    const lh = Math.abs(co), mh = Math.sqrt(0 * 0 + vz * vz);
    vy += GRAVITY * (-1 + cs * 0.75);
    if (vy < 0 && lh > 0) { const cv = vy * -0.1 * cs; vy += cv; vz += co * cv / lh; }
    if (s < 0 && lh > 0) { const cv = mh * -s * 0.04; vy += cv * 3.2; vz += -co * cv / lh; }
    if (lh > 0) vz += (co / lh * mh - vz) * 0.1;
    vy *= DRAG_Y; vz *= DRAG_Z;
    const nh2 = h + vy, r = time ? 1 : vz;
    if (nh2 < 0) return h / (h - nh2) * r;
    return r + at(nh2, vy, vz);
  }
  const q = new Float64Array(n);
  // The choice (first best, as floordp's fold), its value, and the best pitch more than `gap`
  // degrees from it. `ok` is false if any candidate read V outside the crop. The result object
  // is reused: copy what you keep.
  const out = {ok: true, pitch: 0, q: 0, alt: null, margin: Infinity};
  function ranked(h, vy, vz, gap = 10) {
    let best = 0, ok = true;
    for (let t = 0; t < n; t++) {
      const v = backup(h, vy, vz, t);
      q[t] = v;
      if (v !== v) ok = false;
      if (v > q[best]) best = t;
    }
    let alt = -1;
    const pb = pitches[best];
    for (let t = 0; t < n; t++)
      if (Math.abs(pitches[t] - pb) > gap && (alt < 0 || q[t] > q[alt])) alt = t;
    out.ok = ok; out.pitch = pb; out.q = q[best];
    out.alt = alt < 0 ? null : pitches[alt]; out.margin = alt < 0 ? Infinity : q[best] - q[alt];
    return out;
  }
  return {at, backup, ranked};
}
if (typeof module !== "undefined") module.exports = {makePolicy};
