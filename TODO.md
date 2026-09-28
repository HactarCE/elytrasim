# TODO

josie's todo/notes, don't edit this, tho you can include it in commits

- plot ablations on the same graph as the optimized ones, the coloring distinguishes them and tells us how much optimization buys. maybe restrict snap timing range to declutter? but i kinda like seeing all of them. if there was some way to measure the goodness of each phase independently? when comparing entries, we kinda don't want the gain on the other side to be varying/confounding us. but it's bad to be grading entries on OOD endings. tho maybe not? also i think we can factor out stuff post snap, bc the snap collapses y_vel to 0 for every profile prefix (that's the point) so they only vary along z_vel. maybe an entire research direction is factoring along the snap.
    - model is that we enter with some y and z vel, and exit with 0 y vel and some z vel. and the init/final y completely factor.
    - check that this holds for the profiles from the 30 n soft fit, and then for initial values that are OOD for the optimized profiles but plausible for a wider class of cycle profiles. ooo you can find the region for which this model holds.
    - actually we pitch up before hitting y-vel 0. maybe it's when your delta z vel would become negative. and the start might be the min y vel. but i still expect a nice band.
    - plot on the energy grid for pitch=0, not argmax_pitch delta TE (they're basically the same on the domain we're interested in, but pitch=0 is more principled)
- symbolic derivative / autograd
- find profiles st delta y 0
    - like optimize for time or distance with the constraint that they gain > 0
    - but there's two other axes: for lambda and num_ticks
- min num_ticks st distance > ... and delta y > ...
- do analysis from v0 = right after sprint jump, or maybe after ehop
- natural thing to tas is luna's min-time floor to build height
- plot glide angle against pitch
- what's a nice way to plot dv vs pitch? in game? ehh idk if it's useful
- luna's request: **max distance given init vel and distance from floor**
- on the flicksoft_dive, the right cut is chosen at the point where z_vel *actually* starts falling. but this is bc this is when the flick up starts. the question is whether the optimizer chose to start the flick up *because* holding 0 for another tick would cause you to lose z_vel anyway. (actually, i expect you would keep gaining z-vel for a few more ticks, but this trades off something else, so it ends up not worth it)
- make a decision tree that selects between myopic metrics / try to compress the cycle
- mechanistic explanations
    - entry: unknown, pitch down to get -y-vel
    - dive angle: known, pitch st y-vel dir doesn't change, y-vel falls linearly
    - dive end timing: unknown
    - dive end pitch up: unknown, maybe this isn't good, it's just indifferent
    - dive end pitch down: known, sometimes skipped, we're about to convert -y to z (this interpretation is from different initializations for n=300, validate on actually optimal profiles for different n)
    - roll up to 0: unknown, associated with the dive end pitch down
    - hold 0: known, this is the best way to convert -y to z
    - hold 0 end timing: current question
- have a population of profiles, maximize the min distance between them under the constraint that they must get > 20 gain.
- on the energy grid, plot the ridge contours. some direction dotted with the gradient is 0?
- main: update to the new color scheme + grid lines
- main: distinguish between the way we're obtaining the pitch for each v0, and the arrow that we're drawing (pitch vs delta vel)
- luna: "when moving up, looking down doesn't give z-vel"
- find full set of near-optimal profiles
    - obtain optimum, add noise, filter by utility threshold
- we do a dense .25 stride refit every 4. the stride should be larger.  and we don't know if every 4 is good. we generally can make this faster. how expensive is the dense refit? how often does it choose something else? does it only choose something else at early epochs?
- gain-efficiency: max gain/num_ticks, for steady state
- sweep λDPE + (1-λ)DTE (use something other than λ)
- gain phase myopic-metric
- initial snap down switching time myopic-metric. snap down until you hit some y-vel? maybe similar to the hold-0 switching time? where you are going to convert y-vel to z-vel
- optimize optimizer
    - A ramp that should start a tick later, or bend, has to move many ticks together, so a one-pitch-at-a-time search advances it by slivers.
- how to do a pump over a min barrier st you exit as fast as possible
- use clap
- gain markers for when we have a huge about of vel for monster. this is ood for the lookahead 20 marker.

## turning experiments

- for a fixed pitch, how does moving yaw affect vel/energy?
- for various turning speeds, for a fixed pitch, what is the steady-state vel?
- for a given num_ticks and yaw_final and y_vel and z_vel, maximize x_vel, energy with vel projected onto xy
- for a given num_ticks and vel, plot pitch, yaw, energy after holding that rot for num_ticks
- energy grid, but for a fixed/const/uniform input yaw

## aset

check out the floor constraint optimizer, tho this is more about speedrunning maps, so min-time.

grad ascent is good bc it's fast and has exact physics, but it's local.

dp is good bc it's global and can find the global optimum, but has inexact physics. i also think it is more robust under collision, where reward is highly discontinuous for paths that touch something bc they get killed (even physically realistic relaxations where just their vel gets killed give discontinuous reward). (maybe we do some RL optimizer other than straightforward gradient descent, TODO).

btw for speedrunning we'll have a human run as a seed.

i want to alternate between gradient ascent (or just any local policy with non-interpolated physics) and a local dp bubble around it.

for min-time, i think we should be able to eliminate time from the dp. like we need to do some backwards steps, but i kinda expect we don't need to do on the order of the length of the map.