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


ROOT = sys.argv[1] if len(sys.argv) > 1 else "runs/steady/nsweep"
OUT = sys.argv[2] if len(sys.argv) > 2 else "runs/steady/fig/steady-explorer.html"


def load_pitch(path):
    values = []
    with open(path) as fh:
        for line in fh:
            if not line.startswith("#"):
                values.extend(float(value) for value in line.split())
    return values


def load_data():
    with open(os.path.join(ROOT, "best.csv"), newline="") as fh:
        rows = list(csv.DictReader(fh))
    data = []
    for row in rows:
        n = int(row["n"])
        path = os.path.join(ROOT, "out", row["cell"], row["file"])
        data.append({
            "n": n,
            "vy": float(row["vy"]),
            "vz": float(row["vz"]),
            "dJ": float(row["dJ"]),
            "rate": float(row["dJ"]) / n,
            "file": row["file"],
            "pitch": load_pitch(path),
        })
    return sorted(data, key=lambda row: row["n"])


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
    #steady-explorer { box-sizing: border-box; color: #e6e9ed; background: #14171a; font: 14px system-ui, sans-serif; padding: 16px; width: 100%; height: 100vh; overflow: hidden; display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; }
    #steady-explorer * { box-sizing: border-box; }
    #steady-explorer h1 { font-size: 20px; font-weight: 500; margin: 0 0 12px; }
    #steady-explorer .control-stack { margin-bottom: 10px; }
    #steady-explorer .controls { display: grid; grid-template-columns: auto minmax(220px, 1fr) auto; gap: 12px; align-items: center; }
    #steady-explorer .controls.secondary { margin-top: 5px; }
    #steady-explorer input[type="range"] { width: 100%; }
    #steady-explorer .readout { min-width: 480px; text-align: right; font-variant-numeric: tabular-nums; }
    #steady-explorer .grid { min-height: 0; display: grid; grid-template-columns: minmax(0, 1.25fr) minmax(360px, .9fr); grid-template-rows: 1.2fr repeat(3, minmax(0, 1fr)); grid-template-areas: "field pitch" "field v0" "field gain" "field rate"; gap: 12px; }
    #steady-explorer figure { margin: 0; min-width: 0; min-height: 0; display: grid; grid-template-rows: auto minmax(0, 1fr); }
    #steady-explorer figure.pitch { grid-area: pitch; }
    #steady-explorer figure.field { grid-area: field; }
    #steady-explorer figure.v0 { grid-area: v0; }
    #steady-explorer figure.gain { grid-area: gain; }
    #steady-explorer figure.rate { grid-area: rate; }
    #steady-explorer figcaption { margin: 0 0 4px; color: #b9c0c9; }
    #steady-explorer canvas { display: block; width: 100%; height: 100%; min-height: 0; background: #1b1f24; border: 1px solid #4a525c; }
    #steady-explorer .detail { min-height: 20px; margin-top: 10px; color: #b9c0c9; font-variant-numeric: tabular-nums; }
    @media (max-width: 900px) {
      #steady-explorer { height: auto; min-height: 0; overflow: visible; display: block; }
      #steady-explorer .controls { grid-template-columns: 1fr; }
      #steady-explorer .readout { min-width: 0; text-align: left; }
      #steady-explorer .grid { display: flex; flex-direction: column; gap: 16px; }
      #steady-explorer figure, #steady-explorer figure.pitch, #steady-explorer figure.field, #steady-explorer figure.v0, #steady-explorer figure.gain, #steady-explorer figure.rate { grid-area: auto; display: block; }
      #steady-explorer canvas { height: auto; aspect-ratio: 1.8; }
      #steady-explorer figure.pitch canvas { aspect-ratio: 1.45; }
      #steady-explorer figure.field canvas { aspect-ratio: 1; }
    }
  </style>
  <h1>steady-state profiles</h1>
  <div class="control-stack">
    <div class="controls">
      <label for="steady-n">num_ticks</label>
      <input id="steady-n" type="range" min="0" max="0" value="0" step="1">
      <output id="steady-readout" class="readout" aria-live="polite"></output>
    </div>
    <div class="controls secondary">
      <label for="steady-tick">tick</label>
      <input id="steady-tick" type="range" min="0" max="350" value="127" step="1">
      <output id="steady-tick-readout" class="readout" aria-live="polite"></output>
    </div>
  </div>
  <div class="grid">
    <figure class="pitch"><figcaption>pitch vs time · every winning profile faint; selected profile white</figcaption><canvas id="ss-pitch" aria-label="pitch versus tick for steady-state profiles"></canvas></figure>
    <figure class="field"><figcaption>replay on the one-tick energy field · ring marks the cycle boundary</figcaption><canvas id="ss-field" aria-label="velocity replay over the one-tick energy field"></canvas></figure>
    <figure class="v0"><figcaption>fixed-point v0 vs num_ticks</figcaption><canvas id="ss-v0" aria-label="fixed-point velocity versus number of ticks"></canvas></figure>
    <figure class="gain"><figcaption>dJ vs num_ticks</figcaption><canvas id="ss-gain" aria-label="dJ versus number of ticks"></canvas></figure>
    <figure class="rate"><figcaption>dJ / num_ticks</figcaption><canvas id="ss-rate" aria-label="dJ per tick versus number of ticks"></canvas></figure>
  </div>
  <div id="ss-detail" class="detail"></div>
  <script>
  (() => {
    const DATA = __DATA__;
    const FIELD = "__FIELD__";
    const root = document.querySelector("#steady-explorer");
    const slider = root.querySelector("#steady-n");
    const tickSlider = root.querySelector("#steady-tick");
    const readout = root.querySelector("#steady-readout");
    const tickReadout = root.querySelector("#steady-tick-readout");
    const detail = root.querySelector("#ss-detail");
    const fieldImage = new Image(); fieldImage.src = FIELD;
    slider.max = DATA.length - 1;
    slider.value = Math.max(0, DATA.findIndex(row => row.n === 254));

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
    function axes(s, xd, yd, xl, yl, xdigits=0, ydigits=2, prepaint=null) {
      const {ctx,w,h,m}=s;
      const X=x=>m.l+(x-xd[0])/(xd[1]-xd[0])*(w-m.l-m.r);
      const Y=y=>h-m.b-(y-yd[0])/(yd[1]-yd[0])*(h-m.t-m.b);
      if(prepaint) prepaint({X,Y});
      ctx.font="12px system-ui";
      for(let i=0;i<5;i++){
        const x=xd[0]+i*(xd[1]-xd[0])/4, px=X(x);
        ctx.strokeStyle="rgba(185,192,201,.24)";ctx.lineWidth=1;ctx.beginPath();ctx.moveTo(px,m.t);ctx.lineTo(px,h-m.b);ctx.stroke();
        ctx.fillStyle=C.dim;ctx.textAlign="center";ctx.fillText(fmt(x,xdigits),px,h-m.b+18);
        const y=yd[0]+i*(yd[1]-yd[0])/4, py=Y(y);
        ctx.beginPath();ctx.moveTo(m.l,py);ctx.lineTo(w-m.r,py);ctx.stroke();
        ctx.textAlign="right";ctx.fillText(fmt(y,ydigits),m.l-7,py+4);
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
    function plotMetric(id, row, series, domain, ylabel, digits, guideN=null) {
      const s=setup(root.querySelector("#"+id)), ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const xd=[DATA[0].n,DATA[DATA.length-1].n], yd=domain;
      const sc=axes(s,xd,yd,"num_ticks",ylabel,0,digits); sc.yd0=yd[0];sc.yd1=yd[1];
      if(guideN!==null){
        const x=sc.X(guideN);ctx.strokeStyle=series[0].color;ctx.globalAlpha=.28;ctx.lineWidth=1;ctx.setLineDash([3,4]);
        ctx.beginPath();ctx.moveTo(x,sc.Y(yd[0]));ctx.lineTo(x,sc.Y(yd[1]));ctx.stroke();ctx.setLineDash([]);ctx.globalAlpha=1;
      }
      for(const spec of series) line(ctx,DATA.map(r=>[sc.X(r.n),sc.Y(r[spec.key])]),spec.color,1.6);
      selection(ctx,sc,row,row[series[0].key]);
      if(series.length>1){
        selection(ctx,sc,row,row[series[1].key]);
        let x=s.w-s.m.r-86,y=s.m.t+12;
        for(const spec of series){ctx.strokeStyle=spec.color;ctx.lineWidth=2;ctx.beginPath();ctx.moveTo(x,y-3);ctx.lineTo(x+16,y-3);ctx.stroke();ctx.fillStyle=C.fg;ctx.textAlign="left";ctx.fillText(spec.label,x+22,y);y+=17;}
      }
    }
    function plotPitch(selected, tick) {
      const s=setup(root.querySelector("#ss-pitch"),{l:62,r:16,t:14,b:42}),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const sc=axes(s,[0,DATA[DATA.length-1].n],[90,-90],"tick","pitch, degrees",0,0);
      DATA.forEach((row,i)=>{
        if(row===selected)return;
        line(ctx,row.pitch.map((p,t)=>[sc.X(t),sc.Y(p)]),color(i/(DATA.length-1),.24),.8,.9);
      });
      line(ctx,selected.pitch.map((p,t)=>[sc.X(t),sc.Y(p)]),"#ffffff",2.2);
      ctx.strokeStyle="rgba(230,233,237,.35)";ctx.lineWidth=1;ctx.setLineDash([4,3]);ctx.beginPath();ctx.moveTo(sc.X(selected.n),s.m.t);ctx.lineTo(sc.X(selected.n),s.h-s.m.b);ctx.stroke();ctx.setLineDash([]);
      ctx.strokeStyle=C.tick;ctx.globalAlpha=.42;ctx.lineWidth=1;ctx.setLineDash([3,4]);ctx.beginPath();ctx.moveTo(sc.X(tick),s.m.t);ctx.lineTo(sc.X(tick),s.h-s.m.b);ctx.stroke();ctx.setLineDash([]);ctx.globalAlpha=1;
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
    const replays=DATA.map(replay);
    function plotField(selected, tick) {
      const s=setup(root.querySelector("#ss-field"),{l:58,r:14,t:14,b:42}),ctx=s.ctx;ctx.clearRect(0,0,s.w,s.h);
      const aw=s.w-s.m.l-s.m.r, ah=s.h-s.m.t-s.m.b;
      if(aw>ah){const d=(aw-ah)/2;s.m.l+=d;s.m.r+=d;}else{const d=(ah-aw)/2;s.m.t+=d;s.m.b+=d;}
      const xd=[-.5,3],yd=[-1.5,2];
      const sc=axes(s,xd,yd,"vz, blocks/tick","vy, blocks/tick",1,1,q=>{
        if(fieldImage.complete&&fieldImage.naturalWidth)ctx.drawImage(fieldImage,q.X(xd[0]),q.Y(yd[1]),q.X(xd[1])-q.X(xd[0]),q.Y(yd[0])-q.Y(yd[1]));
      });
      DATA.forEach((row,i)=>{if(row===selected)return;const v=replays[i];line(ctx,v.vy.map((y,t)=>[sc.X(v.vz[t]),sc.Y(y)]),color(i/(DATA.length-1),.24),.8,.9);});
      DATA.forEach((row,i)=>{
        if(row===selected||tick>=replays[i].vy.length)return;
        const v=replays[i];ctx.beginPath();ctx.arc(sc.X(v.vz[tick]),sc.Y(v.vy[tick]),2.4,0,Math.PI*2);ctx.fillStyle=color(i/(DATA.length-1),.42);ctx.fill();
      });
      const i=DATA.indexOf(selected),v=replays[i];
      line(ctx,v.vy.map((y,t)=>[sc.X(v.vz[t]),sc.Y(y)]),"#ffffff",2.2);
      ctx.beginPath();ctx.arc(sc.X(selected.vz),sc.Y(selected.vy),5,0,Math.PI*2);ctx.fillStyle=C.bg;ctx.fill();ctx.strokeStyle="#ffffff";ctx.lineWidth=2;ctx.stroke();
      ctx.beginPath();ctx.arc(sc.X(v.vz[tick]),sc.Y(v.vy[tick]),4.5,0,Math.PI*2);ctx.fillStyle=C.tick;ctx.fill();ctx.strokeStyle=C.bg;ctx.lineWidth=1.5;ctx.stroke();
    }
    const vDomain=padded(DATA.flatMap(r=>[r.vy,r.vz]));
    const gainDomain=padded(DATA.map(r=>r.dJ));
    const rateDomain=padded(DATA.map(r=>r.rate));
    const gainPeak=DATA.reduce((best,row)=>row.dJ>best.dJ?row:best).n;
    const ratePeak=DATA.reduce((best,row)=>row.rate>best.rate?row:best).n;
    function render(){
      const row=DATA[+slider.value];
      tickSlider.max=row.pitch.length-1;
      if(+tickSlider.value>+tickSlider.max)tickSlider.value=tickSlider.max;
      const tick=+tickSlider.value, replayed=replays[+slider.value];
      readout.textContent=`n = ${row.n}  ·  v0 (${row.vy.toFixed(4)}, ${row.vz.toFixed(4)})`;
      tickReadout.textContent=`tick ${tick}  ·  pitch ${row.pitch[tick].toFixed(2)}°  ·  v (${replayed.vy[tick].toFixed(4)}, ${replayed.vz[tick].toFixed(4)})`;
      detail.textContent=`dJ ${row.dJ.toFixed(6)} blocks  ·  dJ / num_ticks ${row.rate.toFixed(6)} blocks/tick  ·  ${row.file}`;
      plotPitch(row,tick);
      plotField(row,tick);
      plotMetric("ss-v0",row,[{key:"vy",color:C.vy,label:"v0,y"},{key:"vz",color:C.vz,label:"v0,z"}],vDomain,"blocks/tick",2);
      plotMetric("ss-gain",row,[{key:"dJ",color:C.gain}],gainDomain,"blocks",1,gainPeak);
      plotMetric("ss-rate",row,[{key:"rate",color:C.rate}],rateDomain,"blocks/tick",3,ratePeak);
    }
    slider.addEventListener("input",render);
    tickSlider.addEventListener("input",render);
    slider.addEventListener("keydown",event=>{if(event.key==="PageUp"||event.key==="PageDown"){event.preventDefault();slider.value=Math.max(0,Math.min(DATA.length-1,+slider.value+(event.key==="PageUp"?5:-5)));render();}});
    let timer;new ResizeObserver(()=>{clearTimeout(timer);timer=setTimeout(render,80);}).observe(root);
    fieldImage.onload=render; render();
  })();
  </script>
</div>'''.replace("__DATA__", DATA).replace("__FIELD__", FIELD)

os.makedirs(os.path.dirname(OUT) or ".", exist_ok=True)
with open(OUT, "w") as fh:
    fh.write(HTML)
print(f"wrote {OUT}")
