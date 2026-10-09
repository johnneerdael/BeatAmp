#!/usr/bin/env python3
"""Check recording identities, native allocations and audio positions in traces."""
import argparse
import json
import pathlib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--partial", action="store_true")
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parents[1]
folder = root / "benchmarks/music-review"
catalog = json.loads((root / "reports/poc-catalog.json").read_text())["presets"]
report = {"scope": "Capture trace integrity; no visual rating or playback performance claim", "engines": {}}
for engine in ["projectm", "adapted"]:
    checked = []
    for rank, preset in enumerate(catalog, 1):
        path = folder / engine / f"{rank:03}.jsonl"
        if not path.exists() and args.partial:
            continue
        rows = []
        for line in path.read_text().splitlines():
            if line.startswith("{"):
                rows.append(json.loads(line))
        samples = [r for r in rows if r.get("state") == "rendering"] if engine == "adapted" else [r for r in rows if "current" in r]
        assert samples, (engine, rank, "missing trace samples")
        for row in samples:
            assert row["rank"] == rank, (engine, rank)
            assert 1 <= row["frame"] <= 900, (engine, rank)
            if engine == "adapted":
                assert row["preset"] == preset["name"], rank
                assert row["native_allocations"] == [[3840,2160]] * 3, rank
                assert row["surface"] == [3840,2160], rank
                assert row["input"] == "Beatport-lossless-source-stereo-PCM-replay", rank
                assert row["input_source_frame"] == (rank-1)*900+row["frame"]-1, rank
                assert not row["resource_substitutions"], (rank, row["resource_substitutions"])
                assert row["recording"] and not row["playback_benchmark_eligible"], rank
                assert row["graphics_backend"] == "Vulkan", rank
                assert 0 <= row["comparison_analysis_lag_samples"] <= 255, rank
            else:
                assert row["source"] == preset["name"] and row["current"].endswith(preset["name"]), rank
                assert row["surface"] == "3840x2160", rank
                assert row["input"].startswith("Beatport same44100 stereo source"), rank
        checked.append({"rank": rank, "source_id": preset["source_id"], "trace_samples": len(samples)})
    if not args.partial:
        assert len(checked) == 100
    report["engines"][engine] = {"count": len(checked), "chapters": checked}
report["state"] = "partial-verified" if args.partial else "verified"
(folder / ("trace-partial-verification.json" if args.partial else "trace-verification.json")).write_text(json.dumps(report, indent=2)+"\n")
print(json.dumps({"state": report["state"], "counts": {engine: details["count"] for engine, details in report["engines"].items()}}))
