#!/usr/bin/env python3
"""Native4K ordered frame capture. Music/sample schedule is shared; not a benchmark."""
import argparse,json,pathlib,subprocess,time,hashlib,fcntl
root=pathlib.Path(__file__).resolve().parents[1];parser=argparse.ArgumentParser();parser.add_argument("engine",choices=["adapted","projectm"]);parser.add_argument("rank",type=int);parser.add_argument("--frames",type=int,default=900);parser.add_argument("--offset",type=int);parser.add_argument("--output");parser.add_argument("--gl",action="store_true");parser.add_argument("--port",type=int,default=26101);args=parser.parse_args()
capture_lock=(root/"benchmarks/music-review/capture.lock").open("a");fcntl.flock(capture_lock,fcntl.LOCK_EX)
assert 1<=args.rank<=100 and 1<=args.frames<=900
offset=args.offset if args.offset is not None else(args.rank-1)*900
out=root/"benchmarks/music-review"/args.engine;out.mkdir(exist_ok=True);video=pathlib.Path(args.output)if args.output else out/f"{args.rank:03}.mp4"
log=video.with_suffix(".ffmpeg.log");adb=["adb","-s","emulator-5660"];pkg="nl.neerdael.presetpoc"if args.engine=="adapted"else"nl.neerdael.presetreference";activity="MainActivity"if args.engine=="adapted"else"ReferenceActivity"
for owned in ["nl.neerdael.presetpoc","nl.neerdael.presetreference"]:subprocess.run(adb+["shell","am","force-stop",owned],check=True)
subprocess.run(adb+["logcat","-c"],check=True)
command=["ffmpeg","-v","warning","-y","-f","mjpeg","-probesize","20000000","-framerate","30","-i",f"tcp://127.0.0.1:{args.port}?listen=1"]
if args.engine=="projectm"or args.gl:command += ["-vf","vflip"]
command += ["-an","-c:v","hevc_videotoolbox","-allow_sw","0","-b:v","24000000","-pix_fmt","yuv420p","-tag:v","hvc1","-movflags","+faststart",str(video)]
started=time.time()
with log.open("wb")as logs:
 process=subprocess.Popen(command,stdout=logs,stderr=logs);time.sleep(1)
 launch=adb+["shell","am","start","-n",pkg+"/."+activity,"--ei","rank",str(args.rank),"--ei","record_frames",str(args.frames),"--ei","record_port",str(args.port),"--ei","input_start_frame",str(offset)]
 if args.engine=="adapted":launch += ["--ez","render_playlist","true","--ez","hide_controls","true","--ez","record_gl",str(args.gl).lower()]
 subprocess.run(launch,check=True)
 deadline=time.monotonic()+max(180,args.frames/3)
 while process.poll()is None and time.monotonic()<deadline:
  time.sleep(1)
  recent=subprocess.check_output(adb+["logcat","-d","-v","raw","-s","PresetMovie","PresetRender"]).decode()
  if '"state":"failed"'in recent or "Recording failed"in recent or "Recorder failed"in recent:
   process.terminate();process.wait();raise SystemExit("Renderer recording failed: "+recent[-2000:])
 if process.poll()is None:process.terminate();raise SystemExit("Capture observer deadline: inspect live renderer before retry")
 if process.returncode:raise SystemExit("Host encoder failed: "+log.read_text()[-2000:])
 raw=subprocess.check_output(adb+["logcat","-d","-v","raw","-s","PresetMovie","PresetRender","PresetReference"]).decode();video.with_suffix(".jsonl").write_text(raw)
probe=json.loads(subprocess.check_output(["ffprobe","-v","error","-count_frames","-show_streams","-show_format","-of","json",str(video)]));stream=next(s for s in probe['streams']if s['codec_type']=='video')
assert (stream['width'],stream['height'],int(stream['nb_read_frames']))==(3840,2160,args.frames),stream
metadata={"engine":args.engine,"rank":args.rank,"input_start_frame":offset,"frames":args.frames,"source_pcm_start_seconds":offset/30,"duration_seconds":args.frames/30,"dimensions":[3840,2160],"encoded_fps":30,"wall_seconds":time.time()-started,"capture_backend":"nativeRGBA8/JPEG92 ordered TCP -> Mac hardwareHEVC; reference vertically reversed on host","playback_benchmark":False,"reference_clock_rng":"uncontrolled shipping core; matching music does not imply pixel equality","sha256":hashlib.sha256(video.read_bytes()).hexdigest(),"probe":probe}
video.with_suffix(".json").write_text(json.dumps(metadata,indent=2)+"\n");print(json.dumps({k:metadata[k]for k in ['engine','rank','frames','wall_seconds','dimensions']}),flush=True)
