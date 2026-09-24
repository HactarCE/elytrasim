#!/usr/bin/env python3
"""Build an interactive steady-state sweep and pitch-profile explorer.

Usage:
    python3 tools/plot_steady_explorer.py [SWEEP_DIR] [OUT.html]
"""

import csv
import json
import os
import re
import sys


ROOT = sys.argv[1] if len(sys.argv) > 1 else "runs/steady/nlamsweep"
OUT = sys.argv[2] if len(sys.argv) > 2 else "runs/steady/fig/steady-explorer.html"

# The extent of the energy field PNG we borrow from the atlas explorer, in blocks/tick. Keep
# these equal to VX_LO, VX_HI, VY_LO, VY_HI in tools/plot_field_replay.py -- they say where the
# image belongs in velocity space, and the replay view is drawn wider than it.
FIELD_VZ = (-0.5, 3.0)
FIELD_VY = (-1.5, 2.0)


def load_pitch(path):
    values = []
    with open(path) as fh:
        for line in fh:
            if not line.startswith("#"):
                values.extend(float(value) for value in line.split())
    return values


def load_data():
    """Return the sweep as a (lambda x num_ticks) grid, with `null` for a cell that is missing.

    The grid shape is what lets the two selection sliders be independent: a num_ticks index
    means the same n at every lambda, so dragging lambda holds the horizon fixed.
    """
    with open(os.path.join(ROOT, "best.csv"), newline="") as fh:
        rows = list(csv.DictReader(fh))
    cells = []
    for row in rows:
        n = int(row["n"])
        path = os.path.join(ROOT, "out", row["cell"], row["file"])
        cells.append({
            "n": n,
            "lam": float(row["lam"]),
            "vy": float(row["vy"]),
            "vz": float(row["vz"]),
            "dJ": float(row["dJ"]),
            "dy": float(row["dy"]),
            "dz": float(row["dz"]),
            "rate": float(row["dJ"]) / n,
            "structure": row["structure"],
            "file": row["file"],
            # Rounded to 1e-4 deg: the solver's own polish grid is 0.25 deg, so this is far below
            # anything a schedule resolves, and full f64 would make the page 15 MB instead of 7.
            "pitch": [round(value, 4) for value in load_pitch(path)],
        })
    ns = sorted({cell["n"] for cell in cells})
    lams = sorted({cell["lam"] for cell in cells})
    n_at = {n: i for i, n in enumerate(ns)}
    lam_at = {lam: i for i, lam in enumerate(lams)}
    grid = [[None] * len(ns) for _ in lams]
    for cell in cells:
        grid[lam_at[cell["lam"]]][n_at[cell["n"]]] = cell
    return {"ns": ns, "lams": lams, "grid": grid}


DATA = json.dumps(load_data(), separators=(",", ":"))


def load_field():
    path = "runs/atlas/fig/mapfine/feasible-explorer.html"
    with open(path) as fh:
        match = re.search(r'const FIELD = "(data:image/png;base64,[^"]+)";', fh.read())
    if not match:
        raise RuntimeError(f"could not find embedded energy field in {path}")
    return match.group(1)


FIELD = load_field()

