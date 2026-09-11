#!/usr/bin/env python3
"""The schedules of one cell, replayed into velocity space over the energy field.

    python3 tools/plot_field_replay.py [cell] [outdir] [--snap | --dive]

The overlay figure draws pitch against tick, which is the control. This draws where that
control *puts you*: the same schedules as trajectories in the (vz, vy) velocity plane, over
the field of how much total energy the best available pitch could make from each velocity.
The field is the thing the cycle has to be flown around -- inside the gain region a cycle can
pay for itself, outside it nothing can -- and it is invisible on a pitch-versus-tick plot.

Background from elytra-vario's chart rather than this repo's vector field. Two reasons, both
elytra-vario's own: the sign flip is a near-black seam instead of two purples that read alike,
and both arms climb in brightness as well as in hue, so magnitude survives being printed. The
constants below are that mod's defaults, copied rather than imported because there is no path
between a Rust repo and a Gradle one -- see VarioConfig and EnergyFieldTexture if they drift.

Curve colors are the overlay's, unchanged: viridis within the cyclic class, flat gray for
COLLAPSED, on scales local to the cell. See plot_atlas.mode_scales for why per class. They are
haloed rather than recolored, and that is not a nicety -- see HALO.

`--snap` draws the snap-to-zero phase alone, zoomed, over the pitch-zero field instead of the
argmax one. Two different questions. The argmax field is what the *best* pitch could make from
each velocity, so it says where a cycle could pay for itself; over a phase that holds one pitch
it is the wrong background, because it prices a control nobody is flying. The pitch-zero field
is what this phase is actually collecting tick by tick, so the curves and the ground underneath
them finally refer to the same control, and a schedule's drift across the seam is its own
energy budget rather than a comparison with a counterfactual.

The ramp constants do not change between the two, deliberately, so a color means the same
blocks/tick in both. The pitch-zero field is shallow here -- about a third of a block/tick of
loss at worst against the argmax field's four across the envelope -- so the zoom comes out
muted, and that muting is the finding, not a rendering fault. The seam is drawn as a contour as
well as a color for exactly that reason: at this depth the gain arm is within a few code values
of the zero color.

The two backgrounds then turn out to be the same picture, and that is the result the zoom
exists to show. Over every cell of the snap window the best whole degree is zero, so the
pitch-zero field and the argmax field agree to 0.000 blocks/tick -- the phase is not merely
near the myopic optimum, it is flying it pointwise, and the schedules hold inside one degree
of level while they do. The level-optimal region is much larger than the window (at vz 1.8 it
runs vy -1.26 to +0.45), so the phase sits inside it with margin rather than tracking its
edge. The consequence for reading the figure: the seam is where *anything* breaks even, not
just where level does, and a schedule crossing it is leaving the region where the cycle can
pay for itself at all.

`--dive` is that same zoom one phase earlier. The cut starts where the nose begins to drop
rather than at the bottom of the dive, so the figure carries the entry as well as the pullout
and the hold: 16 to 27 ticks against the snap window's 14 to 19, of which about six are the
dive in and four the pullout out. The window barely moves -- the dive is a few ticks long and
almost flat in vz, so it buys 0.09 blocks/tick of extra width and no extra height -- and what
changes on the page is that every curve gains a hook at its left end.

The background goes back to the argmax field there, because the argument for the pitch-zero
one was that the phase held pitch zero and this phase sweeps eighty degrees of it. That costs
next to nothing: over this window the two fields are still identical on 95.9% of cells and
differ by at most 0.32 blocks/tick on the rest, which is the low corner the dive itself reaches
and nowhere else. So the two zooms can be read against each other as if the ground were the
same, because very nearly it is.

What the dive figure adds is that the entry is not one behavior. 38 of the 111 schedules move
the nose less than five degrees over that run: they glide at a steady forty-odd, vy sags to a
shallow minimum, and the pullout starts with nothing that deserves the name dive. Those are the
short curves on the figure, and no threshold makes them longer -- see dive_start.
"""

import glob
import math
import os
import sys
import textwrap

import matplotlib
matplotlib.use("Agg")
import matplotlib.lines
import matplotlib.patheffects
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import plot_atlas
from load import load

GRAVITY = 0.08
# f32 in the sim, and 0.99f32 is not 0.99. Same constants tools/load.py replays with.
DRAG_Y, DRAG_Z = 0.9800000190734863, 0.9900000095367432

