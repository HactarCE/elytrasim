#!/usr/bin/env python3
"""Build the interactive myopic-rule comparison for the mapfine reference profile.

The reference is read from feasible-explorer.html instead of duplicated here.  Rule pitches are
evaluated pointwise at the optimum's own state, using the profile's recorded trig and flight modes.

    python3 tools/plot_mapfine_rules.py
    python3 tools/plot_mapfine_rules.py EXPLORER_HTML OUT_HTML [MAX_LOOKAHEAD]
"""

import base64
import csv
import gzip
import io
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from load import load


EXPLORER = (sys.argv[1] if len(sys.argv) > 1
            else "runs/atlas/fig/mapfine/feasible-explorer.html")
OUT = (sys.argv[2] if len(sys.argv) > 2
       else "runs/atlas/fig/mapfine/myopic-rules.html")
MAX_LOOKAHEAD = int(sys.argv[3]) if len(sys.argv) > 3 else 60
RUN_ROOT = "runs/atlas/mapfine/out"


def reference_from_explorer(path):
    text = open(path).read()
    match = re.search(r'const PACKED = "([A-Za-z0-9+/=]+)"', text)
    if not match:
        raise ValueError(f"no packed payload found in {path}")
    payload = json.loads(gzip.decompress(base64.b64decode(match.group(1))))
    return payload["reference"]


def rule_rows(profile):
    vy0, vz0 = profile.v0
    cmd = [
        "cargo", "run", "--quiet", "--release", "--bin", "myopic", "--",
        "--trig", profile.trig, "--flight", profile.header.get("flight", "reference"),
        "rules", profile.path, str(vy0), str(vz0), str(MAX_LOOKAHEAD),
    ]
    completed = subprocess.run(cmd, check=True, text=True, capture_output=True)
    rows = []
    for raw in csv.DictReader(io.StringIO(completed.stdout)):
        rows.append({key: (int(value) if key == "tick" else
                           (round(float(value), 3) if value else None))
                     for key, value in raw.items()})
    return rows, completed.stderr.strip()


