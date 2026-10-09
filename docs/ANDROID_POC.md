# Beatamp Android PoC

The current public app identity is `nl.neerdael.beatamp`, with Java/JNI implementation
classes retained under `nl.neerdael.presetpoc` for ABI continuity. Launch a new build
using `nl.neerdael.beatamp/nl.neerdael.presetpoc.MainActivity`. The existing
`nl.neerdael.presetpoc` installation, historical scripts and completed videos below
remain the independently identified baseline. Do not relabel their evidence as a
new Beatamp/.beat qualification.

# Android / Android TV PoC

`nl.neerdael.presetpoc` is the standalone Rust/wgpu player on the owned
M4 Pro Android TV emulator (`emulator-5660`). It packages all 100 frozen presets,
170 source-derived authored shader stages and 74 original textures. Primary
feedback, final output and the SurfaceView buffers are native 3840×2160.
Half/quarter/eighth blur intermediates are declared auxiliary effects.

## Build and use

```sh
cargo run -p preset-cli --release -- package-poc
JAVA_HOME=/Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home python3 tools/build-android.py
adb -s emulator-5660 install -r platform/android/app/build/outputs/apk/debug/app-debug.apk
adb -s emulator-5660 shell am start -n nl.neerdael.presetpoc/.MainActivity
```

Offline shader-route artifacts must match their recorded identities. The build
requires Java 17, SDK 35 and NDK 27.3.13750724, verifies package hashes, and stages
original assets/CC0 notices. It uses release Rust inside a debug Android shell.

Capture music requests continuous playback PCM consent. Play music in Milkbeat,
return to this app, then capture. Capture test tones uses a labeled own-UID
AudioTrack fixture. Stop/Home ends capture. Center/Menu toggles the controls;
Left/Right selects presets with the controls hidden. Previous/Next covers all 100.

Toggle 4K adaptation resets preset state and switches substantial native strokes,
energy-preserving stroke coverage and bounded dense-geometry emission. The source
stroke comparison removes these geometry adaptations; it retains the bounded
shader/compiler route and is **not** original-engine equivalence. Adapted strokes
start at three native pixels. All original shape state/RNG invocations execute;
at most 64 uniformly distributed instances per shape emit geometry.

ProjectM test comparison opens the matching original source in the independently
installed `nl.neerdael.presetreference` adapter; Back returns. Build/install with
`:reference:assembleDebug`. This uses an identified unmodified ProjectM-TV AAR,
real clock/RNG and derived mono uint8 test input, not the live stereo capture.
Label comparisons accordingly; no fidelity percentage or deterministic equality
is claimed. Native trails are disabled in this reference profile.

## Rendering and audio

Stable source-resource metadata survives compiler identifier mangling. Authored
passes bind actual native feedback/blur, procedural 2D/3D noise and original
textures. Explicit point/linear clamp/repeat and fixed-function clamp/repeat are
implemented. Textured shapes modulate native feedback using source texture angle
and zoom; plain geometry retains source color. Missing image substitutions are
reported in every frame's resource_substitutions field.

The Rust worker consumes stereo i16 PCM at the negotiated rate, independently
of rendering. It computes stereo envelopes, 1024/4096-point spectra, 64 bands,
bass flux and confidence-bearing chroma, preserving sample/generation identity.
The waveform remains stereo and resets on gaps. The renderer rejects stale
features after 150 ms and uses observed spectral energy for adapted legacy names,
with 40 ms attack / 250 ms release attenuated bands. Both shader volume fields
are means of their respective three bands. Reference normalization, exact
waveform geometry/smoothing, blur range conventions and shader rotation
helper equivalence remain limitations; this is source-derived adaptation.

`milkbeat-pcm/` proves continuous external Milkbeat debug playback into the Rust
analyzer/renderer: 48 kHz stereo i16, independent channels, controlled local WAV
played by Milkbeat's public player manager through an isolated instrumentation
adapter. No Milkbeat production source was modified. Network-provider playlists
and direct decoded float32 integration are separate future integration work.

## Evidence and measurement

The early sweep-60 and visual-captures runs contained a compiler-name resource
alias defect. Their authored visual/shader timing evidence is superseded by
resource-binding-review.json; native allocation observations remain useful.
Do not use those captures as fidelity evidence.

native-detail-v4 records all 100 completing 180 frames with native borders,
textured shapes, source waveform families, 64-band spectral input, native 3px
origin-seed adaptation and cap64 dense geometry. Actual emulator presentation
rates span approximately 59–60 fps across every preset. Hot CPU p50/p95/p99 are
5.18/7.89/12.72 ms. One CPU call exceeded 33.333 ms; five observed presentation
gaps exceeded it (maximum 39.19 ms). Strict zero-miss deadline qualification is
not claimed. See native-detail-v4/summary.json and four contact sheets.
Thirteen early snapshots are low-variation and require moving-audio review;
render success is kept separate from visual acceptance.

corrected-bindings-v2 records all 100 completing 120 native frames with fixed
bindings, clamp sampling and volume ABI. CPU hot-frame p50/p95/p99 were
4.39/14.77/28.95 ms; the maximum was 72.57 ms. These are CPU call timings, not
GPU completion or presentation deadline qualification. A subsequent run adds
textured geometry and faster state binding, targets 60 fps to leave 30-fps
headroom, and records SurfaceFlinger actual presentation timestamps separately.

Run tools/emulator-sweep.py OUTPUT_NAME to collect identified all-100 timing,
SurfaceFlinger latency rows and native 4K JPEG previews. Per-preset first-frame
compilation and capture/switching are separate from hot-frame measurement.
Readback runs after the selected timing sample and is never benchmark throughput.
The legacy 36 ms miss field is a diagnostic boundary, not a strict 33.333 ms gate.
Retain every slow/blank/failed outcome in the fixed denominator. Emulator results
do not establish physical-TV performance; the user deferred physical certification.
