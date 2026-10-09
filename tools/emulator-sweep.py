#!/usr/bin/env python3
"""Run identified native-4K PoC evidence on the owned TV AVD only."""
import hashlib,json,pathlib,re,shlex,subprocess,time,sys
root=pathlib.Path(__file__).resolve().parents[1]
out=root/"benchmarks/emulator/native-poc"/sys.argv[1];out.mkdir(exist_ok=True)
adb=["adb","-s","emulator-5660"]
def call(*args):return subprocess.check_output(adb+list(args)).decode()
base=json.loads((root/"benchmarks/emulator/native-poc/visual-sweep-identity.json").read_text())
base.update({"producer":"native-borders-wave-families-spectrum-seed-v4","input":"explicit kick/note stereo control; not captured PCM","frames_per_preset":180,"target_fps":60,"apk_sha256":hashlib.sha256((root/"platform/android/app/build/outputs/apk/debug/app-debug.apk").read_bytes()).hexdigest(),"catalog_sha256":hashlib.sha256((root/"reports/poc-catalog.json").read_bytes()).hexdigest(),"started_epoch":time.time(),"capture":"native RGBA8 readback JPEG92 after timing; excluded from performance samples"})
(out/"identity.json").write_text(json.dumps(base,indent=2)+"\n")
call("shell","am","force-stop","nl.neerdael.presetpoc");call("logcat","-c")
print(call("shell","am","start","-n","nl.neerdael.presetpoc/.MainActivity","--ei","rank","1","--ez","render_fixture","true","--ez","hide_controls","true","--ei","sweep_frames","180","--ez","capture_sweep","true"),flush=True)
latencies=set();previous=0;deadline=time.monotonic()+900
while time.monotonic()<deadline:
    lines=call("logcat","-d","-v","raw","-s","PresetSweep","PresetCapture","PresetRender").splitlines();rows=[]
    for line in lines:
        try:rows.append(json.loads(line))
        except ValueError:pass
    complete={r.get("rank")for r in rows if r.get("frame")==180}
    if len(complete)!=previous:print("Completed",len(complete),"/100",flush=True);previous=len(complete)
    listing=call("shell","dumpsys","SurfaceFlinger","--list")
    layer=next((m.group(1)for line in listing.splitlines() if (m:=re.search(r"RequestedLayerState\{(.+SurfaceView\[nl.neerdael.presetpoc/.+?\]\(BLAST\)#\d+)",line))),None)
    if layer:
        raw=call("shell","dumpsys SurfaceFlinger --latency "+shlex.quote(layer))
        for line in raw.splitlines()[1:]:
            try:row=tuple(map(int,line.split()))
            except ValueError:continue
            if len(row)==3 and all(0<v<2**63-1 for v in row):latencies.add(row)
    if len(complete)==100:
        # Native destruction and buffered export occur after the final log.
        # Read only a complete export; a terminal log is not a flush barrier.
        timings=call("exec-out","run-as","nl.neerdael.presetpoc","cat","files/sweep-timing.jsonl")
        try: exported=[json.loads(line)for line in timings.splitlines()]
        except ValueError:
            time.sleep(0.2);continue
        if len(exported)!=18_000:
            time.sleep(0.2);continue
        (out/"logs.jsonl").write_text("\n".join(json.dumps(r)for r in rows)+"\n")
        (out/"timing.jsonl").write_text(timings)
        (out/"surface-latency.json").write_text(json.dumps({"columns":["desired_present_ns","actual_present_ns","frame_ready_ns"],"rows":sorted(latencies)},indent=2)+"\n")
        with (out/"captures.tar").open("wb")as f:subprocess.run(adb+["exec-out","run-as","nl.neerdael.presetpoc","tar","-C","files/captures","-cf","-","."],stdout=f,check=True)
        print("Terminal full-corpus evidence saved",flush=True);break
    time.sleep(2)
else:raise SystemExit("Sweep observation deadline elapsed; inspect live process before restarting")
