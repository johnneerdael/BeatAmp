#!/usr/bin/env python3
"""Capture native chapters into a verified append-only MP4 to bound disk use."""
import hashlib
import json
import os
import pathlib
import queue
import signal
import shutil
import subprocess
import sys
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "benchmarks/music-review"
ENGINE = sys.argv[1]
assert ENGINE in ["adapted", "projectm"]
FOLDER = OUT / ENGINE
PIPE_DIR = OUT / (ENGINE + "-mux-pipes")
PIPE_DIR.mkdir(exist_ok=True)
PARTIAL = OUT / (ENGINE + "-beatport-50min-4k.partial.mp4")
FINAL = OUT / (ENGINE + "-beatport-50min-4k.mp4")
if "--verify-only" in sys.argv:
    subprocess.run([sys.executable,str(ROOT / "tools/verify-music-master.py"),ENGINE], check=True)
    complete_path=OUT / (ENGINE + "-complete.json")
    summary=json.loads(complete_path.read_text());summary["state"]="complete"
    complete_path.write_text(json.dumps(summary,indent=2)+"\n")
    last=FOLDER / "100.mp4"
    if last.exists():last.unlink()
    (OUT / (ENGINE + "-status.json")).write_text(json.dumps({"state":"complete","count":100,"file":str(FINAL)},indent=2))
    raise SystemExit(0)
assert not PARTIAL.exists() and not FINAL.exists(), "Refuse to overwrite a movie"


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def packets(path):
    return json.loads(subprocess.check_output([
        "ffprobe", "-v", "error", "-select_streams", "v:0", "-show_packets",
        "-show_data_hash", "sha256", "-show_entries",
        "packet=pts_time,dts_time,duration_time,data_hash", "-of", "json", str(path),
    ]))["packets"]


pipes = [PIPE_DIR / f"{rank:03}.pipe" for rank in range(1, 101)]
for pipe in pipes:
    if pipe.exists():
        pipe.unlink()
    os.mkfifo(pipe)
concat = PIPE_DIR / "concat.txt"
concat.write_text("".join(f"file '{pipe}'\nduration 30\n" for pipe in pipes))
ready = queue.Queue(maxsize=2)
cancel = threading.Event()
capture_lock = threading.Lock()
capture_process = None
records = []
expected = []


def produce():
    global capture_process
    try:
        for rank in range(1, 101):
            if cancel.is_set():
                return
            video = FOLDER / f"{rank:03}.mp4"
            meta = video.with_suffix(".json")
            row = json.loads(meta.read_text()) if meta.exists() else None
            if not (row and row["rank"] == rank and row["frames"] == 900
                    and video.exists() and digest(video) == row["sha256"]):
                with capture_lock:
                    if cancel.is_set():return
                    capture_process = subprocess.Popen([
                        sys.executable, str(ROOT / "tools/record-music-chapter.py"), ENGINE, str(rank)
                    ], cwd=ROOT, start_new_session=True)
                    child = capture_process
                code = child.wait()
                if cancel.is_set():return
                if code:raise subprocess.CalledProcessError(code, child.args)
                with capture_lock:capture_process = None
                row = json.loads(meta.read_text())
            assert row["input_start_frame"] == (rank - 1) * 900
            encoded = packets(video)
            assert len(encoded) == 900
            while not cancel.is_set():
                try:
                    ready.put((rank, row, encoded), timeout=.5)
                    break
                except queue.Full:
                    pass
    except BaseException as error:
        while not cancel.is_set():
            try:
                ready.put(error, timeout=.5)
                break
            except queue.Full:
                pass


