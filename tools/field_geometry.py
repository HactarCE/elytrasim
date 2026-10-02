"""Exact local geometry of the one-tick energy field G(v) = max over pitch of g(v, pitch).

The plots run src/bin/field.rs, which is this math in Rust and ~10x faster; this is kept for
one-off probes. Change one and you must change the other: `field_data.py --check` compares them.

g is plot_field_replay.gain: total energy, in blocks, that one tick at a pitch makes from the
velocity v = (vy, vz) in blocks/tick. Nothing here differentiates G on a grid. G is a max, so
its derivatives come from the envelope theorem at the maximizing pitch, and g itself is closed
form, so its partials are finite differences of a formula with steps far below any feature:

    grad G = g_v                                   at the argmax p*
    H_G    = g_vv - g_vp g_vp^T / g_pp             p* interior: it moves with v
    H_G    = g_vv                                  p* stuck: on a bound or a corner of g in pitch

"Stuck" is decided by the one-sided pitch slopes at p*, not by a list of known corners, so the
level corner at pitch 0 and the clamp at +-89 are both found the same way, and so would any
other. Where p* jumps between two separate maxima, G has a crease and none of this applies on
the crease itself; `branches` returns the runner-up maximum so a crease can be located as a tie.
"""

import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from plot_field_replay import GRAVITY, PITCH_LIMIT

GOLD = (math.sqrt(5) - 1) / 2
H1 = 1e-6        # blocks/tick, first derivatives in v
H2 = 1e-4        # blocks/tick, second derivatives in v
HP = 1e-3        # degrees, derivatives in pitch
# A max is stuck when the one-sided slopes at it disagree by more than this (blocks/degree):
# a smooth max has them equal to within the FD error, about HP * g_pp ~ 1e-7.
KINK = 1e-5
CHUNK = 200_000
SNAP = 1e-4     # degrees; see golden_max


def step(vy, vz, pitch):
    """One tick at yaw zero, over arrays of velocity *and* pitch."""
    lean = np.radians(pitch)
    look_z = np.cos(lean)
    lift = look_z * look_z
    move_hor = np.abs(vz)
    vy = vy + GRAVITY * (-1.0 + lift * 0.75)
    conv = np.where(vy < 0.0, vy * -0.1 * lift, 0.0)        # descent -> forward
    vy = vy + conv
    vz = vz + conv                       # look_z / look_hor is 1 over |pitch| < 90
    conv = np.where(lean < 0.0, move_hor * -np.sin(lean) * 0.04, 0.0)   # forward -> up
    vy = vy + conv * 3.2
    vz = vz - conv
    vz = vz + (move_hor - vz) * 0.1
    return vy * np.float64(0.9800000190734863), vz * np.float64(0.9900000095367432)


def g(vy, vz, pitch):
    vy1, vz1 = step(vy, vz, pitch)
    return (vy1 * vy1 + vz1 * vz1 - vy * vy - vz * vz) * 0.5 / GRAVITY + vy1


def golden_max(f, a, b, iters=48):
    """Elementwise argmax of a unimodal f on [a, b]; kinked maxima are fine."""
    for _ in range(iters):
        c, d = b - GOLD * (b - a), a + GOLD * (b - a)
        left = f(c) > f(d)
        a, b = np.where(left, a, c), np.where(left, d, b)
    p = (a + b) / 2
    # A max on the bracket's edge is approached but never sampled; take the edge if it wins.
    for e in (a, b):
        p = np.where(f(e) > f(p), e, p)
    # Golden section stalls a few microdegrees short of the level corner. Nose down, pitch enters
    # g only through cos^2, whose slope is zero at 0, so g there is flat to second order and
    # 2e-6 degrees off it moves g by ~1e-16: below double precision. The corners are at whole
    # degrees, so a max that close to one, and tied with it to rounding, is the corner.
    for corner in (0.0, -PITCH_LIMIT, PITCH_LIMIT):
        near = np.abs(p - corner) < SNAP
        p = np.where(near & (f(np.full_like(p, corner)) >= f(p) - 1e-12), corner, p)
    return p


def _branches_chunk(vy, vz):
    pitches = np.arange(-PITCH_LIMIT, PITCH_LIMIT + 1, dtype=float)
    s = np.stack([g(vy, vz, p) for p in pitches])            # (179, n)
    up = np.vstack([np.full((1, s.shape[1]), True), s[1:] >= s[:-1]])
    down = np.vstack([s[:-1] > s[1:], np.full((1, s.shape[1]), True)])
    vals = np.where(up & down, s, -np.inf)
    i1 = np.argmax(vals, axis=0)
    vals[i1, np.arange(vals.shape[1])] = -np.inf
    i2 = np.argmax(vals, axis=0)
    has2 = np.isfinite(vals[i2, np.arange(vals.shape[1])])
    out = []
    for i in (i1, i2):
        p0 = pitches[i]
        p = golden_max(lambda q: g(vy, vz, q), np.maximum(p0 - 1, -PITCH_LIMIT),
                       np.minimum(p0 + 1, PITCH_LIMIT))
        out.append((p, g(vy, vz, p)))
    (p1, g1), (p2, g2) = out
    # Refinement can reorder two near-tied maxima; keep branch 1 the better one.
    swap = has2 & (g2 > g1)
    p1, p2 = np.where(swap, p2, p1), np.where(swap, p1, p2)
    g1, g2 = np.where(swap, g2, g1), np.where(swap, g1, g2)
    return p1, g1, np.where(has2, p2, np.nan), np.where(has2, g2, -np.inf)


