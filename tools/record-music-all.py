#!/usr/bin/env python3
import subprocess,sys,pathlib,json,time
root=pathlib.Path(__file__).resolve().parents[1]
for engine in ["projectm","adapted"]:
    subprocess.run([sys.executable,str(root/"tools/record-music-series.py"),engine],cwd=root,check=True)
(root/"benchmarks/music-review/complete.json").write_text(json.dumps({"state":"complete","files":["projectm-beatport-50min-4k.mp4","adapted-beatport-50min-4k.mp4"],"duration_seconds":3000,"preset_chapters":100,"shared_soundtrack":"soundtrack.m4a","completed_epoch":time.time()},indent=2)+"\n")
print("Both50-minute native4K comparison videos complete",flush=True)
