//! The one-tick energy field G(v) = max over pitch of g(v, pitch), its exact local geometry on a
//! grid, and its troughs, ridges and creases as curves. The heavy half of
//! `tools/plot_field_troughs.py` and `tools/plot_field_gradient.py`, which load what this writes;
//! README-field.md has the results and the method.
//!
//! g is total energy in blocks after one tick from v = (v_y, v_z) at yaw zero. The kernel here is
//! the f64 `libm` transcription in `tools/field_geometry.py`, *not* the crate's
//! `update_fall_flying_movement`: that takes an f32 pitch and Minecraft's sine table, which makes
//! g(pitch) a staircase of 0.0055° steps, and golden-section refinement and the envelope-theorem
//! derivatives both need it smooth. `tools/field_geometry.py` is the same math, kept for one-off
//! probes; `tools/field_data.py --check` compares the two.
//!
//! Usage: `field --out DIR [--window vz_lo,vz_hi,vy_lo,vy_hi] [--samples N|Nz,Ny] [--pad F]
//! [--no-curves]`; `field --help` has the details.
//!
//! Writes into DIR, as .npy: `vz`, `vy` (axes, blocks/tick); per sample, indexed [vy][vz]:
//! `p1` best pitch, `G`, `p2`/`g2` runner-up local max in pitch (nan/-inf if none), `stuck`
//! (u8: the max is on a corner or bound, so p* does not move with v), `gy` `gz` `hyy` `hyz`
//! `hzz` (grad G and its Hessian, per blocks/tick). And `curves`: rows of
//! (curve id, kind, class, vz, vy), kind 0 tie / 1 conversion / 2 vz=0 / 3 smooth, class +1
//! trough / -1 ridge / 0 kink with no extremum. `spec.txt` records the arguments.

use std::collections::HashMap;
use std::io::Write;

use clap::Parser;
use clap_derive::Parser;
use rayon::prelude::*;

const GRAVITY: f64 = 0.08;
const DRAG_Y: f64 = 0.98_f32 as f64;
const DRAG_Z: f64 = 0.99_f32 as f64;
const PITCH_LIMIT: f64 = 89.0;
const TPS: f64 = 20.0;
// field_geometry's constants; see there for why each is what it is.
const GOLD: f64 = 0.618_033_988_749_894_9;
const H1: f64 = 1e-6;
const H2: f64 = 1e-4;
const HP: f64 = 1e-3;
const KINK: f64 = 1e-5;
const SNAP: f64 = 1e-4;
// Degrees between neighboring samples' best pitches that count as the argmax jumping: the
// `jump` mask, which keeps the smooth-extremum search off ties, and the tie search's main test.
const JUMP: f64 = 3.0;
// Degrees: a smaller pitch change across an edge can still hide a tie, if conversion also
// differs across it. See crease_curves.
const TIE_EDGE: f64 = 0.05;

// ---- the kernel -----------------------------------------------------------------------------

fn g(vy: f64, vz: f64, pitch: f64) -> f64 {
    let lean = pitch.to_radians();
    let lift = lean.cos() * lean.cos();
    let move_hor = vz.abs();
    let mut y = vy + GRAVITY * (-1.0 + lift * 0.75);
    let mut z = vz;
    let conv = if y < 0.0 { y * -0.1 * lift } else { 0.0 }; // descent -> forward
    y += conv;
    z += conv; // look_z / look_hor is 1 over |pitch| < 90
    let up = if lean < 0.0 { move_hor * -lean.sin() * 0.04 } else { 0.0 }; // forward -> up
    y += up * 3.2;
    z -= up;
    z += (move_hor - z) * 0.1;
    let (y1, z1) = (y * DRAG_Y, z * DRAG_Z);
    (y1 * y1 + z1 * z1 - vy * vy - vz * vz) * 0.5 / GRAVITY + y1
}