def write_html(reference, profile, rows):
    data = json.dumps(rows, separators=(",", ":"))
    title = (f"myopic rules at n = {reference['n']}, "
             f"implied &lambda; = {reference['lambda']:.2f}")
    template = r'''<div id="mapfine-rules">
  <style>
    html, body { margin: 0; background: #14171a; }
    #mapfine-rules { box-sizing: border-box; width: 100%; min-height: 520px; padding: 18px 20px 16px; color: #e6e9ed; background: #14171a; font: 14px system-ui, sans-serif; }
    #mapfine-rules * { box-sizing: border-box; }
    #mapfine-rules h1 { margin: 0 0 3px; font-size: 20px; font-weight: 500; }
    #mapfine-rules .subtitle { margin: 0 0 15px; color: #8f98a5; font-size: 13px; }
    #mapfine-rules .legend { display: flex; flex-wrap: wrap; gap: 8px 24px; margin: 0 0 13px; }
    #mapfine-rules .series { display: inline-flex; align-items: center; gap: 8px; padding: 2px 0; border: 0; color: #b9c0c9; background: transparent; font: inherit; cursor: pointer; }
    #mapfine-rules .series[aria-pressed="false"] { opacity: .38; }
    #mapfine-rules .swatch { width: 32px; height: 0; border-top: 3px solid var(--series); }
    #mapfine-rules .swatch.dashed { border-top-style: dashed; }
    #mapfine-rules .control { display: grid; grid-template-columns: auto minmax(160px, 460px) 62px; align-items: center; gap: 10px; margin: 0 0 12px; color: #b9c0c9; }
    #mapfine-rules input[type="range"] { width: 100%; accent-color: #e46bb5; }
    #mapfine-rules output { color: #e6e9ed; font-variant-numeric: tabular-nums; text-align: right; }
    #mapfine-rules .plot-wrap { position: relative; height: min(64vh, 610px); min-height: 390px; }
    #mapfine-rules canvas { display: block; width: 100%; height: 100%; background: #1b1f24; border: 1px solid #4a525c; }
    #mapfine-rules .tooltip { position: absolute; z-index: 2; display: none; min-width: 205px; padding: 8px 10px; border: 1px solid #4a525c; background: rgba(20,23,26,.96); color: #e6e9ed; pointer-events: none; font-size: 12px; font-variant-numeric: tabular-nums; }
    #mapfine-rules .tooltip .tick { margin-bottom: 5px; color: #b9c0c9; }
    #mapfine-rules .tooltip .row { display: grid; grid-template-columns: 10px 1fr auto; gap: 7px; align-items: center; line-height: 1.55; }
    #mapfine-rules .tooltip .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--series); }
    @media (max-width: 600px) { #mapfine-rules { padding: 14px 10px; } #mapfine-rules .control { grid-template-columns: 1fr 54px; } #mapfine-rules .control label { grid-column: 1 / -1; } #mapfine-rules .plot-wrap { min-height: 360px; } }
  </style>
  <h1>__TITLE__</h1>
  <p class="subtitle">Each rule is evaluated at the optimum's state on that tick. &Delta;TE search is capped at &plusmn;85&deg;.</p>
  <div class="legend" aria-label="series">
    <button type="button" class="series" data-series="optimum" aria-pressed="true"><span class="swatch" style="--series:#ffffff"></span><span>optimum</span></button>
    <button type="button" class="series" data-series="hold" aria-pressed="true"><span class="swatch dashed" style="--series:#55c1ff"></span><span>hold flight-path angle</span></button>
    <button type="button" class="series" data-series="lookahead" aria-pressed="true"><span class="swatch dashed" style="--series:#e46bb5"></span><span id="mr-lookahead-legend">argmax &Delta;TE over 20 ticks</span></button>
    <button type="button" class="series" data-series="one" aria-pressed="true"><span class="swatch dashed" style="--series:#f08065"></span><span>argmax &Delta;TE over 1 tick</span></button>
  </div>
  <div class="control">
    <label for="mr-lookahead">&Delta;TE lookahead</label>
    <input id="mr-lookahead" type="range" min="1" max="__MAX__" value="20" step="1">
    <output id="mr-lookahead-value" for="mr-lookahead">20 ticks</output>
  </div>
  <div class="plot-wrap">
    <canvas id="mr-chart" aria-label="Pitch by tick for the optimum and three pointwise rules; negative ninety degrees is at the top"></canvas>
    <div id="mr-tooltip" class="tooltip" role="tooltip"></div>
  </div>
  <script>
  (() => {
    const root = document.getElementById("mapfine-rules");
    const data = __DATA__;
    const slider = root.querySelector("#mr-lookahead");
    const value = root.querySelector("#mr-lookahead-value");
    const legendLabel = root.querySelector("#mr-lookahead-legend");
    const canvas = root.querySelector("#mr-chart");
    const tooltip = root.querySelector("#mr-tooltip");
    const active = {optimum:true, hold:true, lookahead:true, one:true};
    const colors = {optimum:"#ffffff", hold:"#55c1ff", lookahead:"#e46bb5", one:"#f08065"};
    let layout = null, pointer = null, resizeTimer = null;

    function series() {
      const n = +slider.value;
      return [
        {id:"one", label:"argmax \u0394TE over 1 tick", keys:["n1"], color:colors.one, width:2.0, dash:[7,5]},
        {id:"hold", label:"hold flight-path angle", keys:["hold_gamma"], color:colors.hold, width:2.0, dash:[7,5]},
        {id:"lookahead", label:`argmax \u0394TE over ${n} tick${n===1?"":"s"}`, keys:[`n${n}`], color:colors.lookahead, width:2.5, dash:[7,5]},
        {id:"optimum", label:"optimum", keys:["optimum"], color:colors.optimum, width:3.0, dash:[]},
      ];
    }

    function draw() {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round(rect.width * dpr); canvas.height = Math.round(rect.height * dpr);
      const ctx = canvas.getContext("2d"); ctx.setTransform(dpr,0,0,dpr,0,0);
      const w=rect.width, h=rect.height, m={l:w<520?60:72,r:18,t:18,b:54};
      const x0=m.l, x1=w-m.r, y0=m.t, y1=h-m.b;
      const X=t=>x0+t/(data.length-1)*(x1-x0);
      const Y=p=>y0+(p+90)/180*(y1-y0);
      layout={w,h,m,x0,x1,y0,y1,X,Y};
      ctx.clearRect(0,0,w,h);

      ctx.font="12px system-ui"; ctx.lineWidth=1;
      const xticks=w<520?[0,50,100,data.length-1]:[0,20,40,60,80,100,120,data.length-1];
      for(const t of xticks){const x=X(t);ctx.strokeStyle="rgba(185,192,201,.16)";ctx.beginPath();ctx.moveTo(x,y0);ctx.lineTo(x,y1);ctx.stroke();ctx.fillStyle="#b9c0c9";ctx.textAlign=t===0?"left":t===data.length-1?"right":"center";ctx.fillText(String(t),x,y1+20);}
      for(const p of [-90,-60,-30,0,30,60,90]){const y=Y(p);ctx.strokeStyle=p===0?"rgba(230,233,237,.34)":"rgba(185,192,201,.16)";ctx.beginPath();ctx.moveTo(x0,y);ctx.lineTo(x1,y);ctx.stroke();ctx.fillStyle="#b9c0c9";ctx.textAlign="right";ctx.fillText(`${p>0?"+":""}${p}\u00b0`,x0-9,y+4);}
      ctx.strokeStyle="#4a525c";ctx.strokeRect(x0+.5,y0+.5,x1-x0-1,y1-y0-1);
      ctx.fillStyle="#e6e9ed";ctx.textAlign="center";ctx.fillText("tick",(x0+x1)/2,h-12);
      ctx.save();ctx.translate(17,(y0+y1)/2);ctx.rotate(-Math.PI/2);ctx.fillText("pitch, degrees",0,0);ctx.restore();

      for(const s of series()){
        if(!active[s.id]) continue;
        s.keys.forEach((key,branch)=>{
          ctx.beginPath();let drawing=false;
          data.forEach(d=>{const p=d[key];if(p===null||p===undefined){drawing=false;return}const x=X(d.tick),y=Y(p);drawing?ctx.lineTo(x,y):ctx.moveTo(x,y);drawing=true});
          ctx.setLineDash(s.dash);ctx.strokeStyle=s.color;ctx.globalAlpha=branch===0?1:.64;ctx.lineWidth=Math.max(1.25,s.width-branch*.35);ctx.lineJoin="round";ctx.lineCap="round";ctx.stroke();ctx.setLineDash([]);ctx.globalAlpha=1;
        });
      }
      if(pointer) drawPointer(ctx, pointer.x);
    }

    function interpolated(key, t){
      const lo=Math.max(0,Math.min(data.length-1,Math.floor(t))), hi=Math.min(data.length-1,lo+1), f=t-lo;
      const a=data[lo][key],b=data[hi][key];if(a===null||a===undefined||b===null||b===undefined)return null;
      return a+(b-a)*f;
    }

    function drawPointer(ctx, x){
      if(!layout) return;
      const t=(x-layout.x0)/(layout.x1-layout.x0)*(data.length-1);
      ctx.strokeStyle="rgba(230,233,237,.42)";ctx.lineWidth=1;ctx.beginPath();ctx.moveTo(x,layout.y0);ctx.lineTo(x,layout.y1);ctx.stroke();
      for(const s of series()){
        if(!active[s.id]) continue;
        for(const key of s.keys){const p=interpolated(key,t);if(p===null)continue;const y=layout.Y(p);ctx.beginPath();ctx.arc(x,y,3.5,0,Math.PI*2);ctx.fillStyle=s.color;ctx.fill();ctx.strokeStyle="#1b1f24";ctx.stroke();}
      }
    }

    function updateTooltip(clientX, clientY){
      if(!layout) return;
      const rect=canvas.getBoundingClientRect(), x=Math.max(layout.x0,Math.min(layout.x1,clientX-rect.left));
      const t=(x-layout.x0)/(layout.x1-layout.x0)*(data.length-1);pointer={x};
      const rows=series().filter(s=>active[s.id]).map(s=>{const ps=s.keys.map(key=>interpolated(key,t)).filter(p=>p!==null);if(!ps.length)return "";return `<div class="row"><span class="dot" style="--series:${s.color}"></span><span>${s.label}</span><span>${ps.map(p=>p.toFixed(1)+"\u00b0").join(" / ")}</span></div>`}).join("");
      tooltip.innerHTML=`<div class="tick">tick ${t.toFixed(1)}</div>${rows}`;tooltip.style.display="block";
      const tw=tooltip.offsetWidth, th=tooltip.offsetHeight, localY=clientY-rect.top;
      tooltip.style.left=`${Math.min(layout.w-tw-8,Math.max(8,x+12))}px`;tooltip.style.top=`${Math.min(layout.h-th-8,Math.max(8,localY-th/2))}px`;draw();
    }

    slider.addEventListener("input",()=>{const n=+slider.value,s=`${n} tick${n===1?"":"s"}`;value.textContent=s;legendLabel.textContent=`argmax \u0394TE over ${s}`;draw();});
    root.querySelectorAll(".series").forEach(button=>button.addEventListener("click",()=>{const id=button.dataset.series;active[id]=!active[id];button.setAttribute("aria-pressed",String(active[id]));draw();}));
    canvas.addEventListener("pointermove",e=>updateTooltip(e.clientX,e.clientY));
    canvas.addEventListener("pointerleave",()=>{pointer=null;tooltip.style.display="none";draw();});
    const scheduleDraw=()=>{clearTimeout(resizeTimer);resizeTimer=setTimeout(draw,60)};
    new ResizeObserver(scheduleDraw).observe(root.querySelector(".plot-wrap"));
    window.addEventListener("resize",scheduleDraw);
    draw();
  })();
  </script>
</div>'''
    html = (template.replace("__TITLE__", title)
            .replace("__MAX__", str(MAX_LOOKAHEAD))
            .replace("__DATA__", data))
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as fh:
        fh.write(html)


def main():
    reference = reference_from_explorer(EXPLORER)
    path = os.path.join(RUN_ROOT, reference["cell"], reference["profile"])
    profile = load(path)
    assert profile.n == reference["n"]
    rows, branch_summary = rule_rows(profile)
    write_html(reference, profile, rows)
    print(f"wrote {OUT}")
    print(f"reference {reference['cell']}/{reference['profile']}; "
          f"n={reference['n']}, implied lambda={reference['lambda']:.8f}, "
          f"lookahead=1..{MAX_LOOKAHEAD}")
    print(branch_summary)


if __name__ == "__main__":
    main()
