#!/usr/bin/env python3
"""Report all frozen outcomes; separate CPU, presentation and visual evidence."""
import hashlib,json,pathlib,statistics,sys,tarfile
from PIL import Image,ImageDraw,ImageStat
root=pathlib.Path(__file__).resolve().parents[1];out=root/"benchmarks/emulator/native-poc"/sys.argv[1]
rows=[json.loads(l)for l in (out/"timing.jsonl").read_text().splitlines()]
latency=json.loads((out/"surface-latency.json").read_text())["rows"]
cat=json.loads((root/"reports/poc-catalog.json").read_text())["presets"]
captures=out/"captures";captures.mkdir(exist_ok=True)
with tarfile.open(out/"captures.tar")as archive:archive.extractall(captures,filter="data")
def percentile(values,p):return sorted(values)[int((len(values)-1)*p)]if values else None
def summary(values):return {"count":len(values),"p50":percentile(values,.5),"p95":percentile(values,.95),"p99":percentile(values,.99),"max":max(values)if values else None}
results=[]
for rank in range(1,101):
    all_frames=[r for r in rows if r.get("rank")==rank];hot=[r for r in all_frames if r.get("frame",0)>30 and r.get("state")=="rendering"]
    cpus=[r["cpu_frame_ms"]for r in hot];starts=[r["frame_start_ns"]for r in hot]
    present=sorted({actual for desired,actual,ready in latency if starts and min(starts)<=desired<=max(starts)+hot[-1]["cpu_frame_ms"]*1e6})
    intervals=[(b-a)/1e6 for a,b in zip(present,present[1:])]
    frame=captures/f"{rank:03}.jpg";image=Image.open(frame);assert image.size==(3840,2160);preview=image.copy();preview.thumbnail((320,180));stats=ImageStat.Stat(preview)
    results.append({"rank":rank,"source":cat[rank-1]["name"],"source_sha256":cat[rank-1]["source_sha256"],"state":"rendered"if len(all_frames)==180 and len(hot)==150 else "incomplete","native_dimensions":image.size,"cpu_ms":summary(cpus),"cpu_over_33_333_ms":sum(v>1000/30 for v in cpus),"present_interval_ms":summary(intervals),"observed_present_fps":(len(present)-1)*1e9/(present[-1]-present[0])if len(present)>1 else None,"present_gaps_over_33_333_ms":sum(v>1000/30 for v in intervals),"resource_substitutions":hot[-1].get("resource_substitutions",[])if hot else [],"visual_snapshot":"low-spatial-variation-review-required"if max(stats.stddev)<4 else "detail-present-not-motion-acceptance","rgb_mean":stats.mean,"rgb_stddev":stats.stddev,"preview_sha256":hashlib.sha256(frame.read_bytes()).hexdigest()})
for group in range(4):
    sheet=Image.new("RGB",(1600,1000),"#101216");draw=ImageDraw.Draw(sheet)
    for i,row in enumerate(results[group*25:(group+1)*25]):
        im=Image.open(captures/f"{row['rank']:03}.jpg");im.thumbnail((320,180));x=i%5*320;y=i//5*200;sheet.paste(im,(x,y));draw.text((x+5,y+182),str(row["rank"]),fill="white")
    sheet.save(out/f"contact-{group+1}.jpg",quality=92)
report={"scope":"M4 Android TV emulator; 150 hot frames/preset after30 warmup; comparison readback excluded","presentation_source":"SurfaceFlinger per-layer desired/actual/ready fence timestamps; final surface presentation, not physical panel","qualification":"Strict all-frame 30-fps deadlines require zero measured misses; averages do not establish qualification","rendered":sum(r['state']=="rendered"for r in results),"count":100,"cpu_ms":summary([r["cpu_frame_ms"]for r in rows if r.get('frame',0)>30 and r.get('state')=='rendering']),"outcomes":results}
(out/"summary.json").write_text(json.dumps(report,indent=2)+"\n")
print("Rendered",report["rendered"],"CPU",report["cpu_ms"])
print("Lowest observed present fps",sorted((r['observed_present_fps'],r['rank'])for r in results if r['observed_present_fps'])[:5])
print("Low-variation snapshots",[r['rank']for r in results if r['visual_snapshot'].startswith('low')])
