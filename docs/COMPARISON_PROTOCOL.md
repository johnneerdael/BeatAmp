# Comparison protocol and reference adapters

Status: implementation contract, not an executed capture. Paths beginning with
`../Projectm-TV/` refer to the sibling checkout; all new adapters live here.

## Preserve reference identities

ProjectM-TV is the requested practical visual/behavior reference. Matching it
does not establish original Windows MilkDrop2 equivalence. Its renderer may
already apply resolution adaptation, and those settings belong in the record.
Original sources under `../milkdrop2/src/` are semantic references, not captured
images. Optional original-MilkDrop capture requires its own executable/backend
identity and artifact provenance.

Do not modify or relabel historical frozen workers and evidence. Existing
protocols are design references. New instrumented producers require isolated
source/artifact identities and fresh output directories.

## Existing routes: observed source contracts

| Property | Desktop Preset Lab | Android laboratory core workers |
|---|---|---|
| Reference implementation | Private build of pinned projectM plus app patches | Production JNI via separately instrumented source core AAR |
| Entry | Worker `--job` JSON; `worker.py` consumes stdout | Private instrumentation APK with job/seed/clock bridge |
| PCM | Mono float32, 44,100 Hz | Mono unsigned uint8, 44,100 Hz |
| Frame block at 30 fps | 1,470 samples / 5,880 bytes | 1,470 samples / 1,470 bytes |
| Logical render time | `(frame + 1) / fps` | `frame / 30.0` |
| Main audio window | Latest engine-capacity input window; verify producer | Latest 576 samples per frame in documented Android protocol |
| Typical protocol | Four-second warmup, twenty-second measurement | Frames 0–479: four-second warmup, twelve-second measurement |
| Frame output | Sequential full-resolution top-down RGB8 stream | Portable RGBA readback, rows reversed, RGB8 hashes; bounded PNGs |
| Meaning of speed | Offline host capture, not TV performance | Offline pbuffer/capture, not TV presented-frame performance |

Source references: `../Projectm-TV/tools/preset-lab/src/preset_lab/worker.py`,
`../Projectm-TV/tools/preset-lab/src/preset_lab/models.py`,
`../Projectm-TV/tools/preset-lab/src/preset_lab/native/worker.cpp`,
`../Projectm-TV/tools/preset-lab/src/preset_lab/native/gl_capture.hpp`,
`../Projectm-TV/tools/core-corpus/README.md` and
`../Projectm-TV/docs/superpowers/evidence/upstream-master-4-2/random-100/README.md`.
The unmodified shipping core has uncontrolled clock/RNG. Shipping screenshots
are useful smoke evidence; they cannot support deterministic equality claims.

## New canonical job

The historical uint8 reference profile below is a comparison protocol, not an
accepted live capture tier. The Android PoC must capture continuous higher-
fidelity playback PCM (R17). Preserve that recording and its native rate,
channels, encoding and timing; any deterministic down-conversion supplied to a
legacy reference is an explicitly identified derived artifact.

Create a versioned job schema consumed by all new adapters. Implement the
existing desktop profile first for the development loop, then the canonical
Android qualification profile. The latter deliberately matches the Android
laboratory convention:

- Seed 12345, 30 logical frames/s, 480 frames; time `frame / 30.0`.
- Mono 44,100-Hz unsigned uint8 PCM; exactly 1,470 samples/frame.
- Freeze original PCM bytes, the unsigned-byte decoding rule and the exact
  delivered window. Characterize the JNI decode and audio analyzer before
  implementing the Rust version; do not assume generic PCM normalization.
- Initialize time to zero before preset initialization. Reset every declared
  random subsystem and record its algorithm/stream rules.
- Lock the exact preset and full texture candidate set. Disable random preset
  switching, beat cuts, blank skipping, auto quality and adaptive render sizes.
- Use 48×32 warp mesh as the initial reference convention; record all changes.
- Proposed development profiles: authored-scale 1182×665 and native 3840×2160.
  Record logical/reference canvas dimensions and trails/transition settings
  explicitly. Initial Android Standard profile uses reference 1024×768 and
  Standard trails; confirm actual chosen reference producer's settings.
- RGB8 top-down comparison images, alpha excluded from the image metric.
  Test alpha/blending separately; exclusion is not permission to ignore alpha.

Retain a distinct `desktop-preset-lab-v1` job profile for the existing desktop
clock/float32 encoding. Feed both its reference and Rust candidate the same
profile. Never merge its results into the Android profile. To create canonical
Android-profile desktop references later, use a new isolated instrumented
producer and record its clock/audio/decoder changes. Do not silently shift time
or convert PCM while keeping the old job identity.

