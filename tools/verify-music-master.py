#!/usr/bin/env python3
"""Verify an assembled comparison movie without changing its frames or audio."""
import argparse
import hashlib
import json
import pathlib
import subprocess
from fractions import Fraction


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def audio_digest(path):
    return subprocess.check_output([
        "ffmpeg", "-v", "error", "-i", str(path), "-map", "0:a:0",
        "-c", "copy", "-f", "hash", "-hash", "sha256", "-",
    ], text=True).strip().split("=", 1)[1]


def decoded_audio_digest(path):
    return subprocess.check_output([
        "ffmpeg", "-v", "error", "-i", str(path), "-map", "0:a:0", "-t", "3000",
        "-c:a", "pcm_s16le", "-f", "hash", "-hash", "sha256", "-",
    ], text=True).strip().split("=", 1)[1]


def audio_start(path):
    result = json.loads(subprocess.check_output([
        "ffprobe", "-v", "error", "-select_streams", "a:0", "-read_intervals", "%+#2",
        "-show_packets", "-of", "json", str(path),
    ]))["packets"]
    return [{k: p.get(k) for k in ["pts_time", "dts_time", "duration_time", "side_data_list"]} for p in result]


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("engine", choices=["projectm", "adapted"])
parser.add_argument("--manifest", type=pathlib.Path)
parser.add_argument("--report", type=pathlib.Path)
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[1]
folder = root / "benchmarks/music-review"
complete = json.loads((args.manifest or folder / f"{args.engine}-complete.json").read_text())
movie = pathlib.Path(complete["file"])
probe = json.loads(subprocess.check_output([
    "ffprobe", "-v", "error", "-count_packets", "-show_streams",
    "-show_format", "-show_chapters", "-of", "json", str(movie),
]))
video = next(s for s in probe["streams"] if s["codec_type"] == "video")
audio = next(s for s in probe["streams"] if s["codec_type"] == "audio")
assert (video["width"], video["height"]) == (3840, 2160)
assert video["r_frame_rate"] == "30/1"
assert int(video.get("nb_frames", video["nb_read_packets"])) == int(video["nb_read_packets"]) == 90000
assert abs(float(video["duration"]) - 3000) < .001
# ffprobe prints seconds rounded to six decimals; compare exact track ticks.
audio_duration = int(audio["duration_ts"]) * Fraction(audio["time_base"])
assert abs(audio_duration - 3000) <= Fraction(1024, 44100)
assert abs(float(video["start_time"])) < .00001
assert abs(float(audio["start_time"])) < .00001
assert audio["sample_rate"] == "44100" and audio["channels"] == 2
assert len(probe["chapters"]) == len(complete["chapter_records"]) == 100
for rank, (chapter, record) in enumerate(zip(
    probe["chapters"], complete["chapter_records"], strict=True
), 1):
    assert abs(float(chapter["start_time"]) - (rank - 1) * 30) < .001
    assert abs(float(chapter["end_time"]) - rank * 30) < .001
    assert record["engine"] == args.engine and record["rank"] == rank
    assert record["input_start_frame"] == (rank - 1) * 900
    assert record["frames"] == 900 and record["dimensions"] == [3840, 2160]
    captured = next(s for s in record["probe"]["streams"] if s["codec_type"] == "video")
    assert int(captured["nb_read_frames"]) == 900  # Decoded before assembly.
movie_hash = digest(movie)
assert movie_hash == complete["sha256"]
soundtrack_hash = audio_digest(folder / "soundtrack.m4a")
assert audio_digest(movie) == soundtrack_hash
assert audio_start(movie) == audio_start(folder / "soundtrack.m4a")
decoded_hash = decoded_audio_digest(folder / "soundtrack.m4a")
assert decoded_audio_digest(movie) == decoded_hash
report = {
    "state": "verified", "engine": args.engine, "file": str(movie),
    "sha256": movie_hash, "native_dimensions": [3840, 2160],
    "video_packets": 90000, "encoded_fps": 30, "duration_seconds": 3000,
    "chapters": 100, "chapter_input_offsets_verified": True,
    "chapter_decoded_frames_verified_before_assembly": 90000,
    "audio_packet_payload_sha256": soundtrack_hash,
    "audio_identical_to_shared_soundtrack": True,
    "decoded_3000_second_audio_sha256": decoded_hash,
    "audio_timeline_and_priming_match": True,
    "audio_metadata_duration_excess_seconds": float(audio_duration - 3000),
    "scope": "Compressed comparison recording; not playback timing certification",
}
(args.report or folder / f"{args.engine}-verification.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