def branches(vy, vz):
    """(p1, G, p2, g2): the best pitch and the field, and the runner-up local max in pitch.

    Local maxima are found on the whole-degree sweep and then refined, so two maxima closer
    than a degree are one; p2 is nan and g2 -inf where pitch has a single max.
    """
    shape = vy.shape
    vy, vz = vy.ravel(), vz.ravel()
    parts = [_branches_chunk(vy[i:i + CHUNK], vz[i:i + CHUNK]) for i in range(0, len(vy), CHUNK)]
    return tuple(np.concatenate(c).reshape(shape) for c in zip(*parts))


def stuck(vy, vz, p):
    """True where the max at p is a corner or a bound: g's pitch slope jumps there."""
    gl = (g(vy, vz, p) - g(vy, vz, p - HP)) / HP
    gr = (g(vy, vz, p + HP) - g(vy, vz, p)) / HP
    at_bound = np.abs(p) >= PITCH_LIMIT - 1e-9
    return at_bound | (gl - gr > KINK)


def derivatives(vy, vz, p, is_stuck):
    """grad G and the Hessian of G at the max p: (gy, gz, hyy, hyz, hzz)."""
    f = lambda dy, dz, dp=0.0: g(vy + dy, vz + dz, p + dp)
    gy = (f(H1, 0) - f(-H1, 0)) / (2 * H1)
    gz = (f(0, H1) - f(0, -H1)) / (2 * H1)
    f0 = f(0, 0)
    hyy = (f(H2, 0) - 2 * f0 + f(-H2, 0)) / H2 ** 2
    hzz = (f(0, H2) - 2 * f0 + f(0, -H2)) / H2 ** 2
    hyz = (f(H2, H2) - f(H2, -H2) - f(-H2, H2) + f(-H2, -H2)) / (4 * H2 ** 2)
    # The pitch-coupling correction, where p* is free to move.
    gyp = (f(H2, 0, HP) - f(H2, 0, -HP) - f(-H2, 0, HP) + f(-H2, 0, -HP)) / (4 * H2 * HP)
    gzp = (f(0, H2, HP) - f(0, H2, -HP) - f(0, -H2, HP) + f(0, -H2, -HP)) / (4 * H2 * HP)
    gpp = (f(0, 0, HP) - 2 * f0 + f(0, 0, -HP)) / HP ** 2
    free = ~is_stuck & (gpp < 0)
    safe = np.where(free, gpp, -1.0)
    hyy = np.where(free, hyy - gyp * gyp / safe, hyy)
    hyz = np.where(free, hyz - gyp * gzp / safe, hyz)
    hzz = np.where(free, hzz - gzp * gzp / safe, hzz)
    return gy, gz, hyy, hyz, hzz


def grad_g(vy, vz, p):
    """grad of g at fixed pitch: the gradient of one branch, for comparing two at a tie."""
    gy = (g(vy + H1, vz, p) - g(vy - H1, vz, p)) / (2 * H1)
    gz = (g(vy, vz + H1, p) - g(vy, vz - H1, p)) / (2 * H1)
    return gy, gz


def conversion_kink(vy):
    """The pitch magnitude at which vy after gravity is 0, where descent->forward switches on.

    g has a kink in pitch there at +-this (lift is cos^2, even), and where conversion pays it is
    convex: the slope jumps up, so it is the wall between two humps rather than a max. nan where
    no pitch puts vy after gravity at 0, i.e. vy outside [0.02, 0.08) blocks/tick.
    """
    c = (GRAVITY - vy) / (0.75 * GRAVITY)
    ok = (c > 0) & (c <= 1)
    return np.where(ok, np.degrees(np.arccos(np.sqrt(np.clip(c, 0, 1)))), np.nan)


def bracket(vy, hint, width):
    """[a, b] around hint, within the clamp and not across a conversion kink.

    A bracket that crosses the kink can reach the next hump's flank, and golden section then
    climbs it to the bracket's edge: the branch being followed is lost even though it exists.
    This is what hid the tie near vz = 16 b/s, where the level hump is a fraction of a degree
    wide.
    """
    a = np.clip(hint - width, -PITCH_LIMIT, PITCH_LIMIT)
    b = np.clip(hint + width, -PITCH_LIMIT, PITCH_LIMIT)
    k = conversion_kink(vy)
    for wall in (k, -k):
        has = np.isfinite(wall)
        b = np.where(has & (wall > hint) & (wall < b), wall, b)
        a = np.where(has & (wall < hint) & (wall > a), wall, a)
    return a, b


def local_max(vy, vz, hint, width=2.0):
    """The pitch max near hint, for following one branch a short way from where it was found."""
    a, b = bracket(vy, hint, width)
    p = golden_max(lambda q: g(vy, vz, q), a, b)
    return p, g(vy, vz, p)


def is_local_max(vy, vz, p, d=1e-3):
    """Whether p is a max of g in pitch, from samples just either side (bounds count as one side)."""
    f0 = g(vy, vz, p)
    lo = (p <= -PITCH_LIMIT) | (g(vy, vz, np.maximum(p - d, -PITCH_LIMIT)) <= f0)
    hi = (p >= PITCH_LIMIT) | (g(vy, vz, np.minimum(p + d, PITCH_LIMIT)) <= f0)
    return lo & hi