# elytra-vario VarioConfig: the chart's domain, in blocks/tick.
VX_LO, VX_HI, VY_LO, VY_HI = -0.5, 3.0, -1.5, 2.0
# ... its ramp. Loss and gain both run from the same near-black zero, mixed in sRGB.
FIELD_ZERO, FIELD_GAIN, FIELD_LOSS = "#0C0D10", "#9E3692", "#4A70A8"
# ... and the energy change at which that ramp is half way to saturated. The field spans about
# four blocks/tick end to end and everything worth seeing is in the first tenth, so it
# compresses by |x|/(|x|+SCALE) rather than clipping: never saturates, smooth through zero.
FIELD_SCALE = 0.4
# elytra-vario HudChrome: white at 15% for the grid, 40% for the zero axes. The mod's grid is
# every half block/tick, which is right for the whole envelope and far too coarse for a zoom
# onto one phase -- so the step is the coarsest of these that still puts four lines across the
# narrower axis, and the half-block spacing is simply the first rung of that ladder.
GRID_A, AXIS_A = 0x26 / 255, 0x66 / 255
GRID_STEPS = (0.5, 0.25, 0.1, 0.05, 0.025, 0.01)

# Vanilla clamps pitch to ninety; elytra-vario leaves the last degree out of the sweep, so the
# field here is the same whole-degree sweep it paints. The schedules are capped at 85 by the
# optimizer, which is a fact about them and not about the field they are drawn on.
PITCH_LIMIT = 89
PIXELS_PER_VEL = 260     # square pixels in velocity units, as on the mod's chart

# Degrees per tick below which the nose counts as not moving, for --dive. Not a place on the
# curve: see dive_start for why the answer does not depend on it where there is a dive to find.
DIVE_RATE = 1.0

# Every curve gets a near-black outline, in the field's own zero color.
#
# The overlay's ramp cannot simply be dropped on this background, and no subrange of it fixes
# that. Relative luminance: the field runs 0.004 at the seam to 0.12 (gain) and 0.16 (loss),
# and viridis's lower half runs 0.02 to 0.16 -- it traverses exactly the range the field
# occupies. Measured as contrast ratios, viridis(0.00) is 1.28 against the seam, and lifting
# the floor to viridis(0.25) trades that for 1.23 against the gain arm and 1.51 against the
# loss arm. There is nowhere to move it to: darker collides with the seam, lighter collides
# with the arms, and only viridis's top third clears both.
#
# So separate the curves by edge instead of by fill. An outline in the seam's color is 3.1
# against the gain arm and 4.0 against the loss arm whatever it encloses, so a dark-purple
# low-dJ curve reads over magenta without being repainted -- and the overlay's colors, which
# are what makes the two figures comparable at a glance, survive exactly as they are.
HALO = "#0C0D10"


def step(vy, vz, pitch):
    """One tick of update_fall_flying_movement at yaw zero, over whole arrays of velocity.

    A transcription of the scalar replay in tools/load.py, kept elementwise so the field --
    a million velocities times a hundred and seventy-nine pitches -- is seconds and not an
    afternoon. The pitch is scalar, so every test on it stays a plain branch; only the tests
    on velocity have to become masks.
    """
    lean = math.radians(pitch)
    look_z = math.cos(lean)
    look_hor = abs(look_z)
    lift = look_z * look_z
    move_hor = np.abs(vz)                     # captured before any of this tick's updates

    vy = vy + GRAVITY * (-1.0 + lift * 0.75)
    if look_hor > 0.0:
        conv = np.where(vy < 0.0, vy * -0.1 * lift, 0.0)       # descent -> forward
        vy = vy + conv
        vz = vz + look_z * conv / look_hor
        if lean < 0.0:                                         # forward -> up, nose above level
            conv = move_hor * -math.sin(lean) * 0.04
            vy = vy + conv * 3.2
            vz = vz - look_z * conv / look_hor
        vz = vz + (look_z / look_hor * move_hor - vz) * 0.1     # turning
    return vy * DRAG_Y, vz * DRAG_Z


def gain(vy, vz, pitch):
    """Total energy one tick at this pitch makes from each velocity, in blocks.

    Total energy is height: kinetic is |v|^2/2g and potential is the altitude, and vanilla
    advances position after velocity, so the tick's climb is the *new* vy. Dropping that term
    would score a dive and a climb alike and the whole picture would mean nothing.
    """
    vy1, vz1 = step(vy, vz, pitch)
    return (vy1 * vy1 + vz1 * vz1 - vy * vy - vz * vz) * 0.5 / GRAVITY + vy1


