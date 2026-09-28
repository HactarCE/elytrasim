#!/usr/bin/env python3
"""Build the self-contained booster flight explorer from the cell CSVs."""

import base64
import csv
import gzip
import io
import json
import math
import re
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
CELLS = ROOT / "runs/booster/explorer/cells"
OUT = ROOT / "runs/booster/explorer/booster-explorer.html"
SPEEDS = [round(i / 5, 1) for i in range(1, 26)]
# Approach pitches, Minecraft convention (negative is nose up): one downward, 5-degree steps up
# to straight up, and 1-degree steps around the best.
PITCHES = sorted(set([5, *range(0, -91, -5), *range(-29, -20)]))
RULES = ("optimum", "dTE n=20", "law K=0.771")
COLUMNS = ("pitch", "y", "z", "vy", "vz")
HEADER = re.compile(r"# speed ([-+\d.]+) pitch (-?\d+) apex ([-+\d.]+)")


def read_cells():
    data, missing, anomalies = [], [], []
    for speed in SPEEDS:
        group = []
        for pitch in PITCHES:
            path = CELLS / f"v{speed:.1f}_p{pitch}.csv"
            if not path.exists():
                missing.append(path.name)
                continue
            with path.open(newline="") as stream:
                header = stream.readline().strip()
                match = HEADER.match(header)
                if not match or abs(float(match[1]) - speed) > 1e-8 or int(match[2]) != pitch:
                    raise ValueError(f"unexpected header in {path}: {header}")
                rows = list(csv.DictReader(stream))
            runs = []
            for rule in RULES:
                series = [row for row in rows if row["rule"] == rule]
                if not series or [int(r["t"]) for r in series] != list(range(len(series))):
                    raise ValueError(f"missing or nonconsecutive {rule} rows: {path}")
                if series[-1]["pitch"]:
                    raise ValueError(f"final pitch must be empty: {path} / {rule}")
                peak = max(range(len(series)), key=lambda i: float(series[i]["y"]))
                series = series[:peak + 1]
                summary = [round(float(series[peak][key]), 4) for key in ("y", "z", "vz")]
                summary.insert(1, peak)
                cols = [[None if row[key] == "" else round(float(row[key]), 4)
                         for row in series] for key in COLUMNS]
                runs.append([summary, cols])
            # The header can report a negative terminal height for a descending
            # launch. The brief defines apex as the maximum state height, which
            # includes the launch at y=0, so use the trajectory throughout.
            for j in (1, 2):
                loss = runs[0][0][0] - runs[j][0][0]
                if loss < -0.05:
                    anomalies.append((speed, pitch, RULES[j], round(-loss, 4)))
            group.append([pitch, runs])
        data.append(group)
    return data, missing, anomalies


def refined_bests(data):
    result = []
    for speed, group in zip(SPEEDS, data):
        by_pitch = {a: runs[0][0][0] for a, runs in group}
        if not by_pitch:
            result.append(None)
            continue
        best = max(by_pitch, key=by_pitch.get)
        refined = float(best)
        pitches = sorted(by_pitch)
        i = pitches.index(best)
        if 0 < i < len(pitches) - 1:
            x0, x1, x2 = pitches[i - 1:i + 2]
            y0, y1, y2 = (by_pitch[x] for x in (x0, x1, x2))
            d0 = (y1 - y0) / (x1 - x0)
            d1 = (y2 - y1) / (x2 - x1)
            a = (d1 - d0) / (x2 - x0)
            if a < 0:
                vertex = (x0 + x1 - d0 / a) / 2
                if x0 <= vertex <= x2:
                    refined = vertex
        near = [a for a, h in by_pitch.items() if h >= by_pitch[best] - 1]
        apex_run = next(runs[0][0] for a, runs in group if a == best)
        result.append([round(refined, 3), min(near), max(near), best, *apex_run])
    return result


# The energy field's extent, blocks/tick. Launches here have v_z in [0, 5] and v_y in [-0.5, 6.3].
FIELD_VZ = (-1, 6)
FIELD_VY = (-1, 7)


