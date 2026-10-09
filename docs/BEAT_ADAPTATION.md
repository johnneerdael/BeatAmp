# Programmatic native adaptation

The current authoring compiler composes five independently implemented procedural
components into a native wgpu graph. Predictions guide bounded artistic targets;
source hashes select reproducible compositions and palettes. This is an initial
reinterpretation vocabulary, not recovery of executable geometry from47 statistics.
Exact source translation remains the separately selectable diagnostic engine.

Ready predictions live in the user's
`/Users/jneerdael/Downloads/ProjectM-TV-preset-corpus-15fps-480p-memory-fixed`.
The producer checkout is `../Projectm-TV/.worktrees/predictor-memory-repair`.
Never change, rerun or import its workers. The independent auditor reads schemas,
source bytes, prepared inputs, run identity and record digests using the producer's
Python JSON hash convention. Rust retains original prediction bytes and rechecks
their file hashes; it does not substitute serde serialization for that convention.

```sh
uv run --with jsonschema python tools/audit-predictions.py --input /Users/jneerdael/Downloads/ProjectM-TV-preset-corpus-15fps-480p-memory-fixed --producer ../Projectm-TV/.worktrees/predictor-memory-repair --output reports/predictor-inspection/65da6947857df106
cargo run -p preset-cli -- beat-adapt-ready --audit reports/predictor-inspection/65da6947857df106/inspection.json --output effects/beat/prediction-pilot-65da6947857df106
cargo run -p preset-cli -- beat-adapt-corpus --audit reports/predictor-inspection/65da6947857df106/inspection.json --output effects/beat/frozen-native-pilot
python3 tools/build-beatamp.py
```

The snapshot audited on2026-10-09 contained43 computed records with no integrity
errors. Those records have no overlap with the frozen100. The two pilots retain
separate denominators. Unpredicted frozen sources are explicitly pending and
receive source-guided artistic scenes, not invented computed evidence. Terminal
failures and authoring errors remain visible in catalogs. Future records require
a fresh audit. Use a new output directory for a different run identity.

The archive carries WGSL, graph/resource declarations, control defaults and
adaptation metadata. Playback needs no `.milk` files, predictor or source parser.
The Android runtime depends on `beat-authoring` with default features disabled;
only its full-PCM controller is included. The offline CLI enables source import.

```sh
cargo run -p preset-cli -- beat-render --input effects/beat/frozen-native-pilot/001.beat --pcm benchmarks/music-review/playlist.s16le --rate 44100 --channels 2 --frames 120 --output benchmarks/beat-pilot/001-native.png
adb -s emulator-5660 install -r platform/android/app/build/outputs/apk/debug/app-debug.apk
adb -s emulator-5660 shell am start -n nl.neerdael.beatamp/nl.neerdael.presetpoc.MainActivity --ez native_beat true --ez render_fixture true
```

The PCM file is local licensed music and
is not bundled or published. Preview output is lossless native3840×2160 PNG.
It is an offline render with final-frame readback, not a presentation benchmark.
The native Android path accepts live playback PCM, explicit synthetic stereo
control and full-PCM playlist replay. It rejects the older limited-feature replay.
For playlist mode, place unnormalized44.1kHz stereo PCM16LE at
`filesDir/beat/playlist.s16le`; input offsets are expressed in30fps frames.

The controller delivers independent1024/4096 FFT channels, including DC and actual
Nyquist, through exact-size storage buffers. Fast controls do not wait for detail
analysis. Generation resets, timestamp/frame ordering, source availability and
stale-input damping are explicit. See `crates/beat-authoring/README.md` for the ABI.

Native color/feedback render targets are allocated at the actual output extent.
The model and renderer report logical resource sizes; Android reports the acquired
surface extent separately. CPU submission/call intervals do not prove presented
frame deadlines. Premium texture integration and emulated4K/30 measurement are
still pending; generated packages are not automatically visually accepted.

The adapted Android interface retains ProjectM-TV's D-pad conventions and copied
UI notices under LGPL2.1. Source/Adapted switching applies to the diagnostic route;
the independent `.beat` route exposes its actual adaptation identity.