Every job records source and asset hashes, converter/IR/runtime versions and
hashes, adapter/instrumentation identity, artifact hashes, input profile, audio
hash/window/rate/encoding, seed and RNG rules, frame times, dimensions, reference
scale, mesh, trails, color format/transfer, sampler behavior, capture backend,
device fingerprint, selected frame indexes and output identities.

## Runs and outputs

Run reference twice and candidate twice with clean preset state. Same-producer
repeatability must be established before interpreting cross-producer differences.
Byte hashes verify repeats where the backend is deterministic; a non-repeatable
producer gets an explicit unstable result and diagnostic replay, not a success
badge. Cross-engine exact equality is not the adaptation acceptance criterion.

Stream frames to bounded comparison processing. Retain all frame hashes and
metrics, plus PNGs at 120, 150, 180, 210, 239, 300, 390 and 479. Produce synchronized
reference/translation/adaptation clips for the measured window. Lossless frames
or lossless clips are the metric inputs; compressed preview videos are for
human inspection. Do not retain all raw 4K frames for the entire matrix.

Report RGB MAE and SSIM at a declared common comparison size, native-resolution
detail crops, luma distribution/bright-area fraction, palette/saturation,
motion/structure and audio-response magnitude/delay. Preserve original frames;
do not align time, normalize brightness or adjust exposure to hide differences.
Brightness ratios for near-black references are undefined. Treat numerical
thresholds as review triggers until development-set calibration has been approved.

Add matched control/intervention audio: silence, stationary bass/mid/treble,
impulses and a frozen mixed clip. Compare unchanged warmup/control prefixes,
intrinsic motion and incremental audio response. Fixed-seed rendered repeats
separate musical response from self-running animation; static band references
alone do not prove audible reactivity.

## Viewer contract

One local viewer selects a preset/job and shows reference, translation-only and
adapted clips on the same timeline. Provide synchronized play/pause/scrub, A/B
toggle, side-by-side, difference view, native-detail crop and metadata/diagnostic
inspection. Display adaptation identity and acceptance state. Save review
decisions outside the immutable corpus, keyed to exact job/package hashes.
Keyboard navigation and labels are sufficient for the first version.

| Job state | Viewer behavior |
|---|---|
| Processing | Show phase/progress and available metadata; unavailable panes remain labeled pending |
| Complete | Enable shared playback/scrub across all required valid panes |
| Partial | Show available panes, missing-pane reasons and the common valid frame range |
| Failed / unsupported | Keep diagnostics and source spans openable; show explicit unavailable output panes |
| Unstable / unverified | Show evidence with its status; prevent saving a qualifying acceptance as if verified |

Restrict synchronized playback to the common valid frame range of the displayed
available clips and show that range. Disable synchronization when no common
range exists. Missing/unavailable feature traces, including chroma, stereo and
source timing, have labeled placeholders; they are not zero-valued measurements.

Minimal review flow: select preset/job and intervention → inspect synchronized
clips/features → record visual, bass and melodic outcomes independently → save
with rationale and exact source/package/job identities. Add an intentionally
absent/not-applicable outcome requiring a reason. Input quality and performance
are separate evidence-derived statuses and cannot be overwritten by subjective
visual approval. Intervention selection changes every pane/plot together.
Provide visible focus and keyboard access to intervention choice, outcome fields
and Save; avoid a separate editor/dashboard.

## Performance is a separate protocol

PoC target override: the user requests M4-hosted Android TV emulation and no
physical TV. Apply the timing/resolution/readback separation below to a dedicated
4K TV AVD, record host/emulator/backend identity and frame misses, and label all
results as emulator measurements. Physical-TV qualification remains deferred.
Visual/audio fidelity and performance take priority over exact likeness. Native
4K feedback and substantial native-pixel strokes remain mandatory.

Benchmark a release build on a named physical TV with actual native surfaces,
3840×2160 primary feedback and final composite. Remove comparison readback,
hashing, video encoding and synthetic simulation clocks. Use wall-clock audio,
real presentation and a fixed declared 30-fps pacing policy.

Record CPU critical-path, GPU timing when available, presented-frame intervals,
deadline misses, queue stalls, memory, thermal behavior, resolution/settings and
switch/transition costs. Query timing capabilities; missing GPU timestamps are
explicitly unavailable, not zero. Average FPS cannot conceal missed deadlines.
Certification conditions and durations are specified in PLAN.md.