def energy_field():
    # Same whole-degree sweep and sRGB ramp as myopic-metrics/tools/plot_field_replay.py.
    (vxlo, vxhi), (vylo, vyhi) = FIELD_VZ, FIELD_VY
    width, height = 70 * (vxhi - vxlo), 70 * (vyhi - vylo)
    vz, vy = np.meshgrid(np.linspace(vxlo, vxhi, width),
                         np.linspace(vyhi, vylo, height))
    best = np.full(vy.shape, -np.inf)
    for pitch in range(-89, 90):
        lean = math.radians(pitch)
        look_z = math.cos(lean)
        lift = look_z * look_z
        move_hor = np.abs(vz)
        yy = vy + .08 * (-1 + lift * .75)
        zz = vz
        down = np.where(yy < 0, yy * -.1 * lift, 0.0)
        yy = yy + down
        zz = zz + down  # look_z / abs(look_z) is 1 for this pitch range
        if lean < 0:
            up = move_hor * -math.sin(lean) * .04
            yy = yy + up * 3.2
            zz = zz - up
        zz = zz + (move_hor - zz) * .1
        yy = yy * .9800000190734863
        zz = zz * .9900000095367432
        gain = (yy * yy + zz * zz - vy * vy - vz * vz) * .5 / .08 + yy
        np.maximum(best, gain, out=best)
    zero = np.array([12, 13, 16], dtype=float)
    gain_color = np.array([158, 54, 146], dtype=float)
    loss_color = np.array([74, 112, 168], dtype=float)
    ends = np.where(best[..., None] < 0, loss_color, gain_color)
    amount = (np.abs(best) / (np.abs(best) + .4))[..., None]
    pixels = np.uint8(np.round(zero + (ends - zero) * amount))
    buffer = io.BytesIO()
    Image.fromarray(pixels).save(buffer, format="PNG", optimize=True)
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")