fn golden_max(f: impl Fn(f64) -> f64, mut a: f64, mut b: f64) -> f64 {
    for _ in 0..48 {
        let (c, d) = (b - GOLD * (b - a), a + GOLD * (b - a));
        if f(c) > f(d) { b = d } else { a = c }
    }
    let mut p = (a + b) / 2.0;
    for e in [a, b] {
        if f(e) > f(p) { p = e }
    }
    // Golden section stalls microdegrees short of the level corner, where g is flat to second
    // order nose-down; a max that close to a whole-degree corner, tied to rounding, is it.
    for corner in [0.0, -PITCH_LIMIT, PITCH_LIMIT] {
        if (p - corner).abs() < SNAP && f(corner) >= f(p) - 1e-12 { p = corner }
    }
    p
}

/// |pitch| at which v_y after gravity is 0: the convex kink between two humps of g(pitch).
fn conversion_kink(vy: f64) -> Option<f64> {
    let c = (GRAVITY - vy) / (0.75 * GRAVITY);
    (c > 0.0 && c <= 1.0).then(|| c.sqrt().acos().to_degrees())
}

/// [a, b] around hint, within the clamp and not across a conversion kink.
fn bracket(vy: f64, hint: f64, width: f64) -> (f64, f64) {
    let mut a = (hint - width).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    let mut b = (hint + width).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    if let Some(k) = conversion_kink(vy) {
        for wall in [k, -k] {
            if wall > hint && wall < b { b = wall }
            if wall < hint && wall > a { a = wall }
        }
    }
    (a, b)
}

fn local_max(vy: f64, vz: f64, hint: f64, width: f64) -> (f64, f64) {
    let (a, b) = bracket(vy, hint, width);
    let p = golden_max(|q| g(vy, vz, q), a, b);
    (p, g(vy, vz, p))
}

fn is_local_max(vy: f64, vz: f64, p: f64) -> bool {
    let f0 = g(vy, vz, p);
    let lo = p <= -PITCH_LIMIT || g(vy, vz, (p - 1e-3).max(-PITCH_LIMIT)) <= f0;
    let hi = p >= PITCH_LIMIT || g(vy, vz, (p + 1e-3).min(PITCH_LIMIT)) <= f0;
    lo && hi
}

/// (p1, G, p2, g2): the best pitch and the field, and the runner-up local max in pitch.
fn branches(vy: f64, vz: f64) -> (f64, f64, f64, f64) {
    let n = (2.0 * PITCH_LIMIT) as usize + 1;
    let s: Vec<f64> = (0..n).map(|i| g(vy, vz, i as f64 - PITCH_LIMIT)).collect();
    let (mut i1, mut i2): (Option<usize>, Option<usize>) = (None, None);
    for i in 0..n {
        let up = i == 0 || s[i] >= s[i - 1];
        let down = i == n - 1 || s[i] > s[i + 1];
        if !(up && down) { continue }
        if i1.is_none_or(|j| s[i] > s[j]) {
            i2 = i1;
            i1 = Some(i);
        } else if i2.is_none_or(|j| s[i] > s[j]) {
            i2 = Some(i);
        }
    }
    let refine = |i: usize| {
        let p0 = i as f64 - PITCH_LIMIT;
        let p = golden_max(|q| g(vy, vz, q), (p0 - 1.0).max(-PITCH_LIMIT), (p0 + 1.0).min(PITCH_LIMIT));
        (p, g(vy, vz, p))
    };
    let (p1, g1) = refine(i1.expect("g(pitch) has a max"));
    match i2 {
        None => (p1, g1, f64::NAN, f64::NEG_INFINITY),
        Some(i) => {
            let (p2, g2) = refine(i);
            if g2 > g1 { (p2, g2, p1, g1) } else { (p1, g1, p2, g2) }
        }
    }
}

fn stuck(vy: f64, vz: f64, p: f64) -> bool {
    let gl = (g(vy, vz, p) - g(vy, vz, p - HP)) / HP;
    let gr = (g(vy, vz, p + HP) - g(vy, vz, p)) / HP;
    p.abs() >= PITCH_LIMIT - 1e-9 || gl - gr > KINK
}

