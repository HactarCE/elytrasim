# The snap-timing sweep: one window per cell, and what survives the cross product

`README-atlas.md` polishes many seeds at one cell and asks what the population of optima looks
like. This is the same instrument run across a grid instead of at a point, with one change to
how the seeds are chosen and one question it was built to answer:

**does the hold-0 invariant survive the parameters moving together?**

The atlas found the optimal hold-0 at 11 to 13 ticks in eight of nine cells and called it "the
strongest invariant in this document"; but those nine cells are the *axes* through the reference
point. An axis cannot see an interaction. The cross product can.

## The seeds are chosen in two stages, not assumed

The atlas swept the flick tick over `0.12n .. 0.96n`. Those bounds are a guess about where the
manoeuvre belongs, written as a range and then never revisited -- and a seed window is exactly
the kind of assumption that quietly decides its own answer.

So: **stage 1** scans the flick tick across the whole horizon, `0 .. n`, at stride 10. **Stage 2**
takes the best `dJ` from that scan and re-scans `+/-100` ticks around it at stride 2. The fine
window is placed by the data rather than by a prior.

The coarse profiles are deleted afterwards -- keeping them would put two tick strides in one
directory for every plot to have to separate, and they are cheap to regenerate. But the *choice*
they encode is not regenerable from nothing, so each cell keeps a `snap_window.json` holding the
whole coarse table (every tick, its `dJ`, its structure), the winner, and the resulting window.
The decision outlives its inputs.

Two things were considered and dropped. Filtering the winner to `structure == cyclic` sounds
prudent and is dead code: across 208 cells the winner is cyclic in 202, multicycle in 4, and
collapsed in 2 -- and in the single cell where no cyclic profile exists anywhere in the scan, the
filter's fallback picks the same point it would have picked anyway. And `--tol 0.002` was
dropped for `--tol 0`: the instrument is a fixed 30-pass stopping time, and in the atlas's own
`flicksoft30` cell the convergence test fired twice, silently giving two profiles a different
stopping time than the other 126 with nothing in the file to say so.

## What was run

Six grids, in six directories, because they answer six questions and mixing them makes a
listing of any one misleading:

```
runs/atlas/crossproduct   240 cells   n in {150, 300, 450} x lambda in {-2,-1,0,+1,+2}
                                      x v0 in {-0.2, 0, 0.2, 0.4}^2      -- complete 3 x 5 x 16
runs/atlas/nsweep         151 cells   n = 150 .. 450 stride 2, at lambda = 0, v0 = (0,0)
runs/atlas/nsweepv0      3636 cells   n = 150 .. 350 stride 2 x v0 in {-0.2,0,0.1,0.2,0.3,0.4}^2
                                      at lambda = 0                      -- complete 101 x 36
runs/atlas/nsweepv0fine  4949 cells   n = 150 .. 350 stride 2 x v0 in {0,.05,.1,.15,.2,.25,.3}^2
                                      at lambda = 0                      -- complete 101 x 49
runs/atlas/mapsweep      5986 cells   n = 60 .. 350 stride 2 x lambda = -2 .. 8 stride 0.25
                                      at v0 = (0, 0.4)                   -- complete 146 x 41
runs/atlas/mapfine       6601 cells   n = 120 .. 200 stride 2 x lambda = -2 .. 6 stride 0.05
                                      at v0 = (0, 0.4)                   -- complete 41 x 161
```

The first two isolate the hold-0 invariant; `nsweepv0` asks whether the best horizon depends on
where you start and `nsweepv0fine` resolves that dependence at 0.05 inside the positive quadrant;
the last two exist to be filtered post-hoc for a distance-and-height
constraint, which is why they are dense in lambda and thin in v0. `mapfine`
is a lambda refinement of `mapsweep`'s interior, not an extension of it: its ranges nest inside
`mapsweep`'s on both axes, so the two never disagree, they only differ in resolution.