HTML = r'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Booster explorer</title>
<style>
:root{color-scheme:dark;--bg:#14171a;--panel:#1b1f24;--edge:#4a525c;--ink:#e6e9ed;--muted:#b9c0c9;--cyan:#54c7e8;--orange:#f5ae68;--pink:#e67dbc}
*{box-sizing:border-box}html,body{margin:0;background:var(--bg);color:var(--ink);font:13px system-ui,sans-serif}
main{max-width:1800px;margin:auto;padding:14px 18px 24px}h1{font-size:21px;font-weight:550;margin:0 0 3px}header p{color:var(--muted);margin:0 0 12px}
.controls{display:grid;grid-template-columns:auto minmax(180px,1fr) auto auto auto;gap:8px 14px;align-items:center;margin:0 0 10px}
input[type=range]{width:100%;accent-color:var(--cyan)}label{white-space:nowrap}.readout{min-width:100px;font-variant-numeric:tabular-nums;color:var(--ink)}.toggle{display:flex;gap:5px;align-items:center;color:var(--muted)}
.detail{min-height:27px;color:var(--ink);font-variant-numeric:tabular-nums;line-height:1.5}.note{color:var(--muted);margin:4px 0 12px;font-size:12px}
section{margin-top:9px}h2{font-size:13px;font-weight:600;color:var(--muted);letter-spacing:.04em;text-transform:uppercase;margin:10px 0 7px}
.grid{display:grid;gap:10px}.profiles{grid-template-columns:repeat(3,minmax(0,1fr));grid-template-rows:repeat(3,220px)}.summaries{grid-template-columns:repeat(3,minmax(0,1fr));grid-template-rows:210px}.heatmaps{grid-template-columns:repeat(2,minmax(0,1fr));grid-template-rows:250px}
figure{min-width:0;min-height:0;margin:0;display:grid;grid-template-rows:auto minmax(0,1fr)}figure.field{grid-row:span 2}figcaption{color:var(--muted);padding:0 0 4px}canvas{display:block;background:var(--panel);border:1px solid var(--edge);width:100%;height:100%;min-height:0}canvas.selectable{cursor:crosshair}
.legend{display:flex;gap:16px;align-items:center;flex-wrap:wrap;color:var(--muted);font-size:12px;margin:5px 0 0}.swatch{display:inline-block;width:16px;border-top:2px solid;vertical-align:middle;margin-right:5px}.ramp{display:inline-block;width:75px;height:7px;vertical-align:middle;background:linear-gradient(90deg,#442154,#2a788e,#7ad151,#fde725);margin:0 4px}
@media(max-width:1150px){.profiles,.summaries{grid-template-columns:repeat(2,minmax(0,1fr));grid-template-rows:none;grid-auto-rows:230px}.profiles .field{grid-row:span 2}.heatmaps{grid-auto-rows:240px}}
@media(max-width:690px){main{padding:10px}.controls{grid-template-columns:1fr auto}.controls>label:first-child{grid-column:1/-1}.controls input[type=range]{grid-column:1/-1}.controls .toggle{grid-column:1/-1}.profiles,.summaries,.heatmaps{display:block}.profiles figure,.summaries figure,.heatmaps figure{height:230px;margin-bottom:12px}.profiles .field{height:360px}.detail{min-height:70px}}
</style></head><body><main>
<header><h1>Booster explorer</h1><p>Elytra flights from a fixed-speed boost · yaw pinned · 20 ticks/s · every angle is Minecraft pitch, negative is nose up; the approach pitch is the look the booster reads</p></header>
<div class="controls"><label for="speed">Booster |v|</label><input id="speed" type="range" min="0.2" max="5" step="0.2" value="3"><output id="speed-value" class="readout"></output><label class="toggle"><input id="show-dte" type="checkbox" checked> dTE n=20</label><label class="toggle"><input id="show-law" type="checkbox" checked> law K=0.771</label><label class="toggle"><input id="follow-best" type="checkbox" checked> follow best apex</label></div>
<div id="detail" class="detail" aria-live="polite"></div><div id="missing" class="note"></div>
<div class="legend"><span><span class="ramp"></span>approach pitch, 5° to −90°</span><span><span class="swatch" style="color:#fff"></span>highlighted: best-apex approach, or the one clicked</span><span><span class="swatch" style="color:var(--orange)"></span>dTE n=20</span><span><span class="swatch" style="color:var(--pink)"></span>law K=0.771</span></div>
<section><h2>Flights at the selected booster speed</h2><div class="grid profiles">
<figure class="field"><figcaption>One-tick energy field · velocity trajectory</figcaption><canvas id="field" class="selectable"></canvas></figure>
<figure><figcaption>Pitch schedule</figcaption><canvas id="pitch" class="selectable"></canvas></figure>
<figure><figcaption>Height vs tick</figcaption><canvas id="height" class="selectable"></canvas></figure>
<figure><figcaption>Flight path</figcaption><canvas id="path" class="selectable"></canvas></figure>
<figure><figcaption>Apex vs approach pitch</figcaption><canvas id="apex" class="selectable"></canvas></figure>
<figure><figcaption>Time to apex vs approach pitch</figcaption><canvas id="time" class="selectable"></canvas></figure>
<figure><figcaption>Distance to apex vs approach pitch</figcaption><canvas id="distance" class="selectable"></canvas></figure>
<figure><figcaption>Forward speed at apex vs approach pitch</figcaption><canvas id="forward" class="selectable"></canvas></figure>
</div></section>
<section><h2>Across booster speeds</h2><div class="grid summaries">
<figure><figcaption>Best approach pitch · refined parabola · within 1 block band</figcaption><canvas id="best-pitch" class="selectable"></canvas></figure>
<figure><figcaption>Best apex</figcaption><canvas id="best-apex" class="selectable"></canvas></figure>
<figure><figcaption>Time to best apex</figcaption><canvas id="best-time" class="selectable"></canvas></figure>
</div></section>
<section><h2>Marker loss by speed and approach pitch · blocks</h2><div class="grid heatmaps">
<figure><figcaption>dTE n=20 loss</figcaption><canvas id="loss-dte" class="selectable"></canvas></figure>
<figure><figcaption>law K=0.771 loss</figcaption><canvas id="loss-law" class="selectable"></canvas></figure>
</div></section>
</main><script>
const PACKED="__PACKED__";
const FIELD="__FIELD__";
const FIELD_VZ=__FIELD_VZ__, FIELD_VY=__FIELD_VY__;
const SPEEDS=__SPEEDS__, PITCHES=__PITCHES__, BEST=__BEST__, MISSING=__MISSING__;
(async()=>{
const binary=Uint8Array.from(atob(PACKED),c=>c.charCodeAt(0));
const stream=new Blob([binary]).stream().pipeThrough(new DecompressionStream('gzip'));
const DATA=JSON.parse(await new Response(stream).text());
const $=id=>document.getElementById(id), slider=$('speed'), dte=$('show-dte'), law=$('show-law');
const charts={}, field=new Image();field.src=FIELD;
let speed=3, approach=25, hover=null, hoverSpeed=null, follow=true;
$('missing').textContent=MISSING.length?'Missing cells: '+MISSING.join(', '):'All 700 cells present. Click a profile or summary point to select a launch.';
const palette=[[68,33,84],[59,82,139],[33,145,140],[94,201,98],[253,231,37]];
function color(t,a=1){t=Math.max(0,Math.min(.999,t));let q=t*4,i=Math.floor(q),f=q-i;return `rgba(${palette[i].map((v,k)=>Math.round(v+(palette[i+1][k]-v)*f)).join(',')},${a})`}
function current(){return DATA[Math.round(speed*5)-1]}
function selected(){return current().find(c=>c[0]===approach)||current()[0]}
function summary(c,j=0){return c[1][j][0]}
function setup(id){const canvas=$(id),r=canvas.getBoundingClientRect(),d=Math.min(devicePixelRatio||1,2);canvas.width=Math.round(r.width*d);canvas.height=Math.round(r.height*d);const ctx=canvas.getContext('2d');ctx.setTransform(d,0,0,d,0,0);ctx.clearRect(0,0,r.width,r.height);return {canvas,ctx,w:r.width,h:r.height,m:{l:49,r:11,t:10,b:35}}}
// Round axes, as in tools/plot_floor_profiles.py: ticks on 1/2/2.5/5 steps, data domains rounded
// out to a tick. A pitch axis is passed high-to-low, so nose up (negative) is up the page.
function niceStep(span,n){const raw=span/n,mag=10**Math.floor(Math.log10(raw)),q=raw/mag;return (q<=1?1:q<=2?2:q<=2.5?2.5:q<=5?5:10)*mag}
function niceMax(v){const st=niceStep(v,5);return Math.ceil(v/st-1e-9)*st}
function bounds(vals){const v=vals.filter(Number.isFinite);if(!v.length)return [0,1];let lo=Math.min(...v),hi=Math.max(...v);if(lo===hi){lo-=1;hi+=1}const st=niceStep(hi-lo,4);return [Math.floor(lo/st+1e-9)*st,Math.ceil(hi/st-1e-9)*st]}
function tickDigits(st){for(let k=0;k<=6;k++)if(Math.abs(st*10**k-Math.round(st*10**k))<1e-9)return k;return 6}
function ticks(d,st){const lo=Math.min(...d),hi=Math.max(...d),out=[];for(let v=Math.ceil(lo/st-1e-9)*st;v<=hi+st*1e-9;v+=st)out.push(+v.toFixed(10));return out}
function axes(s,xd,yd,xlabel,ylabel,opt={}){const {ctx,w,h,m}=s,X=x=>m.l+(x-xd[0])/(xd[1]-xd[0])*(w-m.l-m.r),Y=y=>h-m.b-(y-yd[0])/(yd[1]-yd[0])*(h-m.t-m.b);
if(opt.paint)opt.paint({X,Y});
ctx.font='11px system-ui';ctx.lineWidth=.7;ctx.strokeStyle='rgba(185,192,201,.22)';ctx.fillStyle='#b9c0c9';
const xs=opt.xstep||niceStep(Math.abs(xd[1]-xd[0]),Math.max(3,Math.floor((w-m.l-m.r)/80))),ys=opt.ystep||niceStep(Math.abs(yd[1]-yd[0]),Math.max(3,Math.floor((h-m.t-m.b)/40)));
const fmt=(v,st)=>Math.abs(v)<1e-12?'0':v.toFixed(tickDigits(st));
for(const x of ticks(xd,xs)){const px=X(x);ctx.beginPath();ctx.moveTo(px,m.t);ctx.lineTo(px,h-m.b);ctx.stroke();ctx.textAlign='center';ctx.fillText(fmt(x,xs),px,h-m.b+15)}
for(const y of ticks(yd,ys)){const py=Y(y);ctx.beginPath();ctx.moveTo(m.l,py);ctx.lineTo(w-m.r,py);ctx.stroke();ctx.textAlign='right';ctx.fillText(fmt(y,ys),m.l-5,py+3)}
ctx.fillStyle='#e6e9ed';ctx.textAlign='center';ctx.fillText(xlabel,(m.l+w-m.r)/2,h-4);ctx.save();ctx.translate(11,(m.t+h-m.b)/2);ctx.rotate(-Math.PI/2);ctx.fillText(ylabel,0,0);ctx.restore();if(opt.seconds){ctx.fillStyle='#b9c0c9';ctx.textAlign='left';ctx.fillText('÷20 = seconds',m.l+4,m.t+11)}return {X,Y};}
function line(s,pts,sc,stroke,width=1,dash=[]){if(pts.length<2)return;const c=s.ctx;c.beginPath();pts.forEach((p,i)=>i?c.lineTo(sc.X(p[0]),sc.Y(p[1])):c.moveTo(sc.X(p[0]),sc.Y(p[1])));c.setLineDash(dash);c.strokeStyle=stroke;c.lineWidth=width;c.stroke();c.setLineDash([])}
function dot(s,x,y,r,fill,outline){const c=s.ctx;c.beginPath();c.arc(x,y,r,0,Math.PI*2);c.fillStyle=fill;c.fill();if(outline){c.lineWidth=1.5;c.strokeStyle=outline;c.stroke()}}
function marks(s,sc,xd,yd){const c=s.ctx;c.save();c.setLineDash([4,3]);for(const [v,name,col] of [[1.65,'goldrush','#e5bf65'],[3,'monster','#7bd9fa']]){if(v<xd[0]||v>xd[1])continue;const x=sc.X(v);c.beginPath();c.moveTo(x,sc.Y(yd[0]));c.lineTo(x,sc.Y(yd[1]));c.strokeStyle=col;c.lineWidth=1;c.stroke();c.fillStyle=col;c.font='10px system-ui';c.textAlign=v===3?'right':'left';c.fillText(name,v===3?x-3:x+3,sc.Y(yd[1])+11)}c.restore()}
function showRuns(){const cells=current(),sel=selected(),maxT=Math.max(...cells.flatMap(c=>[0,1,2].map(j=>summary(c,j)[1]))),maxY=Math.max(...cells.map(c=>summary(c)[0])),maxZ=Math.max(...cells.map(c=>summary(c)[2]));
const configs={field:{xd:FIELD_VZ,yd:FIELD_VY,xstep:1,ystep:1,xl:'vz, blocks/tick',yl:'vy, blocks/tick',get:(c,j)=>{let r=c[1][j][1];return r[3].map((v,i)=>[r[4][i],v])}},pitch:{xd:[0,niceMax(maxT)],yd:[90,-90],ystep:45,xl:'tick',yl:'pitch, degrees',get:(c,j)=>c[1][j][1][0].slice(0,-1).map((v,i)=>[i,v])},height:{xd:[0,niceMax(maxT)],yd:[0,niceMax(maxY)],xl:'tick',yl:'height, blocks',get:(c,j)=>c[1][j][1][1].map((v,i)=>[i,v])},path:{xd:[0,niceMax(maxZ)],yd:[0,niceMax(maxY)],xl:'distance, blocks',yl:'height, blocks',get:(c,j)=>{let r=c[1][j][1];return r[1].map((v,i)=>[r[2][i],v])}}};
for(const [id,cfg] of Object.entries(configs)){const s=setup(id),sc=axes(s,cfg.xd,cfg.yd,cfg.xl,cfg.yl,{xstep:cfg.xstep,ystep:cfg.ystep,paint:id==='field'&&field.complete?({X,Y})=>s.ctx.drawImage(field,X(FIELD_VZ[0]),Y(FIELD_VY[1]),X(FIELD_VZ[1])-X(FIELD_VZ[0]),Y(FIELD_VY[0])-Y(FIELD_VY[1])):null});
for(const c of cells){if(c===sel||c[0]%5!==0)continue;line(s,cfg.get(c,0),sc,color((5-c[0])/95,.35),.85)}
if(sel){const draw=(j,col,width,dash=[])=>{let pts=cfg.get(sel,j);if(id==='field')line(s,pts,sc,'#0c0d10',width+2);line(s,pts,sc,col,width,dash)};draw(0,'#ffffff',2.2);if(id==='pitch'||id==='height'){if(dte.checked)draw(1,'#f5ae68',1.8,[5,3]);if(law.checked)draw(2,'#e67dbc',1.8,[2,3])}}
charts[id]={s,sc,points:cells.map(c=>[c[0],cfg.get(c,0)])};}
}
function showAngleMetrics(){const cells=current(),best=BEST[Math.round(speed*5)-1];for(const [id,index,label] of [['apex',0,'apex, blocks'],['time',1,'ticks'],['distance',2,'distance, blocks'],['forward',3,'vz, blocks/tick']]){const s=setup(id),ys=cells.flatMap(c=>[0,1,2].map(j=>summary(c,j)[index])),yd=bounds(ys),sc=axes(s,[-90,5],yd,'approach pitch, degrees',label,{seconds:id==='time',xstep:15});
for(const [j,col,on] of [[1,'#f5ae68',dte.checked],[2,'#e67dbc',law.checked]])if(on)line(s,cells.map(c=>[c[0],summary(c,j)[index]]),sc,col,1.4,[5,3]);
line(s,cells.map(c=>[c[0],summary(c)[index]]),sc,'#86cfdf',1.7);
for(const c of cells){const x=sc.X(c[0]),y=sc.Y(summary(c)[index]);dot(s,x,y,c[0]===approach?4:2,c[0]===approach?'#fff':color((5-c[0])/95,.95),c[0]===approach?'#0c0d10':null)}
if(best){let c=cells.find(c=>c[0]===best[3]);if(c)dot(s,sc.X(c[0]),sc.Y(summary(c)[index]),6,'transparent','#f5e475')}
charts[id]={s,sc,points:cells.map(c=>[c[0],[sc.X(c[0]),sc.Y(summary(c)[index])]])};}}
function showSummaries(){for(const [id,metric,label] of [['best-pitch',0,'pitch, degrees'],['best-apex',4,'apex, blocks'],['best-time',5,'ticks']]){const s=setup(id),rows=BEST.map((b,i)=>b?[SPEEDS[i],b]:null).filter(Boolean),yd=id==='best-pitch'?[10,-95]:bounds(rows.map(r=>r[1][metric])),xd=[.2,5],sc=axes(s,xd,yd,'|v|, blocks/tick',label,{seconds:id==='best-time',ystep:id==='best-pitch'?30:null});
if(id==='best-pitch'){const c=s.ctx;c.beginPath();rows.forEach(([x,b],i)=>i?c.lineTo(sc.X(x),sc.Y(b[2])):c.moveTo(sc.X(x),sc.Y(b[2])));rows.slice().reverse().forEach(([x,b])=>c.lineTo(sc.X(x),sc.Y(b[1])));c.closePath();c.fillStyle='rgba(134,207,223,.15)';c.fill()}
marks(s,sc,xd,yd);line(s,rows.map(([x,b])=>[x,b[metric]]),sc,'#86cfdf',1.8);for(const [x,b] of rows)dot(s,sc.X(x),sc.Y(b[metric]),Math.abs(x-speed)<.01?4:2.3,Math.abs(x-speed)<.01?'#fff':'#86cfdf');charts[id]={s,sc,points:rows.map(([x,b])=>[x,[sc.X(x),sc.Y(b[metric])]])};}
}
function heatColor(loss,max){if(!Number.isFinite(loss))return '#33383e';if(loss<0)return '#e67dbc';const t=Math.max(0,Math.min(1,Math.log1p(Math.max(0,loss))/Math.log1p(max)));const a=[22,35,43],b=[246,182,75];return `rgb(${a.map((v,i)=>Math.round(v+(b[i]-v)*t)).join(',')})`}
function showHeatmaps(){let max=Math.max(1,...DATA.flatMap(g=>g.flatMap(c=>[1,2].map(j=>summary(c)[0]-summary(c,j)[0]))));for(const [id,j] of [['loss-dte',1],['loss-law',2]]){const s=setup(id),xd=[.1,5.1],yd=[10,-95],sc=axes(s,xd,yd,'|v|, blocks/tick','approach pitch, degrees',{ystep:30}),c=s.ctx;
for(let i=0;i<DATA.length;i++)for(const cell of DATA[i]){let x=sc.X(SPEEDS[i]-.1),x2=sc.X(SPEEDS[i]+.1),a=cell[0],y=sc.Y(a+1.6),y2=sc.Y(a-1.6);c.fillStyle=heatColor(summary(cell)[0]-summary(cell,j)[0],max);c.fillRect(x,y,x2-x,y2-y)}
for(const v of [1.65,3]){c.strokeStyle=v===3?'#7bd9fa':'#e5bf65';c.setLineDash([3,3]);c.beginPath();c.moveTo(sc.X(v),sc.Y(yd[0]));c.lineTo(sc.X(v),sc.Y(yd[1]));c.stroke();c.setLineDash([])}
c.strokeStyle='#fff';c.lineWidth=1.5;c.strokeRect(sc.X(speed-.1),sc.Y(approach+1.6),sc.X(speed+.1)-sc.X(speed-.1),sc.Y(approach-1.6)-sc.Y(approach+1.6));
c.fillStyle='#b9c0c9';c.textAlign='right';c.fillText(`0 → ${max.toFixed(1)} blocks (log color)`,s.w-12,s.m.t+12);charts[id]={s,sc,heat:true};}}
function detail(){const c=hover||selected();if(!c)return;const a=summary(c),x=summary(c,1),y=summary(c,2);$('detail').textContent=`|v| ${(hoverSpeed??speed).toFixed(1)} b/t · approach pitch ${c[0]}° · optimum apex ${a[0].toFixed(2)} blocks at tick ${a[1]} (${(a[1]/20).toFixed(2)} s) · distance ${a[2].toFixed(2)} blocks · forward ${a[3].toFixed(3)} b/t · dTE apex ${x[0].toFixed(2)} (loss ${(a[0]-x[0]).toFixed(2)}) · law apex ${y[0].toFixed(2)} (loss ${(a[0]-y[0]).toFixed(2)})`;}
function render(){if(!current().length)return;$('speed-value').textContent=speed.toFixed(1)+' blocks/tick';showRuns();showAngleMetrics();showSummaries();showHeatmaps();detail()}
function bestPitch(){return current().reduce((a,b)=>summary(b)[0]>summary(a)[0]?b:a)[0]}
function pin(a){approach=a;follow=false;$('follow-best').checked=false}
function pickSpeed(v){speed=SPEEDS.reduce((a,b)=>Math.abs(b-v)<Math.abs(a-v)?b:a);slider.value=speed;if(follow||!current().some(c=>c[0]===approach))approach=bestPitch();hover=null;hoverSpeed=null;render()}
for(const id of ['field','pitch','height','path','apex','time','distance','forward','best-pitch','best-apex','best-time','loss-dte','loss-law']){const canvas=$(id);canvas.addEventListener('click',e=>{let ch=charts[id],r=canvas.getBoundingClientRect(),x=e.clientX-r.left,y=e.clientY-r.top;if(ch.heat){pickSpeed(SPEEDS.reduce((a,b)=>Math.abs(ch.sc.X(b)-x)<Math.abs(ch.sc.X(a)-x)?b:a));pin(PITCHES.reduce((a,b)=>Math.abs(ch.sc.Y(b)-y)<Math.abs(ch.sc.Y(a)-y)?b:a));render();return}if(id.startsWith('best-')){let p=ch.points.reduce((a,b)=>Math.abs(b[1][0]-x)<Math.abs(a[1][0]-x)?b:a);pickSpeed(p[0]);return}let points=ch.points;if(id==='field'||id==='pitch'||id==='height'||id==='path'){let best=[Infinity,null];for(const [a,pts] of points)for(const p of pts){let dx=ch.sc.X(p[0])-x,dy=ch.sc.Y(p[1])-y,d=dx*dx+dy*dy;if(d<best[0])best=[d,a]}if(best[0]<900){pin(best[1]);hover=null;render()}return}let p=points.reduce((a,b)=>(b[1][0]-x)**2+(b[1][1]-y)**2<(a[1][0]-x)**2+(a[1][1]-y)**2?b:a);pin(p[0]);hover=null;render()});
canvas.addEventListener('mousemove',e=>{let ch=charts[id];if(!ch)return;let r=canvas.getBoundingClientRect(),x=e.clientX-r.left,y=e.clientY-r.top;if(ch.heat){let si=SPEEDS.reduce((a,b)=>Math.abs(ch.sc.X(b)-x)<Math.abs(ch.sc.X(a)-x)?b:a),ai=PITCHES.reduce((a,b)=>Math.abs(ch.sc.Y(b)-y)<Math.abs(ch.sc.Y(a)-y)?b:a);hover=DATA[Math.round(si*5)-1].find(c=>c[0]===ai)||null;hoverSpeed=si;detail();return}if(id.startsWith('best-')){let si=ch.points.reduce((a,b)=>Math.abs(b[1][0]-x)<Math.abs(a[1][0]-x)?b:a)[0],i=Math.round(si*5)-1;hover=DATA[i].find(c=>c[0]===BEST[i][3])||null;hoverSpeed=si;detail();return}let a=null;if(id==='field'||id==='pitch'||id==='height'||id==='path'){let best=[400,null];for(const [ang,pts] of ch.points)for(const p of pts){let d=(ch.sc.X(p[0])-x)**2+(ch.sc.Y(p[1])-y)**2;if(d<best[0])best=[d,ang]}a=best[1]}else{let best=[400,null];for(const [ang,p] of ch.points){let d=(p[0]-x)**2+(p[1]-y)**2;if(d<best[0])best=[d,ang]}a=best[1]}hover=current().find(c=>c[0]===a)||null;hoverSpeed=null;detail()});canvas.addEventListener('mouseleave',()=>{hover=null;hoverSpeed=null;detail()})}
slider.addEventListener('input',()=>pickSpeed(+slider.value));$('follow-best').addEventListener('change',e=>{follow=e.target.checked;if(follow)approach=bestPitch();render()});dte.addEventListener('change',render);law.addEventListener('change',render);
let timer;new ResizeObserver(()=>{clearTimeout(timer);timer=setTimeout(render,90)}).observe(document.querySelector('main'));
approach=bestPitch();field.onload=render;render();
})().catch(e=>{document.getElementById('detail').textContent='Could not load embedded data: '+e.message;console.error(e)});
</script></body></html>'''


def main():
    data, missing, anomalies = read_cells()
    best = refined_bests(data)
    payload = json.dumps(data, separators=(",", ":"))
    packed = base64.b64encode(gzip.compress(payload.encode(), compresslevel=9)).decode("ascii")
    html = (HTML.replace("__PACKED__", packed).replace("__FIELD__", energy_field())
            .replace("__SPEEDS__", json.dumps(SPEEDS))
            .replace("__PITCHES__", json.dumps(PITCHES))
            .replace("__FIELD_VZ__", json.dumps(FIELD_VZ)).replace("__FIELD_VY__", json.dumps(FIELD_VY))
            .replace("__BEST__", json.dumps(best))
            .replace("__MISSING__", json.dumps(missing)))
    OUT.write_text(html)
    print(f"Wrote {OUT} ({OUT.stat().st_size:,} bytes)")
    print("Missing:", ", ".join(missing) or "none")
    print("Marker above optimum by >0.05 blocks:", anomalies or "none")
    print("Speed  refined pitch  near-optimal pitch band  grid best  apex")
    for speed, row in zip(SPEEDS, best):
        if row:
            print(f"{speed:4.1f}  {row[0]:7.3f}°  {row[1]:3d}–{row[2]:3d}°  {row[3]:3d}°  {row[4]:8.3f}")
        else:
            print(f"{speed:4.1f}  no cells")


if __name__ == "__main__":
    main()