def verify_prefix(count, encoded):
    assert len(encoded) >= count, (len(encoded), count)
    for index, actual in enumerate(encoded[:count]):
        source = expected[index]
        assert actual["data_hash"] == source["data_hash"], index
        assert abs(float(actual["pts_time"]) - float(source["pts_time"]) - (index // 900) * 30) < .00001, index
        assert abs(float(actual["dts_time"]) - float(source["dts_time"]) - (index // 900) * 30) < .00001, index


log = (OUT / (ENGINE + "-assembly.log")).open("wb")
mux = subprocess.Popen([
    "ffmpeg", "-v", "warning", "-f", "concat", "-safe", "0", "-i", str(concat),
    "-i", str(OUT / "soundtrack.m4a"), "-f", "ffmetadata", "-i", str(OUT / "chapters.ffmetadata"),
    "-map", "0:v:0", "-map", "1:a:0", "-map_chapters", "2", "-map_metadata", "2",
    "-c", "copy", "-avoid_negative_ts", "disabled", "-movflags",
    "+frag_keyframe+delay_moov+default_base_moof", "-flush_packets", "1", str(PARTIAL),
], stdout=log, stderr=log)
producer = threading.Thread(target=produce, daemon=True)
producer.start()
try:
    for rank, pipe in enumerate(pipes, 1):
        while True:
            if mux.poll() is not None:
                raise RuntimeError(f"Mux exited: {mux.returncode}")
            try:
                item = ready.get(timeout=.5)
                break
            except queue.Empty:
                pass
        if isinstance(item, BaseException):
            raise item
        captured_rank, row, encoded = item
        assert captured_rank == rank
        records.append(row)
        expected.extend(encoded)
        # Open without blocking forever if the muxer has failed.
        while True:
            try:
                fd = os.open(pipe, os.O_WRONLY | os.O_NONBLOCK)
                os.set_blocking(fd, True)
                break
            except OSError as error:
                if error.errno != 6:  # ENXIO: reader not yet waiting.
                    raise
                if mux.poll() is not None:
                    raise RuntimeError("Mux failed before opening chapter")
                time.sleep(.1)
        with os.fdopen(fd, "wb") as sink, (FOLDER / f"{rank:03}.mp4").open("rb") as source:
            shutil.copyfileobj(source, sink)
        if rank > 1:
            deadline = time.monotonic() + 30
            while True:
                time.sleep(.2)
                prefix = packets(PARTIAL)
                if len(prefix) >= (rank - 1) * 900:
                    break
                assert time.monotonic() < deadline, "Mux prefix did not flush"
            verify_prefix((rank - 1) * 900, prefix)
            previous = FOLDER / f"{rank-1:03}.mp4"
            meta = previous.with_suffix(".json")
            verified_row = json.loads(meta.read_text())
            verified_row["assembled_packet_payloads_and_timestamps_verified"] = True
            meta.write_text(json.dumps(verified_row, indent=2) + "\n")
            previous.unlink()
        (OUT / (ENGINE + "-status.json")).write_text(json.dumps({
            "state": "recording", "engine": ENGINE, "completed": rank, "count": 100,
            "assembly": "fragmented MP4; verified prefixes retained; no video/audio re-encode",
        }, indent=2))
        print(f"{ENGINE}: {rank}/100 captured; {max(0,rank-1)*900} muxed packets verified", flush=True)
    assert mux.wait(timeout=60) == 0
    verify_prefix(90000, packets(PARTIAL))
    probe = json.loads(subprocess.check_output([
        "ffprobe", "-v", "error", "-count_packets", "-show_streams", "-show_format",
        "-show_chapters", "-of", "json", str(PARTIAL),
    ]))
    video = next(s for s in probe["streams"] if s["codec_type"] == "video")
    assert (video["width"], video["height"], int(video["nb_read_packets"])) == (3840,2160,90000)
    assert abs(float(video["duration"]) - 3000) < .001 and len(probe["chapters"]) == 100
    PARTIAL.rename(FINAL)
    summary = {"state": "assembled-pending-verification", "engine": ENGINE, "count": 100, "file": str(FINAL),
               "sha256": digest(FINAL), "probe": probe, "chapter_records": records,
               "packet_payloads_and_presentation_timestamps_verified": 90000,
               "assembly": "Fragmented MP4; intermediate chapters deleted only after matching verified master prefix"}
    (OUT / (ENGINE + "-complete.json")).write_text(json.dumps(summary, indent=2)+"\n")
    subprocess.run([sys.executable,str(ROOT / "tools/verify-music-master.py"),ENGINE], check=True)
    summary["state"]="complete"
    (OUT / (ENGINE + "-complete.json")).write_text(json.dumps(summary, indent=2)+"\n")
    (FOLDER / "100.mp4").unlink()
    (OUT / (ENGINE + "-status.json")).write_text(json.dumps({"state":"complete","count":100,"file":str(FINAL)},indent=2))
    print("Verified50-minute native4K master",FINAL,flush=True)
except BaseException as error:
    cancel.set()
    cleanup_errors=[]
    with capture_lock:
        child = capture_process
    if child is not None:
        # An exited group leader can still have a live encoder descendant.
        try:os.killpg(child.pid,signal.SIGTERM)
        except ProcessLookupError:pass
        except OSError as cleanup_error:cleanup_errors.append(str(cleanup_error))
        try:child.wait(timeout=10)
        except subprocess.TimeoutExpired:pass
        try:os.killpg(child.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        except OSError as cleanup_error:cleanup_errors.append(str(cleanup_error))
        try:child.wait(timeout=10)
        except subprocess.TimeoutExpired:cleanup_errors.append("Capture child did not exit")
    producer.join(timeout=10)
    try:
        stopped=subprocess.run(["adb","-s","emulator-5660","shell","am","force-stop",
                     "nl.neerdael.presetpoc" if ENGINE=="adapted" else "nl.neerdael.presetreference"],timeout=10)
        if stopped.returncode:cleanup_errors.append("Owned app cleanup failed")
    except (OSError,subprocess.TimeoutExpired) as cleanup_error:cleanup_errors.append(str(cleanup_error))
    try:
        mux.terminate()
        try:mux.wait(timeout=30)
        except subprocess.TimeoutExpired:mux.kill();mux.wait(timeout=10)
    except (OSError,subprocess.TimeoutExpired) as cleanup_error:cleanup_errors.append(str(cleanup_error))
    (OUT / (ENGINE + "-status.json")).write_text(json.dumps({"state":"failed","engine":ENGINE,"reason":str(error),"captured":len(records),"cleanup_errors":cleanup_errors},indent=2))
    complete_path=OUT / (ENGINE + "-complete.json")
    if complete_path.exists():
        failed=json.loads(complete_path.read_text());failed["state"]="failed-verification"
        complete_path.write_text(json.dumps(failed,indent=2)+"\n")
    raise
finally:
    cancel.set()
    log.close()
