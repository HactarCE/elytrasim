# Implied lambda from a population of local optima

## Motivation

`mapfine` optimizes many initial pitch schedules in every `(num_ticks, lambda)` cell. The pitch
optimizer can polish a schedule within its local flick-timing basin, but it has difficulty moving
the whole flick earlier or later. Consequently, a profile can be a good local optimum while its
flick timing is poorly matched to the lambda in its header. Treating that stated lambda as a
property of the resulting profile confounds the seed's basin with the objective.

Implied lambda instead asks: **at what lambda does this profile come closest to the best profile
the sampled population knows about?** The population is used as an empirical estimate of the
unknown global optimum. This deliberately measures the macro-coordinate that coordinate
gradients miss.

## Definition

For a fixed horizon `n`, profile `i` has

```text
U_i(lambda) = dte_i + (21.5 / 330) lambda dz_i
```

because the optimized objective is `TE(s_n) + w z_n`, with
`w = lambda (21.5 / 330)`. Once a profile is fixed, its utility is therefore a straight line in
lambda. Its header's stated lambda is not used in this calculation.

The sampled population defines the empirical value function

```text
V_hat_n(lambda) = max_j U_j(lambda).
```

This is the upper envelope of the population's utility lines and is a lower bound on the unknown
true optimum at each lambda. Over the sampled domain `[0, 6]`, profile `i`'s implied lambda is

```text
lambda_hat_i = argmin_lambda [V_hat_n(lambda) - U_i(lambda)].
```

The bracketed quantity is empirical regret. A profile on the envelope is empirically optimal over
an interval; the implementation assigns the midpoint of that interval. A dominated profile is
never the sampled optimum, so its implied lambda is the point where it comes closest to the
envelope. Slopes outside the envelope's sampled range map to the boundary, 0 or 6.

The envelope is convex and piecewise linear, so this construction is exact and continuous; it
does not search only the lambdas in the original sweep. Adjacent envelope segments meet at the
only interior candidates. One useful but initially surprising consequence is that the minimizing
lambda depends on the profile's slope, hence `dz`, while its `dte` intercept determines the amount
of regret. This is geometry of regret against a convex envelope, not an omitted term.

## Why this is population-dependent

The construction estimates a global value function from the local optima that were actually
found. Adding a previously unseen timing basin can raise part of the envelope, move its kinks, and
therefore move implied lambdas. An implied lambda is not a certificate that a profile is a true
global optimum. It is a certificate relative to a named empirical population, and improves as
that population covers the important basins.

The explorer uses **all certified profiles at the same `num_ticks`** to build the envelope, before
applying `dy > 0` and `dz > 150`. This estimates the unconstrained physical optimum rather than a
frontier created by the display filter. Feasible profiles are then grouped by their implied lambda
and ranked by utility at that lambda. The depth control shows the top `k` in every group.

## Sensitivity to the feasibility constraint

As a check, the implied lambda of every feasible profile was recomputed against an envelope made
only from feasible profiles. Of 45,265 feasible profiles over 30 horizons:

- 44,574 (98.47%) had exactly the same implied lambda.
- The median and 95th-percentile absolute movement were both zero.
- 227 profiles (0.50%) moved by at least 0.25 lambda; 154 (0.34%) moved by at least 0.5.
- Mean absolute movement was 0.0071, while the maximum was 3.37.

The large changes are concentrated at the feasibility boundary. Among profiles with
`0 < dy < 0.05`, 63.6% moved and the mean movement was 0.89. No profile with `dy >= 5` moved at
all. At `n = 142` only three profiles are feasible, so its constrained envelope is especially
unstable. Thus the population choice barely affects ordinary feasible profiles, but matters
substantially for marginal profiles that only just clear `dy > 0`.

## Explorer semantics

`tools/plot_mapfine.py` writes `runs/atlas/fig/mapfine/feasible-explorer.html`.

- The depth is the top `k` per implied-lambda group, scored at that implied lambda.
- All four charts show the unconstrained ranking by default; a checkbox switches all four to the
  feasible ranking. Both populations use implied lambdas from the all-profile envelope, so
  switching does not silently change the horizontal coordinate.
- Selecting a point reports its feasibility, stated lambda, implied lambda, empirical regret, and
  source profile.

Regenerate the data and explorer with:

```sh
/Users/josie/.venv-global/bin/python3 tools/plot_mapfine.py
```