`crossproduct` and `nsweep` use a **+/-100** stride-2 fine window; everything built after them
uses **+/-20**. A wider window is wasted where only the optimum is wanted, because the coarse
stride-10 scan already localizes the flick to within five ticks, and across the four later grids
it is the difference between hours and a day. The cost is that the two conventions are not
comparable at the level of the profile *population* -- a +/-100 cell holds up to 101 profiles
against a +/-20 cell's 21 -- so the grids that import cells from the older two say so where they
do it, and every cell's `snap_window.json` states its own `half`.

The three cells at the intersection (`n = 150, 300, 450` at `lambda = 0, v0 = 0`) are in both,
duplicated rather than assigned, so neither directory is missing a row of its own grid. Each
carries its own `cells.tsv`, its own `best.csv`, and a figure per cell under
`runs/atlas/fig/<grid>`.

The v0 grid was filled in by a second run. Combining it with the first is only legitimate
because the two agree set-wise, not merely because each file certifies: `sweep verify` replays a profile
under the physics its *own* header claims, so it would pass just as happily on a corpus built
with a different window or pass budget. What licenses the merge is that every profile in both
carries the same `trig`, `flight`, `jitter`, `rough` and 30-pass stopping time, every
`snap_window.json` in both reads `half=100, step=2, coarse_step=10`, the two binaries share one
blob for `src/`, and the profiles-per-cell distributions match at each n (74.2 vs 74.6 at 150,
99.2 vs 99.0 at 300, 101 vs 101 at 450). The `cells.tsv` label column still records which run
each cell came from.

Splitting is the easy direction: a partition of a set that is uniform in every parameter is
uniform in every parameter.

`n = 600` is deliberately absent: it is well inside the two-cycle-optimal zone, so its best
profile answers a different question than the single-cycle cells around it.

`crossproduct` and `nsweep` carry `commit bf5b9e3` / `20d3304`; `nsweepv0` carries `56d1ca6` on
the 3406 cells it solved and its sources' stamps on the 230 it imported, and `nsweepv0fine`
`56d1ca6` on its own 3333 and `nsweepv0`'s mix on the 1616 it imported. Every profile in all
six grids carries `flight algebraic` and certifies.

The map grids each carry **two** commit stamps, because they were extended after they were
first built: `20d3304` on the original cells and `cd8f19b` on the expansion (mapsweep 80,869 +
40,429; mapfine 104,181 + 34,430). That is a provenance record, not a physics difference --
`git rev-parse` gives the same tree hash for `src/`, `Cargo.lock` and `Cargo.toml` at both
commits, so `git diff 20d3304 cd8f19b -- src/` is empty and every commit between the two is
tools and prose. The expansion was built at its real HEAD rather than forged back to `20d3304`
with `ELYTRASIM_COMMIT_OVERRIDE`: that override exists so a source-only export can state a hash
it cannot prove, and using it to manufacture a uniform-looking corpus would spend exactly the
trust it depends on. One row per cell in each grid's `best.csv` (`tools/snapsweep_best.py`) -- dJ, dy, dz, structure
and the winning profile -- which is the input to any post-hoc constraint filter.

## The hold-0 invariant is sharper than the atlas could see

Across all 207 cells of the original n-sweep that have a cyclic optimum:

```
  hold-0   9   10    11    12    13
  cells    2    1    40   161     3
```

204 of 207 sit in 11-13, and **161 of them are exactly 12**. The atlas reported a band; the
cross product says it is a number with a little noise on it.

Filling in the v0 grid to all sixteen combinations of `{-0.2, 0, 0.2, 0.4}^2` -- doubling the
span of the velocity domain -- does not move it. Over the 235 cross-product cells with a cyclic
optimum the mean hold-0 by component is `vy` 11.49 to 11.60 and `vz` 11.47 to 11.61: a range of
**0.11 and 0.14 ticks**. Sixteen of the nineteen cells outside 11-13 are at `n = 150` with
`lambda < 0` and negative `dJ`, which is to say the invariant weakens exactly where the cycle is
barely worth flying -- the same place the short horizons weakened it.

Nothing moves it. Mean hold-0 by parameter:

