"""Plot the flights `examples/booster.rs flights` writes: the optimum against three gain markers.

    TRIG=mth_lut LIM=89 cargo run --release --example booster -- flights 0.5 3.0 > f.csv
    python3 tools/plot_booster.py f.csv out.png "v_z 60 b/s, v_y 10 b/s"
"""
import csv
import sys
from collections import defaultdict

import matplotlib.pyplot as plt

INK, INK2, GRID, SURFACE = "#1f1f1e", "#5f5e58", "#e4e3dc", "#fcfcfb"
STYLE = {  # fixed categorical order; the optimum is the reference, drawn in ink
    "optimum": dict(color=INK, lw=3.0, label="optimum (max apex)"),
    "dTE n=20": dict(color="#2a78d6", lw=2.0, label="lookahead 20"),
    "dTE n=1": dict(color="#eb6834", lw=2.0, label="lookahead 1"),
    "law K=0.771": dict(color="#1baf7a", lw=2.0, ls=(0, (5, 2)), label="gain law K=0.771"),
}


def main(path, out, title):
    runs = defaultdict(list)
    for r in csv.DictReader(open(path)):
        runs[r["rule"]].append(r)
    col = lambda rows, k: [float(r[k]) for r in rows if r[k] != ""]
    n_opt = len(runs["optimum"])

    fig, axes = plt.subplots(1, 3, figsize=(16, 5.2), facecolor=SURFACE)
    for ax in axes:
        ax.set_facecolor(SURFACE)
        ax.grid(color=GRID, lw=0.8)
        ax.tick_params(colors=INK2, labelsize=9)
        for s in ax.spines.values():
            s.set_visible(False)

    order = ["dTE n=1", "dTE n=20", "law K=0.771", "optimum"]  # optimum drawn last, on top
    for rule in order:
        rows, st = runs[rule], STYLE[rule]
        rows_p = [r for r in rows if r["pitch"] != ""][: n_opt + 5]
        axes[0].plot([int(r["t"]) for r in rows_p], [float(r["pitch"]) for r in rows_p], **st)
        rows_c = rows[: n_opt + 5]
        axes[1].plot(col(rows_c, "t"), col(rows_c, "y"), **st)
        axes[2].plot(col(rows_c, "z"), col(rows_c, "y"), **st)

    axes[0].set(xlabel="tick", ylabel="pitch, deg  (negative is nose up)", title="pitch flown")
    axes[1].set(xlabel="tick", ylabel="height gained, blocks", title="height against time")
    axes[2].set(xlabel="horizontal distance, blocks", ylabel="height gained, blocks",
                title="height against distance")
    for ax in axes:
        ax.title.set_color(INK)
        ax.xaxis.label.set_color(INK2)
        ax.yaxis.label.set_color(INK2)
    axes[0].set_ylim(-95, 5)
    axes[0].legend(frameon=False, fontsize=9, labelcolor=INK)
    fig.suptitle(f"Booster start, {title}: every rule flown closed-loop (vanilla trig, |pitch| ≤ 89)",
                 color=INK, fontsize=12)
    fig.tight_layout()
    fig.savefig(out, dpi=140, facecolor=SURFACE)


if __name__ == "__main__":
    main(*sys.argv[1:4])