/// grad G and its Hessian at the max p, by the envelope theorem: (gy, gz, hyy, hyz, hzz).
fn derivatives(vy: f64, vz: f64, p: f64, is_stuck: bool) -> [f64; 5] {
    let f = |dy: f64, dz: f64, dp: f64| g(vy + dy, vz + dz, p + dp);
    let gy = (f(H1, 0.0, 0.0) - f(-H1, 0.0, 0.0)) / (2.0 * H1);
    let gz = (f(0.0, H1, 0.0) - f(0.0, -H1, 0.0)) / (2.0 * H1);
    let f0 = f(0.0, 0.0, 0.0);
    let mut hyy = (f(H2, 0.0, 0.0) - 2.0 * f0 + f(-H2, 0.0, 0.0)) / (H2 * H2);
    let mut hzz = (f(0.0, H2, 0.0) - 2.0 * f0 + f(0.0, -H2, 0.0)) / (H2 * H2);
    let mut hyz = (f(H2, H2, 0.0) - f(H2, -H2, 0.0) - f(-H2, H2, 0.0) + f(-H2, -H2, 0.0)) / (4.0 * H2 * H2);
    let gyp = (f(H2, 0.0, HP) - f(H2, 0.0, -HP) - f(-H2, 0.0, HP) + f(-H2, 0.0, -HP)) / (4.0 * H2 * HP);
    let gzp = (f(0.0, H2, HP) - f(0.0, H2, -HP) - f(0.0, -H2, HP) + f(0.0, -H2, -HP)) / (4.0 * H2 * HP);
    let gpp = (f(0.0, 0.0, HP) - 2.0 * f0 + f(0.0, 0.0, -HP)) / (HP * HP);
    if !is_stuck && gpp < 0.0 {
        hyy -= gyp * gyp / gpp;
        hyz -= gyp * gzp / gpp;
        hzz -= gzp * gzp / gpp;
    }
    [gy, gz, hyy, hyz, hzz]
}

fn converts(vy: f64, p: f64) -> bool {
    vy + GRAVITY * (-1.0 + 0.75 * p.to_radians().cos().powi(2)) < 0.0
}

// ---- the grid -------------------------------------------------------------------------------

struct Field {
    vz: Vec<f64>,
    vy: Vec<f64>,
    hz: f64,
    hy: f64,
    p1: Vec<f64>,
    big_g: Vec<f64>,
    p2: Vec<f64>,
    g2: Vec<f64>,
    stuck: Vec<bool>,
    d: Vec<[f64; 5]>,
    jump: Vec<bool>,
}

impl Field {
    fn new(vz: Vec<f64>, vy: Vec<f64>) -> Self {
        let (nz, ny) = (vz.len(), vy.len());
        let cells: Vec<(f64, f64, f64, f64, bool, [f64; 5])> = (0..ny * nz)
            .into_par_iter()
            .map(|k| {
                let (y, z) = (vy[k / nz], vz[k % nz]);
                let (p1, big_g, p2, g2) = branches(y, z);
                let st = stuck(y, z, p1);
                (p1, big_g, p2, g2, st, derivatives(y, z, p1, st))
            })
            .collect();
        let p1: Vec<f64> = cells.iter().map(|c| c.0).collect();
        // Where the argmax jumps to a neighbor: a tie runs through here.
        let mut jump = vec![false; ny * nz];
        for i in 0..ny {
            for j in 0..nz {
                let k = i * nz + j;
                if j + 1 < nz && (p1[k] - p1[k + 1]).abs() > JUMP { jump[k] = true; jump[k + 1] = true }
                if i + 1 < ny && (p1[k] - p1[k + nz]).abs() > JUMP { jump[k] = true; jump[k + nz] = true }
            }
        }
        Field {
            hz: vz[1] - vz[0],
            hy: vy[1] - vy[0],
            big_g: cells.iter().map(|c| c.1).collect(),
            p2: cells.iter().map(|c| c.2).collect(),
            g2: cells.iter().map(|c| c.3).collect(),
            stuck: cells.iter().map(|c| c.4).collect(),
            d: cells.iter().map(|c| c.5).collect(),
            p1,
            jump,
            vz,
            vy,
        }
    }

