#!/usr/bin/env python3
"""Generate an offline synchronized viewer beside the two comparison movies."""
import json
import pathlib

root = pathlib.Path(__file__).resolve().parents[1]
folder = root / "benchmarks/music-review"
catalog = json.loads((root / "reports/poc-catalog.json").read_text())["presets"]
playlist = json.loads((folder / "playlist.json").read_text())
data = json.dumps({"presets": [p["name"] for p in catalog], "tracks": [
    {"start": s["playlist_start_seconds"], "title": s["tags"].get("artist", "") + " — " + s["tags"].get("title", "")}
    for s in playlist["sources"]
]}).replace("<", "\\u003c")
html = r'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>MilkDrop · Beatport comparison</title>
<style>
:root{color-scheme:dark;--ink:#f4f1e8;--muted:#aca99e;--paper:#151614;--line:#42443d;--accent:#eeac64}
*{box-sizing:border-box}body{margin:0;background:var(--paper);color:var(--ink);font:15px/1.5 'Avenir Next',Avenir,sans-serif}
main{max-width:1800px;margin:auto;padding:32px 36px}header{display:flex;align-items:end;justify-content:space-between;gap:24px;margin-bottom:30px}
h1{font:500 36px/1.12 Georgia,serif;letter-spacing:-.8px;margin:7px 0}p{margin:0}.eyebrow,.tiny{font:12px/1.5 'SFMono-Regular',Consolas,monospace;letter-spacing:.08em;text-transform:uppercase;color:var(--muted)}
.description{color:var(--muted);max-width:440px}button,select{font:inherit;color:inherit;background:transparent;border:1px solid var(--line);padding:9px 14px;border-radius:2px;cursor:pointer}
button:hover,button[aria-pressed="true"]{border-color:var(--accent);color:var(--accent)}button:disabled{opacity:.4;cursor:default}
button:focus-visible,select:focus-visible,input:focus-visible{outline:2px solid var(--accent);outline-offset:4px}
.screens{display:grid;grid-template-columns:1fr 1fr;gap:20px}.screens[data-view="adapted"],.screens[data-view="projectm"]{grid-template-columns:1fr;max-width:1300px;margin:auto}
.screens[data-view="adapted"] .reference,.screens[data-view="projectm"] .candidate{display:none}
figure{margin:0;min-width:0}figcaption{display:flex;align-items:center;justify-content:space-between;margin:0 0 9px;font-size:16px}
figcaption span{color:var(--muted);font-size:12px}video{display:block;width:100%;aspect-ratio:16/9;object-fit:contain;background:#070807}
.transport{padding:22px 0;border-bottom:1px solid var(--line)}.time{display:flex;justify-content:space-between;gap:16px;font:13px 'SFMono-Regular',Consolas,monospace;color:var(--muted);margin-bottom:12px}
input[type="range"]{width:100%;accent-color:var(--accent);cursor:pointer}input[type="checkbox"]{accent-color:var(--accent)}
.controls{display:flex;align-items:center;flex-wrap:wrap;gap:10px;margin-top:18px}.primary{background:var(--ink);color:var(--paper);border-color:var(--ink);min-width:92px;font-weight:600}.primary:hover{background:var(--accent);color:var(--paper)}
.mode{margin-left:auto;display:flex;gap:6px}.controls label{margin-left:10px;color:var(--muted);white-space:nowrap}
.preset{display:grid;grid-template-columns:auto minmax(0,1fr) auto;align-items:center;gap:20px;margin-top:24px}select{width:100%;background:var(--paper)}
.song{color:var(--muted);margin:16px 0 24px;font-size:14px}.song span{color:var(--ink)}
footer{display:flex;justify-content:space-between;align-items:baseline;gap:20px;color:var(--muted);font-size:12px}footer a{color:var(--muted)}.status{color:var(--accent)}
@media(max-width:900px){main{padding:24px 18px}header{display:block}.description{margin-top:12px}.screens{grid-template-columns:1fr}.mode{margin-left:0}.preset{grid-template-columns:1fr;gap:8px}footer{display:block}h1{font-size:30px}}
</style></head><body><main>
<header><div><p class="eyebrow">Beatport / 100 presets / native 4K</p><h1>Two renders. One playlist.</h1></div><p class="description">Compare the adapted renderer with ProjectM-TV on the same music timeline. Each preset gets 30 seconds.</p></header>
<section class="screens" data-view="both" aria-label="Comparison videos">
<figure class="candidate"><figcaption>Adapted <span>3840 × 2160</span></figcaption><video id="adapted" preload="metadata" playsinline aria-label="Adapted renderer video" src="adapted-beatport-50min-4k.mp4"></video></figure>
<figure class="reference"><figcaption>ProjectM-TV <span>3840 × 2160</span></figcaption><video id="projectm" preload="metadata" playsinline muted aria-label="ProjectM-TV reference video" src="projectm-beatport-50min-4k.mp4"></video></figure>
</section>
<section class="transport" aria-label="Shared playback controls">
<div class="time"><output id="clock">00:00 / 50:00</output><output id="alignment">Loading movies…</output></div>
<input id="timeline" type="range" min="0" max="2999.966667" step="0.033333333" value="0" aria-label="Shared video timeline" disabled>
<div class="controls"><button id="play" class="primary" disabled>Play</button><button id="previous" disabled aria-label="Previous preset">← Preset</button><button id="next" disabled aria-label="Next preset">Preset →</button><button id="backframe" disabled aria-label="Previous frame">− Frame</button><button id="nextframe" disabled aria-label="Next frame">+ Frame</button><label><input id="loop" type="checkbox"> Loop preset</label><label><input id="music" type="checkbox" checked> Music</label>
<div class="mode" role="group" aria-label="Comparison layout"><button data-mode="both" aria-pressed="true">Side by side</button><button data-mode="adapted" aria-pressed="false">Adapted</button><button data-mode="projectm" aria-pressed="false">ProjectM-TV</button></div></div>
</section>
<section class="preset"><label for="preset" class="tiny">Preset</label><select id="preset" disabled></select><output id="rank" class="tiny">001 / 100</output></section>
<p class="song">Playing <span id="song">—</span></p>
<footer><p id="status" class="status" role="status">Waiting for both movies.</p><p>Space: play/pause · ← / →: 5 seconds · <a href="playlist.json">Playlist & provenance</a></p></footer>
<p style="margin-top:18px;color:var(--muted);font-size:12px">The reference keeps its own animation clock and randomness. These recordings compare visual response; they do not measure playback performance.</p>
</main><script>
const DATA=__DATA__;
const a=document.querySelector('#adapted'),b=document.querySelector('#projectm'),videos=[a,b];
const byId=id=>document.getElementById(id), timeline=byId('timeline'),play=byId('play'),preset=byId('preset');
let ready=false,playing=false,busy=false,scrubbing=false,limit=3000,loopRank=0,seekGeneration=0;
const preview=Number(new URLSearchParams(location.search).get('preview'));
if(preview>0&&preview<=3000){a.src='adapted-beatport-50min-4k.partial.mp4';limit=preview;byId('status').textContent='Recording preview: completed chapters only.';}
DATA.presets.forEach((name,index)=>{const option=document.createElement('option');option.value=index;option.textContent=String(index+1).padStart(3,'0')+' · '+name;preset.append(option)});
const time=t=>String(Math.floor(t/60)).padStart(2,'0')+':'+String(Math.floor(t%60)).padStart(2,'0');
function pause(){playing=false;videos.forEach(v=>v.pause());play.textContent='Play'}
function render(){const t=a.currentTime||0,rank=Math.min(99,Math.floor(t/30));if(!scrubbing){timeline.value=t;byId('clock').textContent=time(t)+' / '+time(limit)}preset.value=rank;byId('rank').textContent=String(rank+1).padStart(3,'0')+' / 100';byId('song').textContent=DATA.tracks.filter(track=>track.start<=t).at(-1)?.title||'—';if(ready)byId('alignment').textContent=busy?'Seeking…':Math.abs(a.currentTime-b.currentTime)<.08?'Aligned':'Aligning…'}
function availability(){if(videos.every(v=>v.readyState>=1&&!v.error)){limit=Math.min(limit,...videos.map(v=>v.duration));ready=Number.isFinite(limit)&&limit>0;document.querySelectorAll('.transport button,#timeline,#preset').forEach(c=>c.disabled=!ready);timeline.max=Math.max(0,limit-1/30);if(ready){byId('status').textContent=preview>0?'Recording preview: completed chapters only.':'Ready. Shared music, preset order and timeline.';render()}}}
async function start(){if(!ready||busy)return;playing=true;try{await Promise.all(videos.map(v=>v.play()));if(playing)play.textContent='Pause';else videos.forEach(v=>v.pause())}catch(e){pause();byId('status').textContent='Playback unavailable. Open the movies in a player with HEVC support.'}}
async function seek(t,resume=playing){if(!ready)return;const generation=++seekGeneration;pause();busy=true;t=Math.max(0,Math.min(limit-1/30,t));
try{await Promise.all(videos.map(v=>new Promise((resolve,reject)=>{if(Math.abs(v.currentTime-t)<.0001){resolve();return}const timeout=setTimeout(()=>done(new Error('Seek timeout')),15000);const done=error=>{clearTimeout(timeout);v.removeEventListener('seeked',complete);error?reject(error):resolve()};const complete=()=>done();v.addEventListener('seeked',complete,{once:true});v.currentTime=t})));if(generation!==seekGeneration)return;busy=false;render();if(resume)await start()}catch(e){if(generation===seekGeneration){busy=false;byId('status').textContent='Could not seek both movies. Try another position or reload.'}}}
play.onclick=()=>playing?pause():start();
timeline.onpointerdown=()=>{scrubbing=true;pause()};timeline.oninput=()=>{scrubbing=true;byId('clock').textContent=time(Number(timeline.value))+' / '+time(limit)};timeline.onchange=()=>{scrubbing=false;loopRank=Math.floor(Number(timeline.value)/30);seek(Number(timeline.value),false)};
preset.onchange=()=>{loopRank=Number(preset.value);seek(loopRank*30)};
byId('previous').onclick=()=>{loopRank=Math.max(0,Math.floor(a.currentTime/30)-1);seek(loopRank*30)};
byId('next').onclick=()=>{loopRank=Math.min(99,Math.floor(a.currentTime/30)+1);seek(loopRank*30)};
byId('backframe').onclick=()=>seek(a.currentTime-1/30,false);byId('nextframe').onclick=()=>seek(a.currentTime+1/30,false);
byId('music').onchange=()=>a.muted=!byId('music').checked;byId('loop').onchange=()=>loopRank=Math.floor(a.currentTime/30);
document.querySelectorAll('[data-mode]').forEach(button=>button.onclick=()=>{document.querySelector('.screens').dataset.view=button.dataset.mode;document.querySelectorAll('[data-mode]').forEach(other=>other.setAttribute('aria-pressed',String(other===button)))});
videos.forEach(v=>{v.addEventListener('loadedmetadata',availability);v.addEventListener('error',()=>{pause();ready=false;byId('status').textContent=(v===a?'Adapted':'ProjectM-TV')+' movie unavailable. Keep both MP4 files beside this page and use an HEVC-capable browser.';document.querySelectorAll('#play,#timeline,#preset,#previous,#next,#backframe,#nextframe').forEach(c=>c.disabled=true)})});
a.addEventListener('ended',pause);
setInterval(()=>{if(playing&&!busy){if(byId('loop').checked&&a.currentTime>=(loopRank+1)*30-1/30){seek(loopRank*30,true);return}if(a.currentTime>=limit-1/30){pause();return}if(a.readyState<3||b.readyState<3){const t=a.currentTime;seek(t,true);return}const drift=b.currentTime-a.currentTime;if(Math.abs(drift)>.2){seek(a.currentTime,true);return}b.playbackRate=Math.abs(drift)>.025?(drift>0?.98:1.02):1}render()},100);
document.addEventListener('keydown',event=>{if(event.target.closest('button,input,select'))return;if(event.code==='Space'){event.preventDefault();playing?pause():start()}if(event.code==='ArrowLeft'||event.code==='ArrowRight'){event.preventDefault();seek(a.currentTime+(event.code==='ArrowLeft'?-5:5))}});
</script></body></html>'''
(folder / "review.html").write_text(html.replace("__DATA__", data))
print(folder / "review.html")
