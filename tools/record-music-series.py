#!/usr/bin/env python3
"""Collect all100 chapters, then attach the identical soundtrack and chapter cues."""
import json,pathlib,subprocess,hashlib,sys,time
root=pathlib.Path(__file__).resolve().parents[1];out=root/"benchmarks/music-review";catalog=json.loads((root/"reports/poc-catalog.json").read_text())["presets"]
engine=sys.argv[1];assert engine in["adapted","projectm"]
folder=out/engine;folder.mkdir(exist_ok=True);status=out/(engine+"-status.json")
results=[]
for rank in range(1,101):
 video=folder/f"{rank:03}.mp4";meta=video.with_suffix(".json")
 if meta.exists():
  row=json.loads(meta.read_text())
  if row.get("frames")==900 and row.get("rank")==rank and video.exists()and hashlib.sha256(video.read_bytes()).hexdigest()==row['sha256']:
   results.append(row);continue
 process=subprocess.run([sys.executable,str(root/"tools/record-music-chapter.py"),engine,str(rank)],cwd=root)
 if process.returncode:
  status.write_text(json.dumps({"state":"failed","engine":engine,"rank":rank,"completed":len(results)},indent=2));raise SystemExit(process.returncode)
 results.append(json.loads(meta.read_text()));status.write_text(json.dumps({"state":"recording","engine":engine,"completed":len(results),"count":100,"elapsed_wall_seconds":sum(r["wall_seconds"]for r in results)},indent=2));print(f"{engine}: {rank}/100 complete",flush=True)
concat=out/(engine+"-concat.txt");concat.write_text(''.join("file '"+str(folder/f"{rank:03}.mp4")+"'\n"for rank in range(1,101)))
chapters=out/"chapters.ffmetadata";chapters.write_text(';FFMETADATA1\n'+''.join(f"[CHAPTER]\nTIMEBASE=1/1000\nSTART={(rank-1)*30000}\nEND={rank*30000}\ntitle={rank:03} - "+catalog[rank-1]['name'].replace('=','\\=').replace(';','\\;').replace('#','\\#')+'\n'for rank in range(1,101)))
final=out/(engine+"-beatport-50min-4k.mp4")
subprocess.run(["ffmpeg","-v","warning","-y","-f","concat","-safe","0","-i",str(concat),"-i",str(out/"soundtrack.m4a"),"-f","ffmetadata","-i",str(chapters),"-map","0:v","-map","1:a","-map_metadata","2","-map_chapters","2","-c","copy","-movflags","+faststart",str(final)],check=True)
probe=json.loads(subprocess.check_output(["ffprobe","-v","error","-show_format","-show_streams","-show_chapters","-of","json",str(final)]));assert len(probe['chapters'])==100 and abs(float(probe['format']['duration'])-3000)<.1
summary={"state":"complete","engine":engine,"count":100,"file":str(final),"sha256":hashlib.sha256(final.read_bytes()).hexdigest(),"probe":probe,"chapter_records":results};(out/(engine+"-complete.json")).write_text(json.dumps(summary,indent=2)+"\n");status.write_text(json.dumps({"state":"complete","file":str(final),"count":100},indent=2))
# Task-created intermediate clips are removed only after the complete master verifies.
for rank in range(1,101):(folder/f"{rank:03}.mp4").unlink()
print("Verified50-minute native4K master",final,flush=True)