```
  n       150: 11.21   225: 12.00   300: 11.96   375: 12.00   450: 12.05
  lambda   -2: 11.73    -1: 11.58     0: 11.82    +1: 11.83    +2: 11.42
  vy      0.0: 11.81   0.2: 11.62
  vz      0.0: 11.80   0.2: 11.67
```

The only systematic deviation is at small `n`, and all three outliers live there: `n=170` and
`n=172` at hold-0 9, and `n=150, lambda=-1, v0=(0.2,0)` at 10. So the invariant does not merely
survive the cross product -- it is untouched by it, and weakens only where the horizon is short
enough to crowd the manoeuvre. It still has no explanation.

## The reference horizon is not the best horizon

Along `lambda = 0, v0 = 0`, the best single-cycle `dJ` rises monotonically to `n = 328` and falls
monotonically after it, smooth to four decimals at every stride-2 step:

```
  n     300      310      320      328      340      350      360
  dJ  19.716   19.977   20.120   20.163   20.084   19.904   19.622
```

**`n = 300` is 0.447 blocks below the optimum at `n = 328`.** The peak is flat -- `n = 320..332`
is all within 0.01 -- so the number to take from this is not "328" but "the reference operating
point sits on the rising limb, and about half a block is available for free by lengthening the
run ~9%". Why 328 is unknown; `hold0 = 12` holds across the entire range, so whatever sets the
peak is not the hold.

## The best horizon depends on where you start, and monotonically

The section above says `n = 328` is the best horizon. That is true at `v0 = (0,0)` and nowhere
else. Across the 36 velocity columns of `nsweepv0`, the `n` that maximizes `dJ` runs from **290
to 344** -- 54 ticks, 2.7 seconds at 20 ticks per second -- and it moves in an orderly way:

```
        vz    -0.2    0.0    0.1    0.2    0.3    0.4
  vy -0.2     322    316    310    304    298    290
  vy +0.0     332    328    322    312    302    294
  vy +0.1     334    332    326    316    306    296
  vy +0.2     338    336    332    322    314    304
  vy +0.3     342    340    336    328    320    314
  vy +0.4     344    344    340    332    328    324
```

**Monotone in both components, with opposite signs**: more initial `vz` shortens the best
horizon, more initial `vy` lengthens it. No cell in the table breaks either ordering. So the
optimal horizon is not a constant with noise on it the way hold-0 is -- it is a smooth function
of the entry velocity, and quoting a single number for it is quoting one cell of this table.

Two things stop the result from being an artifact of the fine window. Every one of the 36
columns peaks in the **interior** of `150 .. 350`, the closest to an edge being `v0 = (0.4,-0.2)`
at 344. And the `vy = 0` row interleaves three separately built grids -- 332, **328**, 322, 312,
302, **294** -- where the two bold entries come from `nsweep` and `mapsweep`, run at different
times under different fine windows. They land exactly where this grid's neighbours predict.

The peak is flat, as it is at `v0 = 0`. 29 of the 36 columns rise monotonically to the peak and
fall monotonically after it; the other 7 reverse somewhere by **at most 0.0026 blocks** against
peak values near 20, which is the resolution of the instrument rather than structure. So the
number to take from a column is "the peak is near here", not the exact tick.

### At 0.05 resolution the dependence is affine

`nsweepv0fine` re-runs the same question on a 0.05 velocity grid over `[0, 0.3]^2` -- 49 columns
where `nsweepv0` has 16 in that box. The argmax table:

```
        vz     0.00   0.05   0.10   0.15   0.20   0.25   0.30
  vy 0.00      328    324    322    316    312    306    302
  vy 0.05      330    326    322    320    314    312    302
  vy 0.10      332    330    326    322    316    310    306
  vy 0.15      334    332    328    326    320    316    310
  vy 0.20      336    334    332    328    322    320    314
  vy 0.25      338    334    334    330    326    322    320
  vy 0.30      340    338    336    332    328    324    320
```

**A plane fits it to 1.63 ticks rms, which is less than one grid step:**