HTML = r'''<!doctype html>
<meta charset="utf-8">
<title>steady-state profiles</title>
<div id="steady-explorer">
  <style>
    html, body { margin: 0; background: #14171a; }
    #steady-explorer { box-sizing: border-box; color: #e6e9ed; background: #14171a; font: 14px system-ui, sans-serif; padding: 16px; width: 100%; height: 100vh; overflow: hidden; display: grid; grid-template-columns: minmax(0, 1.25fr) minmax(360px, .9fr); gap: 12px; }
    #steady-explorer * { box-sizing: border-box; }
    #steady-explorer h1 { font-size: 20px; font-weight: 500; margin: 0 0 12px; }
    #steady-explorer .control-stack { margin-bottom: 10px; }
    #steady-explorer .controls { display: grid; grid-template-columns: auto minmax(220px, 1fr) auto; gap: 12px; align-items: center; }
    #steady-explorer .controls.secondary { margin-top: 5px; }
    #steady-explorer .toggle { display: flex; align-items: center; gap: 6px; white-space: nowrap; margin-top: 7px; }
    #steady-explorer input[type="range"] { width: 100%; }
    #steady-explorer .readout { min-width: 480px; text-align: right; font-variant-numeric: tabular-nums; }
    #steady-explorer .left { min-width: 0; min-height: 0; display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; }
    #steady-explorer .right { min-width: 0; min-height: 0; display: grid; grid-template-rows: 1.15fr 1fr 1fr 1fr 1.25fr; gap: 12px; }
    #steady-explorer figure { margin: 0; min-width: 0; min-height: 0; display: grid; grid-template-rows: auto minmax(0, 1fr); }
    #steady-explorer figcaption { margin: 0 0 4px; color: #b9c0c9; }
    #steady-explorer canvas { display: block; width: 100%; height: 100%; min-height: 0; background: #1b1f24; border: 1px solid #4a525c; }
    #steady-explorer .detail { min-height: 20px; margin-top: 10px; color: #b9c0c9; font-variant-numeric: tabular-nums; }
    #steady-explorer .collapsed { color: #ffb4a2; }
    @media (max-width: 900px) {
      #steady-explorer { height: auto; min-height: 0; overflow: visible; display: block; }
      #steady-explorer .controls { grid-template-columns: 1fr; }
      #steady-explorer .readout { min-width: 0; text-align: left; }
      #steady-explorer .left, #steady-explorer .right { display: flex; flex-direction: column; gap: 16px; }
      #steady-explorer figure { display: block; }
      #steady-explorer canvas { height: auto; aspect-ratio: 1.8; }
      #steady-explorer figure.pitch canvas { aspect-ratio: 1.45; }
      #steady-explorer figure.field canvas { aspect-ratio: 1; }
    }
  </style>
  <div class="left">
  <h1>steady-state profiles</h1>
  <div class="control-stack">
    <div class="controls">
      <label for="steady-lam">lambda</label>
      <input id="steady-lam" type="range" min="0" max="0" value="0" step="1">
      <output id="steady-lam-readout" class="readout" aria-live="polite"></output>
    </div>
    <div class="controls secondary">
      <label for="steady-n">num_ticks</label>
      <input id="steady-n" type="range" min="0" max="0" value="0" step="1">
      <output id="steady-readout" class="readout" aria-live="polite"></output>
    </div>
    <div class="controls secondary">
      <label for="steady-tick">tick</label>
      <input id="steady-tick" type="range" min="0" max="350" value="127" step="1">
      <output id="steady-tick-readout" class="readout" aria-live="polite"></output>
    </div>
    <label class="toggle"><input id="steady-feasible" type="checkbox"> show only height-sustaining profiles (dy &gt; 0)</label>
  </div>
    <figure class="field"><figcaption>replay on the one-tick energy field &middot; ring marks the cycle boundary &middot; the painted square is the mapped region; the view is wider because high lambda leaves it</figcaption><canvas id="ss-field" aria-label="velocity replay over the one-tick energy field"></canvas></figure>
  <div id="ss-detail" class="detail"></div>
  </div>
  <div class="right">
    <figure class="pitch"><figcaption>pitch vs time &middot; every winning profile at this lambda faint; selected profile white</figcaption><canvas id="ss-pitch" aria-label="pitch versus tick for steady-state profiles"></canvas></figure>
    <figure class="v0"><figcaption>fixed-point v0 vs num_ticks &middot; rescaled per lambda</figcaption><canvas id="ss-v0" aria-label="fixed-point velocity versus number of ticks"></canvas></figure>
    <figure class="gain"><figcaption>dJ vs num_ticks &middot; rescaled per lambda; dJ is not comparable across lambda</figcaption><canvas id="ss-gain" aria-label="dJ versus number of ticks"></canvas></figure>
    <figure class="rate"><figcaption>dz / num_ticks &middot; mean forward speed &middot; bright where dy &gt; 0 (height-sustaining), dim where the cycle sinks &middot; ring = best sustainable at this lambda, diamond = best over all lambda</figcaption><canvas id="ss-rate" aria-label="dJ per tick versus number of ticks"></canvas></figure>
    <figure class="trade"><figcaption>height vs distance at the selected num_ticks &middot; one point per lambda, color runs with lambda &middot; filled cyclic, hollow collapsed, diamond multicycle &middot; dashed line is dy = 0</figcaption><canvas id="ss-trade" aria-label="height gain versus distance covered, one point per lambda"></canvas></figure>
  </div>
  <script>
  (() => {
    const D = __DATA__;
    const NS = D.ns, LAMS = D.lams, GRID = D.grid;
    const FIELD = "__FIELD__";
    // From opt.rs: w = lambda * Y_REF / Z_REF, the exchange rate in blocks of height per block
    // of distance that lambda normalizes.
    const Y_REF = 21.5, Z_REF = 330.0;
    const FIELD_VZ = __FIELD_VZ__, FIELD_VY = __FIELD_VY__;
    const root = document.querySelector("#steady-explorer");
    const lamSlider = root.querySelector("#steady-lam");
    const slider = root.querySelector("#steady-n");
    const tickSlider = root.querySelector("#steady-tick");
    const lamReadout = root.querySelector("#steady-lam-readout");
    const readout = root.querySelector("#steady-readout");
    const tickReadout = root.querySelector("#steady-tick-readout");
    const detail = root.querySelector("#ss-detail");
    const feasibleBox = root.querySelector("#steady-feasible");
    const fieldImage = new Image(); fieldImage.src = FIELD;
    lamSlider.max = LAMS.length - 1;
    lamSlider.value = Math.max(0, LAMS.indexOf(0));
    slider.max = NS.length - 1;
    slider.value = Math.max(0, NS.indexOf(254));

    const C = {fg:"#e6e9ed", dim:"#b9c0c9", grid:"#5d6774", bg:"#14171a", vy:"#55c1ff", vz:"#db6d28", gain:"#7ee787", rate:"#d2a8ff", tick:"#ffd166"};
    const viridis = [[68,1,84],[72,35,116],[64,67,135],[52,94,141],[41,120,142],[32,144,140],[34,168,132],[68,190,112],[121,209,81],[189,223,38],[253,231,37]];
    function color(t, alpha=1) {
      t=Math.max(0,Math.min(.999,t)); const q=t*(viridis.length-1), i=Math.floor(q), f=q-i;
      const a=viridis[i], b=viridis[Math.min(i+1,viridis.length-1)];
      return `rgba(${a.map((v,j)=>Math.round(v+(b[j]-v)*f)).join(",")},${alpha})`;
    }
    function setup(canvas, margins={l:58,r:14,t:14,b:38}) {
      const rect=canvas.getBoundingClientRect(), dpr=Math.min(devicePixelRatio||1,2);
      canvas.width=Math.round(rect.width*dpr); canvas.height=Math.round(rect.height*dpr);
      const ctx=canvas.getContext("2d"); ctx.setTransform(dpr,0,0,dpr,0,0);
      return {ctx,w:rect.width,h:rect.height,m:margins};
    }
    function padded(values, fraction=.07) {
      let lo=Math.min(...values), hi=Math.max(...values); if(lo===hi){lo-=.5;hi+=.5;}
      const pad=(hi-lo)*fraction; return [lo-pad,hi+pad];
    }
    function fmt(value, digits) { return Math.abs(value)<1e-12 ? "0" : value.toFixed(digits); }
    function ticks(d, step) {
      if(!step) return Array.from({length:5},(_,i)=>d[0]+i*(d[1]-d[0])/4);
      const values=[], direction=d[0]<=d[1]?1:-1, lo=Math.min(...d), hi=Math.max(...d);
      for(let v=Math.ceil(lo/step)*step;v<=hi+step*1e-9;v+=step) values.push(+v.toFixed(10));
      return direction>0?values:values.reverse();
    }
    function axes(s, xd, yd, xl, yl, xdigits=0, ydigits=2, prepaint=null, xstep=null, ystep=null) {
      const {ctx,w,h,m}=s;
      const X=x=>m.l+(x-xd[0])/(xd[1]-xd[0])*(w-m.l-m.r);
      const Y=y=>h-m.b-(y-yd[0])/(yd[1]-yd[0])*(h-m.t-m.b);
      if(prepaint) prepaint({X,Y});
      ctx.font="12px system-ui";
      for(const x of ticks(xd,xstep)) {
        const px=X(x);
        ctx.strokeStyle="rgba(185,192,201,.24)";ctx.lineWidth=1;ctx.beginPath();ctx.moveTo(px,m.t);ctx.lineTo(px,h-m.b);ctx.stroke();
        ctx.fillStyle=C.dim;ctx.textAlign="center";ctx.fillText(fmt(x,xdigits),px,h-m.b+18);
      }
      for(const y of ticks(yd,ystep)) {
        const py=Y(y);
        ctx.strokeStyle="rgba(185,192,201,.24)";ctx.lineWidth=1;
        ctx.beginPath();ctx.moveTo(m.l,py);ctx.lineTo(w-m.r,py);ctx.stroke();
        ctx.fillStyle=C.dim;ctx.textAlign="right";ctx.fillText(fmt(y,ydigits),m.l-7,py+4);
      }
      ctx.fillStyle=C.fg;ctx.textAlign="center";ctx.fillText(xl,(m.l+w-m.r)/2,h-6);
      ctx.save();ctx.translate(14,(m.t+h-m.b)/2);ctx.rotate(-Math.PI/2);ctx.fillText(yl,0,0);ctx.restore();
      return {X,Y};
    }
    function line(ctx, points, stroke, width, alpha=1) {
      ctx.beginPath(); points.forEach(([x,y],i)=>i?ctx.lineTo(x,y):ctx.moveTo(x,y));
      ctx.globalAlpha=alpha;ctx.strokeStyle=stroke;ctx.lineWidth=width;ctx.stroke();ctx.globalAlpha=1;
    }
    function selection(ctx, sc, row, value) {
      const x=sc.X(row.n), y=sc.Y(value);
      ctx.strokeStyle="rgba(230,233,237,.42)";ctx.lineWidth=1;ctx.setLineDash([4,3]);
      ctx.beginPath();ctx.moveTo(x,sc.Y(sc.yd0));ctx.lineTo(x,sc.Y(sc.yd1));ctx.stroke();ctx.setLineDash([]);
      ctx.beginPath();ctx.arc(x,y,4.2,0,Math.PI*2);ctx.fillStyle="#ffffff";ctx.fill();ctx.strokeStyle=C.bg;ctx.lineWidth=1.5;ctx.stroke();
    }
    function replay(row) {
      let vy=row.vy, vz=row.vz; const ys=[vy], zs=[vz];
      for(const pitch of row.pitch) {
        const lean=pitch*Math.PI/180, lookZ=Math.cos(lean), lookHor=Math.abs(lookZ), moveHor=Math.abs(vz), lift=lookZ*lookZ;
        vy += .08*(-1+lift*.75);
        if(lookHor>0) {
          const down=vy<0 ? vy*-.1*lift : 0; vy+=down; vz+=lookZ*down/lookHor;
          if(lean<0) { const up=moveHor*-Math.sin(lean)*.04; vy+=up*3.2; vz-=lookZ*up/lookHor; }
          vz += (lookZ/lookHor*moveHor-vz)*.1;
        }
        vy*=.9800000190734863; vz*=.9900000095367432; ys.push(vy); zs.push(vz);
      }
      return {vy:ys,vz:zs};
    }
    const REPLAYS = GRID.map(slice => slice.map(row => row && replay(row)));

    // Fixed velocity view for every lambda. The embedded field image covers its
    // smaller FIELD_VZ x FIELD_VY source extent inside this wider replay domain.
    const VIEW = {x:[-.5,3.5], y:[-1.5,3]};

    // Per-lambda, because dJ runs from about -38 at lambda -2 to about +1200 at lambda 16 and
    // v0z from 0.17 to 3.1. A shared domain would flatten most slices into a straight line.
    // dz/n is the mean forward speed; derived here rather than shipped, it costs nothing.
    for(const slice of GRID) for(const row of slice) if(row) row.rz=row.dz/row.n;
    // Stats are built twice, once over every cell and once over the height-sustaining subset,
    // so the feasibility toggle rescales each panel instead of merely hiding points. That
    // rescaling is the point: the collapsed branch sinks by up to ~290 blocks and otherwise
    // flattens the whole cyclic family against the top of the dy axis.
    const buildSlices = feas => GRID.map((slice, li) => {
      const rows=slice.filter(r=>r&&(!feas||r.dy>0));
      if(!rows.length)return {rows,empty:true};
      const census={};
      for(const row of rows) census[row.structure]=(census[row.structure]||0)+1;
      return {
        rows,
        v: padded(rows.flatMap(r=>[r.vy,r.vz])),
        gain: padded(rows.map(r=>r.dJ)),
        rate: padded(rows.map(r=>r.rate)),
        gainPeak: rows.reduce((best,row)=>row.dJ>best.dJ?row:best).n,
        speed: padded(rows.map(r=>r.rz)),
        // best mean forward speed among cells that at least break even on height
        bestFeasible: rows.filter(r=>r.dy>0).reduce((b,r)=>!b||r.rz>b.rz?r:b,null),
        census: Object.entries(census).sort((a,b)=>b[1]-a[1]).map(([k,v])=>`${k} ${v}`).join("  ·  "),
      };
    });
    const SLICES = [buildSlices(false), buildSlices(true)];
    let FEAS = false;
    const SLICE_OF = li => SLICES[FEAS?1:0][li];
    const vis = r => !!r && (!FEAS || r.dy>0);

    // max dz/num_ticks subject to dy > 0, over the whole sweep: the fastest forward flight
    // that does not lose height.
    const OPT = SLICES[0].map(s=>s.bestFeasible).filter(Boolean).reduce((b,r)=>!b||r.rz>b.rz?r:b,null);
    function plotSpeed(li, row) {
      const s=setup(root.querySelector("#ss-rate")),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const yd=SLICE_OF(li).speed, rows=SLICE_OF(li).rows;
      const sc=axes(s,[NS[0],NS[NS.length-1]],yd,"num_ticks","dz / num_ticks",0,2);
      sc.yd0=yd[0];sc.yd1=yd[1];
      // Break the line where feasibility flips, so the height-losing stretch reads as excluded
      // rather than as more of the same curve. Runs overlap by a point to stay continuous.
      const runs=[]; let run=[];
      for(const r of rows){
        if(run.length&&(r.dy>0)!==(run[run.length-1].dy>0)){runs.push(run);run=[run[run.length-1]];}
        run.push(r);
      }
      if(run.length)runs.push(run);
      for(const part of runs){
        const live=part[part.length-1].dy>0||part.length>1&&part[1].dy>0;
        line(ctx,part.map(r=>[sc.X(r.n),sc.Y(r.rz)]),C.rate,live?1.8:1.2,live?1:.3);
      }
      const bf=SLICE_OF(li).bestFeasible;
      if(bf){
        ctx.beginPath();ctx.arc(sc.X(bf.n),sc.Y(bf.rz),6,0,Math.PI*2);
        ctx.strokeStyle=C.fg;ctx.lineWidth=1.6;ctx.stroke();
      }
      if(OPT&&LAMS[li]===OPT.lam){
        const x=sc.X(OPT.n),y=sc.Y(OPT.rz);
        ctx.beginPath();ctx.moveTo(x,y-6);ctx.lineTo(x+6,y);ctx.lineTo(x,y+6);ctx.lineTo(x-6,y);
        ctx.closePath();ctx.strokeStyle=C.tick;ctx.lineWidth=2;ctx.stroke();
      }
      selection(ctx,sc,row,row.rz);
    }
    function plotMetric(id, li, row, series, domain, ylabel, digits, guideN=null) {
      const s=setup(root.querySelector("#"+id)), ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const xd=[NS[0],NS[NS.length-1]], yd=domain, rows=SLICE_OF(li).rows;
      const sc=axes(s,xd,yd,"num_ticks",ylabel,0,digits); sc.yd0=yd[0];sc.yd1=yd[1];
      if(guideN!==null){
        const x=sc.X(guideN);ctx.strokeStyle=series[0].color;ctx.globalAlpha=.28;ctx.lineWidth=1;ctx.setLineDash([3,4]);
        ctx.beginPath();ctx.moveTo(x,sc.Y(yd[0]));ctx.lineTo(x,sc.Y(yd[1]));ctx.stroke();ctx.setLineDash([]);ctx.globalAlpha=1;
      }
      for(const spec of series) line(ctx,rows.map(r=>[sc.X(r.n),sc.Y(r[spec.key])]),spec.color,1.6);
      selection(ctx,sc,row,row[series[0].key]);
      if(series.length>1){
        selection(ctx,sc,row,row[series[1].key]);
        let x=s.w-s.m.r-86,y=s.m.t+12;
        for(const spec of series){ctx.strokeStyle=spec.color;ctx.lineWidth=2;ctx.beginPath();ctx.moveTo(x,y-3);ctx.lineTo(x+16,y-3);ctx.stroke();ctx.fillStyle=C.fg;ctx.textAlign="left";ctx.fillText(spec.label,x+22,y);y+=17;}
      }
    }
    // The height-versus-distance trade-off at the selected horizon: one point per lambda.
    // Lambda is the curve's parameter rather than an axis, because the lambda slider already is
    // that axis -- the white ring slides along this curve as you drag it. dy and dz are both in
    // blocks, so unlike dJ they mean the same thing at every lambda, which is what makes a
    // single curve across the whole lambda range honest.
    function plotTrade(li, ni) {
      const s=setup(root.querySelector("#ss-trade"),{l:64,r:14,t:14,b:42}),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const col=[];
      GRID.forEach((slice,i)=>{if(vis(slice[ni]))col.push({row:slice[ni],li:i});});
      if(!col.length)return;
      const xd=padded(col.map(c=>c.row.dz)), yd=padded(col.map(c=>c.row.dy));
      const sc=axes(s,xd,yd,"dz, blocks","dy, blocks",0,1);
      // Height-neutral. Above it the cycle climbs; below it the schedule is selling height.
      if(yd[0]<0&&yd[1]>0){
        const y=sc.Y(0);ctx.strokeStyle="rgba(230,233,237,.30)";ctx.lineWidth=1;ctx.setLineDash([5,4]);
        ctx.beginPath();ctx.moveTo(s.m.l,y);ctx.lineTo(s.w-s.m.r,y);ctx.stroke();ctx.setLineDash([]);
      }
      // Break the line where the regime changes. The cyclic and collapsed branches are different
      // flight modes separated by a real jump, and a chord across the gap would draw trade-offs
      // that no schedule in the sweep achieves.
      const segs=[]; let seg=[];
      for(const c of col){
        if(seg.length&&c.row.structure!==seg[seg.length-1].row.structure){segs.push(seg);seg=[];}
        seg.push(c);
      }
      if(seg.length)segs.push(seg);
      for(const part of segs) if(part.length>1) line(ctx,part.map(c=>[sc.X(c.row.dz),sc.Y(c.row.dy)]),"rgba(185,192,201,.45)",1.4);
      for(const c of col){
        const x=sc.X(c.row.dz),y=sc.Y(c.row.dy),tint=color(c.li/(LAMS.length-1));
        ctx.beginPath();
        if(c.row.structure==="MULTICYCLE"){ctx.moveTo(x,y-4.6);ctx.lineTo(x+4.6,y);ctx.lineTo(x,y+4.6);ctx.lineTo(x-4.6,y);ctx.closePath();}
        else ctx.arc(x,y,3.6,0,Math.PI*2);
        if(c.row.structure==="cyclic"){ctx.fillStyle=tint;ctx.fill();}
        else{ctx.strokeStyle=tint;ctx.lineWidth=1.7;ctx.stroke();}
      }
      ctx.font="12px system-ui";ctx.fillStyle=C.dim;
      const ends=[[col[0],"right"],[col[col.length-1],"left"]];
      for(const [c,align] of ends){
        ctx.textAlign=align;
        ctx.fillText(`lambda ${LAMS[c.li]}`,sc.X(c.row.dz)+(align==="right"?-8:8),sc.Y(c.row.dy)+4);
      }
      const sel=col.find(c=>c.li===li);
      if(sel){
        ctx.beginPath();ctx.arc(sc.X(sel.row.dz),sc.Y(sel.row.dy),7,0,Math.PI*2);
        ctx.strokeStyle="#ffffff";ctx.lineWidth=2;ctx.stroke();
      }
    }
    function plotPitch(li, ni, tick) {
      const s=setup(root.querySelector("#ss-pitch"),{l:62,r:16,t:14,b:42}),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const sc=axes(s,[0,NS[NS.length-1]],[90,-90],"tick","pitch, degrees",0,0);
      const slice=GRID[li], selected=slice[ni];
      slice.forEach((row,i)=>{
        if(!vis(row)||i===ni)return;
        line(ctx,row.pitch.map((p,t)=>[sc.X(t),sc.Y(p)]),color(i/(NS.length-1),.24),.8,.9);
      });
      line(ctx,selected.pitch.map((p,t)=>[sc.X(t),sc.Y(p)]),"#ffffff",2.2);
      ctx.strokeStyle="rgba(230,233,237,.35)";ctx.lineWidth=1;ctx.setLineDash([4,3]);ctx.beginPath();ctx.moveTo(sc.X(selected.n),s.m.t);ctx.lineTo(sc.X(selected.n),s.h-s.m.b);ctx.stroke();ctx.setLineDash([]);
      ctx.strokeStyle=C.tick;ctx.globalAlpha=.42;ctx.lineWidth=1;ctx.setLineDash([3,4]);ctx.beginPath();ctx.moveTo(sc.X(tick),s.m.t);ctx.lineTo(sc.X(tick),s.h-s.m.b);ctx.stroke();ctx.setLineDash([]);ctx.globalAlpha=1;
    }
    function plotField(li, ni, tick) {
      const s=setup(root.querySelector("#ss-field"),{l:58,r:14,t:14,b:42}),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const xd=VIEW.x,yd=VIEW.y;
      const aw=s.w-s.m.l-s.m.r, ah=s.h-s.m.t-s.m.b, ratio=(xd[1]-xd[0])/(yd[1]-yd[0]);
      if(aw/ah>ratio){const d=(aw-ah*ratio)/2;s.m.l+=d;s.m.r+=d;}else{const d=(ah-aw/ratio)/2;s.m.t+=d;s.m.b+=d;}
      const sc=axes(s,xd,yd,"vz, blocks/tick","vy, blocks/tick",1,1,q=>{
        if(!(fieldImage.complete&&fieldImage.naturalWidth))return;
        const x0=q.X(FIELD_VZ[0]),x1=q.X(FIELD_VZ[1]),y0=q.Y(FIELD_VY[1]),y1=q.Y(FIELD_VY[0]);
        ctx.drawImage(fieldImage,x0,y0,x1-x0,y1-y0);
        ctx.strokeStyle="rgba(185,192,201,.22)";ctx.lineWidth=1;ctx.strokeRect(x0,y0,x1-x0,y1-y0);
      },.5,.5);
      ctx.save();ctx.beginPath();ctx.rect(s.m.l,s.m.t,s.w-s.m.l-s.m.r,s.h-s.m.t-s.m.b);ctx.clip();
      const slice=GRID[li], replays=REPLAYS[li], selected=slice[ni];
      slice.forEach((row,i)=>{if(!vis(row)||i===ni)return;const v=replays[i];line(ctx,v.vy.map((y,t)=>[sc.X(v.vz[t]),sc.Y(y)]),color(i/(NS.length-1),.24),.8,.9);});
      slice.forEach((row,i)=>{
        if(!vis(row)||i===ni||tick>=replays[i].vy.length)return;
        const v=replays[i];ctx.beginPath();ctx.arc(sc.X(v.vz[tick]),sc.Y(v.vy[tick]),2.4,0,Math.PI*2);ctx.fillStyle=color(i/(NS.length-1),.42);ctx.fill();
      });
      const v=replays[ni];
      line(ctx,v.vy.map((y,t)=>[sc.X(v.vz[t]),sc.Y(y)]),"#ffffff",2.2);
      ctx.beginPath();ctx.arc(sc.X(selected.vz),sc.Y(selected.vy),5,0,Math.PI*2);ctx.fillStyle=C.bg;ctx.fill();ctx.strokeStyle="#ffffff";ctx.lineWidth=2;ctx.stroke();
      ctx.beginPath();ctx.arc(sc.X(v.vz[tick]),sc.Y(v.vy[tick]),4.5,0,Math.PI*2);ctx.fillStyle=C.tick;ctx.fill();ctx.strokeStyle=C.bg;ctx.lineWidth=1.5;ctx.stroke();
      ctx.restore();
    }
    // A missing cell only happens on a ragged grid; snap the num_ticks slider to the nearest
    // lambda has, rather than blanking the page.
    function nIndex(li, want) {
      const slice=GRID[li];
      if(vis(slice[want]))return want;
      for(let d=1;d<slice.length;d++){
        if(vis(slice[want-d]))return want-d;
        if(vis(slice[want+d]))return want+d;
      }
      return -1;
    }
    // With the filter on, lambda >= 7 has no height-sustaining cell at any horizon, so the
    // lambda slider snaps past those rather than landing on an empty panel.
    function lamIndex(want) {
      if(!SLICE_OF(want).empty)return want;
      for(let d=1;d<LAMS.length;d++){
        if(want-d>=0&&!SLICE_OF(want-d).empty)return want-d;
        if(want+d<LAMS.length&&!SLICE_OF(want+d).empty)return want+d;
      }
      return -1;
    }
    function render(){
      FEAS=feasibleBox.checked;
      const li=lamIndex(+lamSlider.value);
      if(li<0)return;
      if(li!==+lamSlider.value)lamSlider.value=li;
      const ni=nIndex(li,+slider.value);
      if(ni<0)return;
      if(ni!==+slider.value)slider.value=ni;
      const lam=LAMS[li], row=GRID[li][ni], slice=SLICE_OF(li);
      tickSlider.max=row.pitch.length-1;
      if(+tickSlider.value>+tickSlider.max)tickSlider.value=tickSlider.max;
      const tick=+tickSlider.value, replayed=REPLAYS[li][ni];
      const nFeas=SLICES[1][li].rows.length;
      lamReadout.textContent=`lambda = ${lam}  ·  w = ${(lam*Y_REF/Z_REF).toFixed(6)} blocks/block  ·  ${slice.census}  ·  ${nFeas}/${SLICES[0][li].rows.length} sustain height`;
      readout.textContent=`n = ${row.n}  ·  v0 (${row.vy.toFixed(4)}, ${row.vz.toFixed(4)})`;
      tickReadout.textContent=`tick ${tick}  ·  pitch ${row.pitch[tick].toFixed(2)}°  ·  v (${replayed.vy[tick].toFixed(4)}, ${replayed.vz[tick].toFixed(4)})`;
      const flag=row.structure==="cyclic" ? "" : ' class="collapsed"';
      detail.innerHTML=`dJ ${row.dJ.toFixed(6)} blocks  ·  dy ${row.dy.toFixed(3)}  ·  dz ${row.dz.toFixed(3)} blocks  ·  dz / num_ticks ${row.rz.toFixed(4)} blocks/tick  ·  <span${flag}>${row.structure}</span>  ·  ${row.file}`;
      plotPitch(li,ni,tick);
      plotTrade(li,ni);
      plotField(li,ni,tick);
      plotMetric("ss-v0",li,row,[{key:"vy",color:C.vy,label:"v0,y"},{key:"vz",color:C.vz,label:"v0,z"}],slice.v,"blocks/tick",2);
      plotMetric("ss-gain",li,row,[{key:"dJ",color:C.gain}],slice.gain,"blocks",1,slice.gainPeak);
      plotSpeed(li,row);
    }
    feasibleBox.addEventListener("change",render);
    lamSlider.addEventListener("input",render);
    slider.addEventListener("input",render);
    tickSlider.addEventListener("input",render);
    slider.addEventListener("keydown",event=>{if(event.key==="PageUp"||event.key==="PageDown"){event.preventDefault();slider.value=Math.max(0,Math.min(NS.length-1,+slider.value+(event.key==="PageUp"?5:-5)));render();}});
    let timer;new ResizeObserver(()=>{clearTimeout(timer);timer=setTimeout(render,80);}).observe(root);
    fieldImage.onload=render; render();
  })();
  </script>
</div>'''.replace("__DATA__", DATA) \
         .replace("__FIELD_VZ__", json.dumps(list(FIELD_VZ))) \
         .replace("__FIELD_VY__", json.dumps(list(FIELD_VY))) \
         .replace("__FIELD__", FIELD)

os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)
with open(OUT, "w") as fh:
    fh.write(HTML)
print(f"wrote {OUT}")