    fn nz(&self) -> usize { self.vz.len() }
    fn ny(&self) -> usize { self.vy.len() }

    /// Index of the sample nearest (vz, vy).
    fn nearest(&self, q: (f64, f64)) -> usize {
        let c = (((q.0 - self.vz[0]) / self.hz).round().max(0.0) as usize).min(self.nz() - 1);
        let r = (((q.1 - self.vy[0]) / self.hy).round().max(0.0) as usize).min(self.ny() - 1);
        r * self.nz() + c
    }

    /// The edges: (axis, i, j) -> sample indices of its two ends. axis 0 runs along vz.
    fn edges(&self) -> Vec<(u8, usize, usize, usize, usize)> {
        let (ny, nz) = (self.ny(), self.nz());
        let mut out = Vec::with_capacity(2 * ny * nz);
        for i in 0..ny {
            for j in 0..nz {
                if j + 1 < nz { out.push((0, i, j, i * nz + j, i * nz + j + 1)) }
                if i + 1 < ny { out.push((1, i, j, i * nz + j, (i + 1) * nz + j)) }
            }
        }
        out
    }

    fn pos(&self, k: usize) -> (f64, f64) { (self.vz[k % self.nz()], self.vy[k / self.nz()]) }
}

// ---- curves ---------------------------------------------------------------------------------

/// A located curve point: position, the branch pitches to follow on either side, and a normal.
#[derive(Clone, Copy)]
struct Pt { q: (f64, f64), pa: f64, pb: f64, n: (f64, f64) }

fn sign(x: f64) -> i8 { if x > 0.0 { 1 } else if x < 0.0 { -1 } else { 0 } }