```
  n* = 327.3 + 53.5 * vy - 78.6 * vz          max residual 4.4 ticks, rms 1.63, grid step 2
```

So inside this box the ordering the coarse grid found is not merely monotone, it is *linear* --
the best horizon shifts about 54 ticks per unit of `vy` and 79 the other way per unit of `vz`,
and `vz` is 1.47x the lever `vy` is. That ratio is the content: the two components do not
trade off one-for-one, so the anti-diagonal is not flat but drifts from 328 to 320.

**The plane does not survive being extrapolated, and `nsweepv0` is the grid that says so.**
Its 16 columns inside `[0, 0.3]^2` sit within **3.4 ticks** of the fit, as they should. Its 20
columns outside it miss by up to **20 ticks** -- and not randomly: every column at `vz = -0.2`
falls short of the plane, by 11 ticks at `vy = 0` growing to 20 at `vy = 0.4`. The surface is
affine in this box and bends away from linear as `vz` goes negative, so the coefficients above
are local and quoting them outside the box is an error the corpus can already catch you in.

The finer grid is also a check on the coarser one rather than just a refinement of it, because
the 33 new columns interleave with the 16 old ones in the same table. They interleave without
contradiction: **49 of 49 columns preserve the `vz` ordering and 48 of 49 the `vy` ordering.**
The single exception is `vz = 0.25`, where `vy` 0.05 to 0.10 moves the argmax 312 to 310 --
a one-step reversal worth **0.0011 blocks**, against a peak region that spans 0.078 blocks over
`n = 300 .. 324`. That is the same flat-peak noise the coarse grid showed, not a different
answer.

All 4949 optima are `cyclic`, as in `nsweepv0`.

**`nsweepv0fine` is complete on its own, and 1616 of its 4949 cells are the 16 columns it shares
with `nsweepv0`.** Only the 33 columns off the 0.1 grid were solved -- 3333 cells, 85,338 coarse
and 69,993 fine solves -- and the shared 16 were copied in afterwards, the same duplicate-rather-
than-assign choice made everywhere else here. So the two directories overlap by 1616 cells and
neither is missing a row of its own grid; the `cells.tsv` label column says which is which.
Its own cells all carry `half=20` and `commit 56d1ca6`, so the only non-uniformity it has is the
one it inherited: 107 of the imported cells read `half=100` -- the `v0 = (0,0)` column, which came
from `nsweep`, and six `crossproduct` cells. 4842 of 4949 read `half=20`.

**The cell name could not encode a 0.05 grid before this run.** `snapsweep_grid.py` rendered
velocity in tenths, so 0.05 collided with 0 and 0.15, 0.2 and 0.25 all collided on `02` --
three velocity columns would have shared one output directory, and since the solver resumes by
file existence the sweep would have reported success. Velocity now takes `lam_tag`'s shape for
anything finer than a tenth (0.05 is `00p50`), which resolves a 0.001 grid, and the generator
refuses a grid finer than the name can encode instead of colliding. The 16,614 names already on
disk across the five older grids all regenerate unchanged.

**`nsweepv0` is complete on its own, and 230 of its 3636 cells were imported rather than
solved.** It was *built* with `--exclude`, so the run produced only the 3406 cells no other grid
had: 14 of the 36 columns were missing `n = 150` and `n = 300` (already in `crossproduct`), and
`v0 = (0,0)` and `(0,0.4)` were missing entirely (complete in `nsweep` and `mapsweep`). Read
that way the directory has a hole at `n = 300` in 14 columns -- in the middle of the band where
every peak lives -- so the 230 cells were copied in afterwards, the same duplicate-rather-than-
assign choice made for the three cells shared by `crossproduct` and `nsweep`. The `cells.tsv`
label column names each cell's origin: 3406 `nsweepv0`, 101 `nsweep`, 101 `mapsweep`, 28
`crossproduct`.

