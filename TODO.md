# TODO

josie's todo/notes, don't edit this

- overnight late flick for other cells
- plot overlapping pitch profiles on the energy grid. ref the coloring that elytra-vario uses, which i like a lot better than elytra sim/mains.
- and then plot different cells overlapping, all the profiles for each cell, colored/keyed by which cell they came from. for a fixed v0, and make different charts for varying lambda and varying num_ticks
- what's a nice way to plot dv vs pitch? in game? ehh idk if it's useful
- plot ablations on the same graph as the optimized ones, the coloring distinguishes them and tells us how much optimization buys. maybe restrict snap timing range to declutter? but i kinda like seeing all of them. if there was some way to measure the goodness of each phase independently? when comparing entries, we kinda don't want the gain on the other side to be varying/confounding us. but it's bad to be grading entries on OOD endings. tho maybe not? also i think we can factor out stuff post snap, bc the snap collapses y_vel to 0 for every profile prefix (that's the point) so they only vary along z_vel. maybe an entire research direction is factoring along the snap.
    - model is that we enter with some y and z vel, and exit with 0 y vel and some z vel. and the init/final y completely factor.
    - check that this holds for the profiles from the 30 n soft fit, and then for initial values that are OOD for the optimized profiles but plausible for a wider class of cycle profiles. ooo you can find the region for which this model holds.
    - actually we pitch up before hitting y-vel 0. maybe it's when your delta z vel would become negative. and the start might be the min y vel. but i still expect a nice band.
    - plot on the energy grid for pitch=0, not argmax_pitch delta TE (they're basically the same on the domain we're interested in, but pitch=0 is more principled)