def best_gain(vy, vz):
    """The most any pitch could make from each velocity. elytra-vario's field."""
    best = np.full(vy.shape, -np.inf)
    for pitch in range(-PITCH_LIMIT, PITCH_LIMIT + 1):
        np.maximum(best, gain(vy, vz, pitch), out=best)
    return best


def hold_span(pitches):
    """[lo, hi) of the flat run immediately before the flick, by a threshold on pitch.

    plot_atlas.hold0 measures the length of this same run and this returns its bounds; the
    walk is character for character that one's. None when the schedule never flicks.

    Kept for comparison against vel_span, and not the default, because the threshold is doing
    more work than it looks. The pullout into the hold is not a corner for every schedule: most
    drop from +19 to exactly 0.000 in one tick, but the late-flick ones decay geometrically at
    about 0.75 a tick and are still descending when the flick arrives. For those there is no
    corner to find, so the reported start is wherever that decay crosses one degree -- eight
    ticks for the eight latest schedules against a mean of twelve, and they are the visibly
    short curves on the figure. Raising the threshold to three degrees lengthens them to ten
    and empties the short bin, which is not a fix: it just moves an arbitrary cut along a
    smooth curve. vel_span removes the cut instead.
    """
    f = plot_atlas.flick_tick(pitches)
    if pitches[f] > -30.0:
        return None
    hi = f
    while hi > 0 and abs(pitches[hi]) > 1.0:
        hi -= 1
    hi += 1
    lo = hi - 1
    while lo > 0 and abs(pitches[lo - 1]) <= 1.0:
        lo -= 1
    return lo, hi


def vel_span(vy, vz):
    """[s, e] of the phase, bounded by the trajectory's own turning points.

    Start at the tick of minimum vy -- the bottom of the dive, where the pullout begins -- and
    run to the tick vz stops rising, which is the top of the speed the pullout is trading that
    descent for. Both ends are extrema of the state, so nothing here is a threshold on the
    control and the answer cannot be tuned.

    This is the phase hold_span was reaching for. Measured against it: every schedule gets 14
    to 19 ticks, mean 16.0, where the pitch threshold gave 8 to 14 and put the eight late-flick
    schedules in a short bin of their own. Aligned on min-vy the shape is the same for all 111
    -- median pitch +41.9 at the bottom, then +27.3, +15.4, +7.1, and exactly 0.000 from the
    fourth tick to the end. The threshold was not finding a shorter phase in those eight, it
    was starting partway down their pullout.

    The cost is that the phase now contains its own entry ramp, so it is not all level flight:
    73% of its ticks are within a degree of level and the other 27% are the first three or four.
    """
    s = int(np.argmin(vy))
    e = s
    while e + 1 < len(vz) and vz[e + 1] >= vz[e]:
        e += 1
    return s, e


def dive_start(pitches, i_min):
    """(a, m): where the nose starts dropping into the dive, and where it is steepest.

    The two ends vel_span finds are turning points of the state, and the tick this returns is
    not -- it is a turning point of the control, the shallowest the nose gets before it goes
    down. That asymmetry is forced. In velocity space the dive's entry is not an extremum of
    anything: vy is already falling, slowly, and the dive just makes it fall faster, so there
    is no corner in the trajectory to find. The corner is in the pitch.

    Walk back from the bottom of the dive to the steepest the nose gets (m, an extremum, so no
    threshold), then back through the run where the nose is still dropping at DIVE_RATE or
    more. The floor is there to stop the walk on a glide rather than to locate anything: a
    schedule that dives does it at 5 to 12 degrees a tick against a glide that wanders by a few
    tenths, and every rate from 0.5 to 2 gives the same tick for at least 96 of the 111
    schedules here. Below that separation the walk runs away down the glide's own wiggle -- at
    a floor of zero it goes back as far as 150 ticks -- and above it, it starts clipping the
    dive.

    Not every schedule has a dive. 38 of the 111 change pitch by less than five degrees over
    this run: they glide at a steady forty-odd degrees, vy drifts down to a shallow minimum,
    and the pullout starts with no flick into it at all. For those, a and m come out within a
    tick or two of the minimum and the drawn curve is short, which is the fact about them and
    not a failure of the cut -- there is no dive there to find, at any threshold.
    """
    m = i_min
    while m > 0 and pitches[m - 1] > pitches[m]:
        m -= 1
    a = m
    while a > 0 and pitches[a] - pitches[a - 1] >= DIVE_RATE:
        a -= 1
    return a, m