**That makes the directory non-uniform, and the non-uniformity is the fine window.** 3507 cells
read `half=20`, and the 129 that came from `nsweep` and `crossproduct` read `half=100`; commit
stamps are `56d1ca6` for the run's own cells and `bf5b9e3`/`20d3304`/`cd8f19b` for the imports.
Everything that is physics is uniform -- `mth_lut`, `algebraic`, `jitter 0`, the same roughness
price, a 30-pass stopping time. So the grid is comparable **at the optimum**, which is what a
`best.csv` argmax asks of it, and **not** at the level of the profile population, where a
+/-100 cell has up to 101 profiles against a +/-20 cell's 21. Each cell's `snap_window.json`
states its own `half`, so a population-level analysis can filter on it rather than trusting the
directory.

All 3636 optima in the grid are `cyclic`; there is no degenerate cell anywhere in it.

## Three smaller things the grid makes visible

**The manoeuvre does not scale with the horizon.** The flick tick as a fraction of `n` drifts
monotonically from 0.653 at `n=150` to 0.740 at `n=438`. A longer run does not stretch the
schedule; it adds glide in front of a manoeuvre that sits progressively later.

**The glide basin shrinks as the horizon grows.** Of ~100 flick seeds per cell, the number
converging to the collapsed glide falls from 41 at `n=150` to 0 by `n=438`, while the cyclic
count rises from 35 to 101. At short horizons the glide swallows most entrances; at long ones it
swallows none.

**Multicycle optima appear only under `lambda = +2`.** 7 cells of 207 hold one, every one of them
at `lambda = +2`, and all at `n >= 300`. No other parameter setting produces one anywhere in the
grid.

## The one degenerate cell

`n0150_lamM2_vy02vz00` has no cyclic optimum at any flick tick. All 16 coarse seeds collapse to
the glide, and they span `dJ -16.630 .. -16.634` -- a spread of 0.004 on a value of 16.6, so the
"winner" beat the runner-up by 5e-7. Its window is placed by rounding noise, which is harmless
here only because there is no manoeuvre to center on. One cell in 208; recorded rather than
handled.

## Reproducing

```
python3 tools/snapsweep_grid.py --ns 150:350:2 --lams 0 --v0s='0,0;0,0.2' \
    --label mygrid --exclude runs/atlas/nsweep/cells.tsv > cells.tsv
RUN=... python3 tools/snapsweep_build.py 1          # coarse seeds + work list
RUN=... tools/snapsweep_submit.sh work/stage1.tsv --wait
RUN=... HALF=20 tools/snapsweep_finish.sh           # stage 2, then delete coarse once complete
tools/snapsweep_pull.sh <remote> <dest>   # tars home, verifies, deletes the remote
tools/snapsweep_figs.sh <celldir> <outdir>
python3 tools/snapsweep_best.py runs/atlas/crossproduct --csv runs/atlas/crossproduct/best.csv
```

This is a recipe for building *a* grid, not a replay of the six above: each grid's own
`cells.tsv` is the authoritative record of what it contains, and `--exclude` is what keeps an
expansion from redoing cells that are already on disk. `HALF` has no default and must be given:
100 keeps the profile population, 20 keeps only the neighbourhood of the optimum at 2.4x fewer
solves, and the two are not comparable, so nothing picks for you. Submit through
`snapsweep_submit.sh` rather
than `sbatch` directly -- it sizes the array to the partition, which a number in the batch
script cannot do without going stale.

`--flight algebraic` is a cluster choice, not a universal one -- see `README-sweep.md`.

The pull deletes the remote run directory as its last act, per sweep rather than per session.
The cluster is compute only; a sweep left there is a second copy of the corpus that nobody is
tracking. Deletion is gated on a per-cell name-and-count comparison -- a total alone would not
catch a truncated transfer, since two different sets can have the same size -- and it refuses,
keeping the remote, if anything does not match. `--keep` opts out for a sweep you mean to
resume. Only *output* is deleted: the toolchain, the source trees and the compiled `target/`
directories all stay, so the next sweep is an rsync and an incremental build rather than a
rustup install and a cold compile. The cluster filesystem is 2.3T at 6% used -- deleting
anything rebuildable buys nothing and costs time later.