/// The root of phi on the segment a -> b; None if phi has the same sign at both ends.
fn bisect_segment(phi: impl Fn((f64, f64)) -> f64, a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let at = |t: f64| (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
    let (mut lo, mut hi) = (0.0, 1.0);
    let mut flo = phi(at(lo));
    if sign(flo) == sign(phi(at(hi))) { return None }
    for _ in 0..55 {
        let mid = (lo + hi) / 2.0;
        let fm = phi(at(mid));
        if sign(fm) == sign(flo) { lo = mid; flo = fm } else { hi = mid }
    }
    Some(at((lo + hi) / 2.0))
}

/// G near q as the best of the maxima found by following each hint.
fn g_local(q: (f64, f64), hints: &[f64]) -> f64 {
    hints.iter().filter(|h| h.is_finite())
        .map(|&h| local_max(q.1, q.0, h, 2.0).1)
        .fold(f64::NEG_INFINITY, f64::max)
}

/// +1 trough, -1 ridge, 0 kink with no extremum: from G's slope just either side along n.
fn classify(p: &Pt) -> i8 {
    let delta = 1e-5;
    let hints = [p.pa, p.pb];
    let g0 = g_local(p.q, &hints);
    let fwd = (g_local((p.q.0 + delta * p.n.0, p.q.1 + delta * p.n.1), &hints) - g0) / delta;
    let bwd = (g0 - g_local((p.q.0 - delta * p.n.0, p.q.1 - delta * p.n.1), &hints)) / delta;
    if fwd > 0.0 && bwd < 0.0 { 1 } else if fwd < 0.0 && bwd > 0.0 { -1 } else { 0 }
}

/// Link points that share a grid cell, and walk the links into ordered chains.
fn chain_cells(keys: &[(u8, usize, usize)], points: &HashMap<(u8, usize, usize), Pt>) -> Vec<Vec<(u8, usize, usize)>> {
    let mut cells: HashMap<(isize, isize), Vec<(u8, usize, usize)>> = HashMap::new();
    for &key in keys {
        let (axis, i, j) = (key.0, key.1 as isize, key.2 as isize);
        // An edge along vz borders the cells below and above it; one along vy, left and right.
        let both = if axis == 0 { [(i - 1, j), (i, j)] } else { [(i, j - 1), (i, j)] };
        for cell in both { cells.entry(cell).or_default().push(key) }
    }
    let mut links: HashMap<(u8, usize, usize), Vec<(u8, usize, usize)>> = keys.iter().map(|&k| (k, vec![])).collect();
    let link = |a, b, links: &mut HashMap<_, Vec<_>>| {
        links.get_mut(&a).unwrap().push(b);
        links.get_mut(&b).unwrap().push(a);
    };
    let mut cell_keys: Vec<_> = cells.keys().copied().collect();
    cell_keys.sort();
    for cell in cell_keys {
        let m = &cells[&cell];
        match m.len() {
            2 => link(m[0], m[1], &mut links),
            4 => {
                // A saddle cell: pair the closest.
                let d = |x: usize, y: usize| {
                    let (a, b) = (points[&m[x]].q, points[&m[y]].q);
                    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
                };
                let pairing = [(0, 1, 2, 3), (0, 2, 1, 3), (0, 3, 1, 2)].into_iter()
                    .min_by(|p, q| (d(p.0, p.1) + d(p.2, p.3)).total_cmp(&(d(q.0, q.1) + d(q.2, q.3))))
                    .unwrap();
                link(m[pairing.0], m[pairing.1], &mut links);
                link(m[pairing.2], m[pairing.3], &mut links);
            }
            _ => {}
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut chains = vec![];
    let starts: Vec<_> = keys.iter().filter(|k| links[k].len() == 1).chain(keys.iter()).copied().collect();
    for s in starts {
        if seen.contains(&s) { continue }
        let (mut chain, mut cur, mut prev) = (vec![], Some(s), None);
        while let Some(c) = cur {
            if !seen.insert(c) { break }
            chain.push(c);
            let next = links[&c].iter().copied().find(|k| Some(*k) != prev && !seen.contains(k));
            prev = Some(c);
            cur = next;
        }
        chains.push(chain);
    }
    chains
}

/// Runs of one class, at least min_len long: [(points, class)].
fn split_runs(pts: Vec<Pt>, classes: Vec<i8>, min_len: usize) -> Vec<(Vec<Pt>, i8)> {
    let mut out = vec![];
    let mut start = 0;
    for k in 1..=pts.len() {
        if k == pts.len() || classes[k] != classes[start] {
            if k - start >= min_len { out.push((pts[start..k].to_vec(), classes[start])) }
            start = k;
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind { Tie = 0, Conversion = 1, VzZero = 2, Smooth = 3 }

/// Every crease of G, located exactly on the grid edges where the regime changes.
fn crease_curves(f: &Field) -> Vec<(Kind, i8, Vec<Pt>)> {
    let conv: Vec<bool> = (0..f.p1.len()).map(|k| converts(f.vy[k / f.nz()], f.p1[k])).collect();
    let neg: Vec<bool> = (0..f.p1.len()).map(|k| f.vz[k % f.nz()] < 0.0).collect();
    let mut tasks = vec![];
    for (axis, i, j, a, b) in f.edges() {
        let dp = (f.p1[a] - f.p1[b]).abs();
        // A tie is two humps of g(pitch). Two humps closer than JUMP need a convex kink between
        // them, and the only convex kink is the conversion one, so a small pitch change hides a
        // tie only where conversion also differs across the edge. That is the case where a
        // branch is born (vz ~ 16 b/s), where the jump shrinks to nothing. Fake candidates,
        // where p* only moves fast, fail the genuineness test below.
        if dp > JUMP || (dp > TIE_EDGE && conv[a] != conv[b]) { tasks.push((Kind::Tie, axis, i, j, a, b)) }
        if conv[a] != conv[b] { tasks.push((Kind::Conversion, axis, i, j, a, b)) }
        if neg[a] != neg[b] { tasks.push((Kind::VzZero, axis, i, j, a, b)) }
    }
    let found: Vec<(Kind, (u8, usize, usize), Pt)> = tasks.par_iter().filter_map(|&(kind, axis, i, j, a, b)| {
        let (pa_, pb_) = (f.p1[a], f.p1[b]);
        let width = ((pa_ - pb_).abs() / 2.0 - 0.5).clamp(2.0, 10.0);
        let q = match kind {
            Kind::Tie => bisect_segment(|q| local_max(q.1, q.0, pa_, width).1 - local_max(q.1, q.0, pb_, width).1, f.pos(a), f.pos(b)),
            Kind::Conversion => bisect_segment(|q| {
                let p = local_max(q.1, q.0, pa_, 2.0).0;
                q.1 + GRAVITY * (-1.0 + 0.75 * p.to_radians().cos().powi(2))
            }, f.pos(a), f.pos(b)),
            _ => bisect_segment(|q| q.0, f.pos(a), f.pos(b)),
        }?;
        let best = branches(q.1, q.0).1;
        let (pa, pb) = if kind == Kind::Tie {
            // Both maxima genuine, distinct, and together on top of G.
            let (ra, va) = local_max(q.1, q.0, pa_, width);
            let (rb, _) = local_max(q.1, q.0, pb_, width);
            let ok = is_local_max(q.1, q.0, ra) && is_local_max(q.1, q.0, rb)
                && (ra - rb).abs() > 0.01 && va >= best - 1e-9;
            if !ok { return None }
            (ra, rb)
        } else {
            let (r, v) = local_max(q.1, q.0, pa_, 2.0);
            if v < best - 1e-9 { return None }
            (r, r)
        };
        // The crease normal is the jump in grad G across it.
        let n = (f.d[a][1] - f.d[b][1], f.d[a][0] - f.d[b][0]);
        let len = (n.0 * n.0 + n.1 * n.1).sqrt().max(1e-300);
        Some((kind, (axis, i, j), Pt { q, pa, pb, n: (n.0 / len, n.1 / len) }))
    }).collect();

    let mut out = vec![];
    for kind in [Kind::Tie, Kind::Conversion, Kind::VzZero] {
        let points: HashMap<_, _> = found.iter().filter(|x| x.0 == kind).map(|x| (x.1, x.2)).collect();
        let mut keys: Vec<_> = points.keys().copied().collect();
        keys.sort();
        let chains = chain_cells(&keys, &points);
        let classified: Vec<(Vec<Pt>, Vec<i8>)> = chains.par_iter().map(|ch| {
            let pts: Vec<Pt> = ch.iter().map(|k| points[k]).collect();
            let cls = pts.iter().map(classify).collect();
            (pts, cls)
        }).collect();
        for (pts, cls) in classified {
            for (run, c) in split_runs(pts, cls, 2) { out.push((kind, c, run)) }
        }
    }
    out
}

/// Smooth valleys (kind +1) or ridges (-1): det[grad G, H grad G] = 0 on the edges, then each
/// point moved onto the true curve by the extremum of G along the major Hessian eigenvector.
fn smooth_curves(f: &Field, kind: f64) -> Vec<Vec<Pt>> {
    let n = f.p1.len();
    let mut det = vec![f64::NAN; n];
    let mut theta = vec![0.0; n];
    for k in 0..n {
        let [gy, gz, hyy, hyz, hzz] = f.d[k];
        let (mean, half) = ((hzz + hyy) / 2.0, ((hzz - hyy) / 2.0).hypot(hyz));
        let (hi, lo) = (mean + half, mean - half);
        let major = if hi.abs() >= lo.abs() { hi } else { lo };
        theta[k] = 0.5 * (2.0 * hyz).atan2(hzz - hyy) + if major == hi { 0.0 } else { std::f64::consts::FRAC_PI_2 };
        if !f.jump[k] && kind * major > 0.0 {
            det[k] = gz * (hyz * gz + hyy * gy) - gy * (hzz * gz + hyz * gy);
        }
    }
    let mut points = HashMap::new();
    for (axis, i, j, a, b) in f.edges() {
        let (da, db) = (det[a], det[b]);
        if da.is_nan() || db.is_nan() || sign(da) == sign(db) { continue }
        let t = da / (da - db);
        let (pa, pb) = (f.pos(a), f.pos(b));
        let q = (pa.0 + t * (pb.0 - pa.0), pa.1 + t * (pb.1 - pa.1));
        points.insert((axis, i, j), Pt { q, pa: 0.0, pb: 0.0, n: (0.0, 0.0) });
    }
    let mut keys: Vec<_> = points.keys().copied().collect();
    keys.sort();
    let chains = chain_cells(&keys, &points);
    let span = 2.0 * f.hz.max(f.hy);
    let dt = 0.25 * f.hz.min(f.hy);
    let refined: Vec<Vec<Option<Pt>>> = chains.par_iter().map(|ch| ch.iter().map(|key| {
        let q0 = points[key].q;
        let k = f.nearest(q0);
        let e = (theta[k].cos(), theta[k].sin());
        let hints = [f.p1[k], f.p2[k]];
        let at = |t: f64| (q0.0 + t * e.0, q0.1 + t * e.1);
        let obj = |t: f64| kind * g_local(at(t), &hints);
        let (mut a, mut b) = (-span, span);
        for _ in 0..40 {
            let (x1, x2) = (b - GOLD * (b - a), a + GOLD * (b - a));
            if obj(x1) < obj(x2) { b = x2 } else { a = x1 }
        }
        let t = (a + b) / 2.0;
        // An interior extremum, not a bracket edge ...
        if !(t.abs() < span - dt && obj(t) <= obj(t - dt) && obj(t) <= obj(t + dt)) { return None }
        // ... and smooth there: same branch, same conversion state and same vz sign either
        // side, or it is a crease, which crease_curves locates exactly.
        let (qa, qb) = (at(t - dt), at(t + dt));
        let pa = local_max(qa.1, qa.0, hints[0], 2.0).0;
        let pb = local_max(qb.1, qb.0, hints[0], 2.0).0;
        let smooth = (pa - pb).abs() < JUMP && converts(qa.1, pa) == converts(qb.1, pb) && (qa.0 < 0.0) == (qb.0 < 0.0);
        smooth.then(|| Pt { q: at(t), pa, pb, n: e })
    }).collect()).collect();
    let mut out = vec![];
    for ch in refined {
        let mut run = vec![];
        for p in ch.into_iter().chain(std::iter::once(None)) {
            match p {
                Some(p) => run.push(p),
                None => {
                    if run.len() >= 3 { out.push(std::mem::take(&mut run)) }
                    run.clear();
                }
            }
        }
    }
    out
}

// ---- output ---------------------------------------------------------------------------------

fn write_npy(path: &std::path::Path, descr: &str, shape: &[usize], bytes: &[u8]) {
    let shape_s = match shape {
        [n] => format!("({n},)"),
        _ => format!("({})", shape.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")),
    };
    let mut header = format!("{{'descr': '{descr}', 'fortran_order': False, 'shape': {shape_s}, }}");
    let total = 10 + header.len() + 1;
    header.push_str(&" ".repeat((64 - total % 64) % 64));
    header.push('\n');
    let mut file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    file.write_all(b"\x93NUMPY\x01\x00").unwrap();
    file.write_all(&(header.len() as u16).to_le_bytes()).unwrap();
    file.write_all(header.as_bytes()).unwrap();
    file.write_all(bytes).unwrap();
}

fn write_f64(dir: &std::path::Path, name: &str, shape: &[usize], data: &[f64]) {
    let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
    write_npy(&dir.join(format!("{name}.npy")), "<f8", shape, &bytes);
}

/// The one-tick energy field's grid and curves, as .npy files (see the module docs).
#[derive(Parser)]
struct Args {
    /// vz_lo,vz_hi,vy_lo,vy_hi in blocks/second; the default is elytra-vario's chart.
    #[arg(long, value_delimiter = ',', num_args = 1, allow_hyphen_values = true,
          default_value = "-10,60,-30,40")]
    window: Vec<f64>,
    /// Samples across the window: N spaces both axes equally (N across the longer one),
    /// Nz,Ny sets each.
    #[arg(long, value_delimiter = ',', num_args = 1, default_value = "1050")]
    samples: Vec<f64>,
    /// Computed past each edge, as a fraction of that axis's span, so no edge effect is in view.
    #[arg(long, default_value_t = 0.05)]
    pad: f64,
    /// Skip the curves: only the grid.
    #[arg(long)]
    no_curves: bool,
    /// Directory to write into.
    #[arg(long)]
    out: std::path::PathBuf,
}

fn main() {
    let a = Args::parse();
    let w = &a.window;
    assert!(w.len() == 4 && w[0] < w[1] && w[2] < w[3], "--window wants vz_lo,vz_hi,vy_lo,vy_hi");
    let s = &a.samples;
    assert!(matches!(s.len(), 1 | 2), "--samples wants N or Nz,Ny");
    let (sz, sy) = if s.len() == 1 {
        // Equal spacing: the longer axis gets the samples.
        let per = s[0] / (w[1] - w[0]).max(w[3] - w[2]);
        (per * (w[1] - w[0]), per * (w[3] - w[2]))
    } else { (s[0], s[1]) };
    let pad = a.pad;
    let out = a.out.as_path();
    std::fs::create_dir_all(out).unwrap();
    let axis = |lo: f64, hi: f64, n: f64| {
        let (lo, hi) = (lo / TPS, hi / TPS);
        let d = pad * (hi - lo);
        let count = (n * (1.0 + 2.0 * pad)).round() as usize + 1;
        (0..count).map(|i| lo - d + (hi - lo + 2.0 * d) * i as f64 / (count - 1) as f64).collect::<Vec<f64>>()
    };
    let (vz, vy) = (axis(w[0], w[1], sz), axis(w[2], w[3], sy));
    let t = std::time::Instant::now();
    let f = Field::new(vz, vy);
    eprintln!("grid {} x {} in {:.1?}", f.nz(), f.ny(), t.elapsed());

    let shape = [f.ny(), f.nz()];
    write_f64(out, "vz", &[f.nz()], &f.vz);
    write_f64(out, "vy", &[f.ny()], &f.vy);
    write_f64(out, "p1", &shape, &f.p1);
    write_f64(out, "G", &shape, &f.big_g);
    write_f64(out, "p2", &shape, &f.p2);
    write_f64(out, "g2", &shape, &f.g2);
    let st: Vec<u8> = f.stuck.iter().map(|&b| b as u8).collect();
    write_npy(&out.join("stuck.npy"), "|u1", &shape, &st);
    for (c, name) in ["gy", "gz", "hyy", "hyz", "hzz"].iter().enumerate() {
        write_f64(out, name, &shape, &f.d.iter().map(|d| d[c]).collect::<Vec<_>>());
    }

    if !a.no_curves {
        let t = std::time::Instant::now();
        let mut rows: Vec<f64> = vec![];
        let mut id = 0.0;
        let mut push = |kind: Kind, class: i8, pts: &[Pt], rows: &mut Vec<f64>| {
            for p in pts { rows.extend([id, kind as u8 as f64, class as f64, p.q.0, p.q.1]) }
            id += 1.0;
        };
        for (kind, class, pts) in crease_curves(&f) { push(kind, class, &pts, &mut rows) }
        let t_crease = t.elapsed();
        for (sign, class) in [(1.0, 1), (-1.0, -1)] {
            for pts in smooth_curves(&f, sign) { push(Kind::Smooth, class, &pts, &mut rows) }
        }
        write_f64(out, "curves", &[rows.len() / 5, 5], &rows);
        eprintln!("curves: creases {:.1?}, smooth {:.1?}", t_crease, t.elapsed() - t_crease);
    }
    std::fs::write(out.join("spec.txt"), format!(
        "field --window {} --samples {} --pad {pad}\n", w.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","),
        s.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))).unwrap();
}