def field_rgb(gains):
    """elytra-vario's ramp: compress the magnitude, then mix zero->gain or zero->loss in sRGB.

    sRGB rather than linear light. Linear is the right way to mix two lights, but this is
    laying out a scale, and on a ramp from near-black to a saturated color the linear version
    spends most of its length near the dark end and arrives at the mid-tones desaturated.
    """
    to_rgb = matplotlib.colors.to_rgb
    zero = np.array(to_rgb(FIELD_ZERO))
    ends = np.where(gains[..., None] < 0.0, np.array(to_rgb(FIELD_LOSS)),
                    np.array(to_rgb(FIELD_GAIN)))
    t = (np.abs(gains) / (np.abs(gains) + FIELD_SCALE))[..., None]
    return zero + (ends - zero) * t


def replay(profile):
    """Velocity per tick from the pitches alone, as arrays. Start state included, as tick 0."""
    vy0, vz0 = profile.v0
    vy, vz = [vy0], [vz0]
    for pitch in profile.pitches:
        a, b = step(np.float64(vy[-1]), np.float64(vz[-1]), pitch)
        vy.append(float(a)); vz.append(float(b))
    return np.array(vy), np.array(vz)


def main():
    argv = [a for a in sys.argv[1:] if not a.startswith("--")]
    snap = "--snap" in sys.argv
    # The same zoom one phase earlier: start at the top of the dive rather than at its bottom,
    # so the figure holds the nose-down entry, the pullout and the level hold instead of only
    # the last two. The background changes back to the argmax field with it, because the
    # argument for the pitch-zero one was that the phase held pitch zero and this phase does
    # not -- it sweeps eighty degrees. That costs nothing in comparability: over this window
    # the two fields are the same picture to within `gap`, printed below.
    dive = "--dive" in sys.argv
    zoom = snap or dive
    # The pitch-threshold cut is still reachable, but it is not the default: see hold_span.
    by_pitch = "--cut=pitch" in sys.argv
    # Off by default. An arrow per dot along the nose, colored by pitch, was tried as a
    # permanent part of this figure and does not survive contact with the density: 2000 of them
    # over a fan this tight reads as noise, and the thing they were meant to show -- that the
    # nose and the curve's tangent are nearly perpendicular, because this is velocity space and
    # lift acts across the nose -- is a single number better said in a sentence than drawn 2000
    # times. It is 96 degrees at worst. Kept behind the flag because it is the right instrument
    # for one schedule at a time, just not for a hundred and eleven at once.
    arrows = "--arrows" in sys.argv
    # How many ticks to show either side of the phase proper. One is enough to make the cut
    # legible: with it, the tick that defines each end is a visible turning point of the curve
    # rather than a claim in the caption -- vy comes down, reaches the circle, and goes back up;
    # vz runs out to the square and falls away. Zero ticks of margin and both ends are just
    # where the line happens to stop, which is indistinguishable from a clipped window.
    MARGIN = 1
    cell = argv[0] if argv else "runs/atlas/cells/flicksoft30_v00_n300"
    out = argv[1] if len(argv) > 1 else "runs/atlas/fig/profiles"
    tag = os.path.basename(cell.rstrip("/"))
    os.makedirs(out, exist_ok=True)
    plot_atlas.theme()

    rows, held = [], []
    for f in sorted(glob.glob(f"{cell}/*.pitches")):
        p = load(f)
        if "certified" not in p.header:
            continue
        vy, vz = replay(p)
        if zoom:
            if plot_atlas.flick_tick(p.pitches) is None or p.header["structure"] != "cyclic":
                continue                  # no flick, so no phase to cut out of it
            if by_pitch:
                span = hold_span(p.pitches)
                if span is None:
                    continue
                lo, hi = span
            else:
                lo, hi = vel_span(vy, vz)
            head, nose = lo, lo                     # where the cut starts; the steepest nose
            if dive:
                head, nose = dive_start(p.pitches, lo)
            # Velocity index t is the state *before* tick t, so the ticks [head, hi) are the
            # velocities [head, hi]: one more point than ticks, which is what a path needs.
            a, b = max(0, head - MARGIN), min(len(p.pitches), hi + MARGIN)
            pit = p.pitches[a:b]
            vy, vz = vy[a:b + 1], vz[a:b + 1]
            i_head, i_min, i_peak = head - a, lo - a, hi - a     # in the grown array
            # i_head had a marker on it once, under --dive, on the grounds that the cut's
            # start is a turning point of the control rather than of the state and so is not
            # visible as a corner the way vel_span's two ends are. Dropped: 111 more glyphs on
            # an already dense fan, to locate something the caption states in words.
            # Where the pullout finishes: the first tick at or after the dive's bottom that is
            # inside a degree of level. A third marker, because under vel_span the phase starts
            # at +42 and only becomes the level hold a few ticks in, and that split is the
            # shape worth seeing.
            k = next((i for i in range(i_min, len(pit)) if abs(pit[i]) <= 1.0), i_peak)
            held.append((hi - head, max(abs(x) for x in p.pitches[head:hi]), k - i_min,
                         lo - head, p.pitches[nose] - p.pitches[head]))
        rows.append((float(p.header["dJ"]), p.header["structure"], vy, vz,
                     *((i_head, i_peak, k, pit) if zoom else (0, 0, 0, None))))
    assert rows, f"no {'holds' if zoom else 'certified profiles'} under {cell}"
    rows.sort(key=lambda r: r[0])            # ascending dJ, so the good ones land on top
    scales = plot_atlas.mode_scales([(dj, None, cls, None) for dj, cls, *_ in rows])

    if zoom:
        # Fit the phase, with a margin wide enough that the seam's shape around it is legible.
        pad = .04
        lo_z, hi_z = min(r[3].min() for r in rows) - pad, max(r[3].max() for r in rows) + pad
        lo_y, hi_y = min(r[2].min() for r in rows) - pad, max(r[2].max() for r in rows) + pad
    else:
        # The mod's domain is sized for a working pump cycle, and a cell full of partial optima
        # is not obliged to stay inside it. Grow it to hold every trajectory rather than
        # clipping one silently, keeping the pixel square so a distance still means a fixed
        # change in speed.
        lo_z = min(VX_LO, min(r[3].min() for r in rows) - .05)
        hi_z = max(VX_HI, max(r[3].max() for r in rows) + .05)
        lo_y = min(VY_LO, min(r[2].min() for r in rows) - .05)
        hi_y = max(VY_HI, max(r[2].max() for r in rows) + .05)
    px = PIXELS_PER_VEL * (4.0 if zoom else 1.0)      # the zoom is a twentieth of the envelope
    w = max(2, round((hi_z - lo_z) * px))
    h = max(2, round((hi_y - lo_y) * px))
    vz_g, vy_g = np.meshgrid(np.linspace(lo_z, hi_z, w),
                             np.linspace(hi_y, lo_y, h))      # row 0 is the top of the chart
    gains = gain(vy_g, vz_g, 0.0) if snap else best_gain(vy_g, vz_g)
    # What the argmax field would have said over this same window. Computed rather than quoted
    # from the other figure, because the two windows are different sizes -- and because the
    # answer is the point of the zoom rather than a footnote to it: the gap is identically
    # zero. See `gap` in the caption.
    gap = lvl_frac = None
    if zoom:
        diff = np.abs((best_gain(vy_g, vz_g) if snap else gain(vy_g, vz_g, 0.0)) - gains)
        gap = float(diff.max())
        lvl_frac = float((diff < 1e-12).mean()) * 100.0
    step_v = next((g for g in GRID_STEPS if min(hi_z - lo_z, hi_y - lo_y) / g >= 4),
                  GRID_STEPS[-1])

    fig, ax = plt.subplots(figsize=(13.5, 13.5 * (hi_y - lo_y) / (hi_z - lo_z) + 1.5))
    ax.imshow(field_rgb(gains), extent=(lo_z, hi_z, lo_y, hi_y), origin="upper",
              interpolation="nearest", aspect="equal", zorder=0)
    for v in np.arange(math.ceil(lo_z / step_v) * step_v, hi_z + 1e-9, step_v):
        ax.axvline(v, color="w", lw=.8, alpha=AXIS_A if abs(v) < 1e-9 else GRID_A, zorder=1)
    for v in np.arange(math.ceil(lo_y / step_v) * step_v, hi_y + 1e-9, step_v):
        ax.axhline(v, color="w", lw=.8, alpha=AXIS_A if abs(v) < 1e-9 else GRID_A, zorder=1)
    if zoom and gains.min() < 0 < gains.max():
        # The seam, as a line as well as a color. At this depth the gain arm never gets more
        # than a fifth of the way up its ramp, so the sign change is a few code values wide and
        # would be invisible -- and it is the one feature of the background worth locating
        # exactly. Drawn in the chrome's own white so it reads as a rule, not as data.
        ax.contour(vz_g, vy_g, gains, levels=[0.0], colors="w", linewidths=1.0, alpha=.5,
                   zorder=1)

    halo = [matplotlib.patheffects.withStroke(linewidth=2.1, foreground=HALO, alpha=.85)]
    for dj, cls, vy, vz, *_ in rows:
        # Ascending dJ, so the best cyclic schedules land on top -- the overlay's order. The
        # other classes are then drawn again over the top, because in velocity space they are
        # short: the whole COLLAPSED class is one hook from the origin out to vz 0.46, inside
        # the corner all 111 cyclic schedules dive through, and it is buried by the first few
        # of them. On a pitch-versus-tick plot it separates by shape and does not need this.
        # (In --snap it does not arise: no flick means no hold, so the class is not drawn.)
        ax.plot(vz, vy, lw=.8, alpha=.8, color=scales[cls][0](dj),
                zorder=2 if cls == "cyclic" else 3, path_effects=halo,
                marker="." if zoom else None, ms=3.2, solid_capstyle="round")
    # No figure here carries a marker. A circle at min vy and a square at max vz went first,
    # because MARGIN already does that job -- with a tick drawn past each end the turn is
    # visible in the curve itself, and a glyph on a visible corner only repeats it. Then the
    # cross at the end of the pullout and the ring at the start of the dive, which mark things
    # the curve genuinely does not show, and which went anyway: on a fan of 111 schedules a
    # hundred-odd glyphs cost more legibility than the tick they locate is worth, and the phase
    # lengths they stood in for are in the title as numbers. Last was the ring on v0 on the
    # full-envelope figure, which the chrome already locates: every curve leaves from the same
    # state and this cell's is the origin, so the two bright zero axes cross on it. (For a cell
    # whose v0 is not the origin that stops being true, and the start goes unmarked -- the
    # cells drawn here are all v00, and a marker for a case nobody plots is not worth its
    # code.)
    if zoom and arrows:
        # An arrow per dot, along the nose, colored by pitch. The cut spans
        # nearly a hundred degrees of pitch -- a median of +53 at the first tick shown, level
        # for most of the middle, -10 at the last -- and the trajectory alone cannot show that,
        # because the curve is where the velocity went and the arrow is where the player was
        # pointing, and through the whole pullout those are not the same thing. The clearest
        # reading on the figure is exactly that disagreement, and it is close to a right angle:
        # a curve here is a path through *velocity* space, so its tangent is the acceleration,
        # and lift acts across the nose rather than along it. Measured below as `skew`.
        #
        # At yaw zero the look vector is (0, -sin p, cos p) -- `step` reads lean < 0 as nose up
        # -- so in the (vz, vy) plane the nose points along (cos p, -sin p). Fixed length: only
        # the direction carries information, and scaling by anything would imply otherwise.
        az = np.concatenate([r[3][:len(r[7])] for r in rows])
        ay = np.concatenate([r[2][:len(r[7])] for r in rows])
        ap = np.concatenate([np.array(r[7]) for r in rows])
        # How far the nose is from the direction the velocity is actually being pushed. Stated
        # in the caption, and computed rather than asserted so it cannot go stale: an earlier
        # draft guessed "about forty degrees" from the nose-down angle at the circle and was
        # wrong by more than a factor of two.
        rad = np.radians(ap)
        nose = np.stack([np.cos(rad), -np.sin(rad)])
        tang = np.concatenate([np.stack([np.diff(r[3]), np.diff(r[2])])[:, :len(r[7])]
                               for r in rows], axis=1)
        tang = tang / np.linalg.norm(tang, axis=0)
        skew = np.degrees(np.arccos(np.clip((nose * tang).sum(0), -1.0, 1.0))).max()
        # Quiver sizes the head in multiples of the shaft width, not of the arrow, so a short
        # arrow with default heads is all head: at width .0022 of the axes the head came to
        # .0134 in data units against an arrow of .0135, i.e. a triangle with no shaft, which
        # is unreadable at any density. Keep the head near a third of the length instead.
        L = (hi_z - lo_z) * 0.014
        shaft = .0013
        # The ramp spans exactly the pitch present, not a symmetric range around zero -- with
        # the margin ticks the phase runs -19 to +79, so a symmetric bar would leave its whole
        # lower half unused and compress everything real into the top. Two slopes rather than a
        # plain Normalize over that range, because zero has to stay the pale middle: level
        # flight is the value this figure exists to locate, and under a plain ramp from -19 to
        # +79 it would land a fifth of the way up and render blue.
        q = ax.quiver(az, ay, L * np.cos(rad), -L * np.sin(rad), ap,
                      angles="xy", scale_units="xy", scale=1.0, cmap="coolwarm",
                      norm=matplotlib.colors.TwoSlopeNorm(0.0, float(ap.min()),
                                                          float(ap.max())),
                      width=shaft, headwidth=3.0, headlength=3.5, headaxislength=3.0,
                      zorder=6)
        fig.colorbar(q, ax=ax, label="pitch, deg  (+ nose down)", pad=.015)

    ax.set_xlim(lo_z, hi_z)
    ax.set_ylim(lo_y, hi_y)
    ax.set_xlabel("vz, blocks/tick  (horizontal speed; x20 for blocks/second)")
    ax.set_ylabel("vy, blocks/tick  (climb rate)")
    if zoom:
        # held rows are (ticks, max |pitch|, pullout ticks, entry ticks, the dive's amplitude).
        ticks = [h[0] for h in held]
        lvl = sum(h[2] for h in held) / len(held)
        entry = sum(h[3] for h in held) / len(held)
        amps = sorted(h[4] for h in held)
        cut = ("min-vy to the vz peak" if not by_pitch else "|pitch| <= 1 deg")
        # Wrapped, not just newlined: two colorbars eat the right third of the figure, and an
        # ax title is centered on the axes rather than the canvas, so a long line runs under
        # them.
        lines = ((f"{tag}: the dive, the pullout and the level hold of all {len(rows)} cyclic "
                  f"schedules, over the energy field",
                  f"cut from where the nose starts dropping to the vz peak, plus a tick either "
                  f"side: {min(ticks)}-{max(ticks)} ticks each "
                  f"(mean {sum(ticks) / len(ticks):.1f}) -- {entry:.1f} down into it, "
                  f"{lvl:.1f} back out, the rest flown level; the nose drops "
                  f"{amps[len(amps) // 2]:.0f} deg at the median and {amps[-1]:.0f} at most")
                 if dive else
                 (f"{tag}: the snap-to-zero phase of all {len(rows)} cyclic schedules, over the "
                  f"pitch-zero field",
                  f"cut at {cut}, plus a tick either side: {min(ticks)}-{max(ticks)} ticks each "
                  f"(mean {sum(ticks) / len(ticks):.1f})"
                  + (f", the first {lvl:.1f} of them the pullout and the rest flown level"
                     if not by_pitch else
                     f", held within {max(h[1] for h in held):.2f} deg of level")
                  + (" -- where level is the 1-tick argmax" if gap == 0.0 else "")))
        ax.set_title("\n".join(textwrap.fill(t, 88) for t in lines), fontsize=12)
    else:
        ax.set_title(f"{tag}: all {len(rows)} schedules replayed into velocity space, over the "
                     f"energy field", fontsize=12)
    handles = [matplotlib.lines.Line2D([], [], color=scales[c][0](
        [r[0] for r in rows if r[1] == c][-1]), lw=2.5, label=scales[c][1])
        for c in plot_atlas.MODES if c in scales]
    if zoom and arrows:
        handles.append(matplotlib.lines.Line2D(
            [], [], color="#9AA0A6", lw=0, marker=r"$\rightarrow$", ms=9,
            label=f"nose direction, one per tick ({ap.min():+.0f} to {ap.max():+.0f} deg)"))
    if zoom and gains.min() < 0 < gains.max():
        # Not called the seam here. That is elytra-vario's word for the sign change and it is
        # right in prose, but in a legend it reads as something about the curves. Naming the
        # contour by the quantity it contours needs no gloss at all.
        handles.append(matplotlib.lines.Line2D([], [], color="w", lw=1.0, alpha=.5,
                                               label="gain=0 contour"))
    ax.legend(handles=handles, fontsize=8, loc="upper left", framealpha=.85)
    for cls in plot_atlas.MODES:
        if cls in scales and scales[cls][2] is not None:
            fig.colorbar(scales[cls][2], ax=ax, label=f"dJ, blocks -- {cls.lower()}", pad=.015)

    what = ("the total-energy change one tick at pitch 0 makes from each velocity -- the "
            "control this phase is actually holding" if snap else
            "the best total-energy change one tick can make from each velocity, over every "
            f"whole degree of pitch in +/-{PITCH_LIMIT}")
    scale_note = ""
    if zoom:
        other = "argmax" if snap else "pitch-zero"
        scale_note = (
            f" The ramp is identical in every one of these figures, so a color is the same "
            f"blocks/tick in each: this window spans {gains.min():+.2f} to {gains.max():+.2f} "
            f"against the whole envelope's -1.21 to +4.10, which is why the zoom reads muted."
            + (f" It is also the {other} field, unchanged: over every cell here the best whole "
               f"degree IS zero, so the two backgrounds differ by {gap:.3f} blocks/tick. The "
               f"seam is therefore not just where level flight breaks even but where any pitch "
               f"does, and this figure and the other zoom stand on identical ground."
               if gap == 0.0 else
               f" It is very nearly the {other} field too: level is the best whole degree over "
               f"{lvl_frac:.1f}% of this window, the two differing by at most {gap:.2f} "
               f"blocks/tick, in the low corner the pullout passes through."))
    caption = "\n".join(textwrap.fill(t, 168) for t in (
        f"background: {what}. Magenta gains, blue loses, the black seam is the boundary; "
        f"magnitude is compressed by |x|/(|x|+{FIELD_SCALE}) so the extremes never clip. "
        f"Colors are elytra-vario's chart.{scale_note}",
        "curves: colored by dJ exactly as in the overlay figure, and outlined in the seam's "
        "color -- viridis's lower half shares the field's luminance range, so no subrange of it "
        "clears both arms."
        + (("  One dot per tick.  A tick is drawn either side of the cut, so that each end "
            "defined by a turning point of the state is visibly one." if not arrows else
            f"  One dot per tick, each with the nose direction at that tick. The two are not "
            f"the same thing and mostly not even close: this is velocity space, so the curve's "
            f"tangent is the acceleration, lift acts across the nose rather than along it, and "
            f"the largest gap between arrow and tangent here is {skew:.0f} deg -- very nearly a "
            f"right angle.  A tick is drawn either side of the cut so that both defining "
            f"extrema are visibly turning points.") if zoom else "")))
    fig.text(.008, .012, caption, fontsize=8, color="#9AA0A6", va="bottom", linespacing=1.5)
    plt.tight_layout()
    fig.subplots_adjust(bottom=.105 if snap else .085)
    name = f"{out}/{tag}_field{'_snap' if snap else '_dive' if dive else ''}.png"
    plt.savefig(name, dpi=110)
    plt.close()
    print(f"wrote {name}")
    print(f"  {len(rows)} profiles, dJ {rows[0][0]:+.2f} .. {rows[-1][0]:+.2f}")
    print(f"  field {w}x{h} over vz [{lo_z:.3f}, {hi_z:.3f}], vy [{lo_y:.3f}, {hi_y:.3f}]"
          f"   grid {step_v}   gain {gains.min():+.3f} .. {gains.max():+.3f} blocks/tick")
    if zoom:
        ticks = [h[0] for h in held]
        print(f"  cut {min(ticks)}-{max(ticks)} ticks, mean {sum(ticks) / len(ticks):.2f}, "
              f"max |pitch| inside it {max(h[1] for h in held):.3f} deg")
        print(f"  argmax field vs pitch-0 field over this window: max |difference| "
              f"{gap:.6f} blocks/tick, level is argmax over {lvl_frac:.1f}% of cells")
        print(f"  pullout {sum(h[2] for h in held) / len(held):.2f} ticks on average, "
              f"then level to the vz peak")
        if dive:
            amps = sorted(h[4] for h in held)
            ent = sorted(h[3] for h in held)
            print(f"  entry {ent[0]}-{ent[-1]} ticks (mean {sum(ent) / len(ent):.2f}), nose "
                  f"drops {amps[0]:.1f}..{amps[-1]:.1f} deg, median {amps[len(amps) // 2]:.1f}"
                  f"   {sum(1 for a in amps if a < 5)} of {len(amps)} under 5 deg (no dive)")


if __name__ == "__main__":
    main()
