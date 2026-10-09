# Implementation ledger — plan: PLAN.md

2026-10-08: Workspace verified: planning documents and frozen corpus only;
no Git repository exists in this directory. No implementation processes were
started before the user's input correction.

Ruling: execute in this directory without Git worktree/commit steps because this
is the supplied non-Git workspace. Keep durable verification records here.

Ruling: R17 supersedes all earlier Visualizer-provider acceptance wording.
Continuous higher-fidelity playback PCM is required. Platform consent and
capture-policy failures remain visible; no microphone or Visualizer fallback.

Ruling: use the existing written design and plan under the user's autonomous
authorization, without additional approval rounds. Offline procedural conversion
and explicit 4K adaptation are authorized; all 100 frozen presets remain in scope.

U0/U1/U8 in progress: implement PCM AudioRecord capture and Rust analysis before
renderer work. Initial target: connected AM9 Pro, Android 14; hardware and actual
capture qualification remain unproven. No native 4K/30 claim exists yet.

2026-10-08 progress:
- Corpus verification implemented: frozen manifest/population identity, seeded
  sampling ranks and 80/20 split, every preset/texture hash, safe paths and exact
  inventory. Parser maps sparse ordered code sections, Unicode/legacy encoding,
  custom scopes, duplicate diagnostics and original shader bodies. All 100 parse.
- Inspector writes `reports/inspect.json`; 100 inspected, 0 converted, 0 rendered,
  0 visually/audio accepted, 0 4K30 qualified. Failed/pending states stay explicit.
- Rust analyzer/worker implemented with planned 1024/4096 FFTs, 256-frame fast
  RMS, 64 bands with actual bin counts, stereo power/envelopes, bass flux and
  confidence-bearing chroma. No legacy analyzer equivalence or tempo claim.
- Android independent shell builds. Controlled stereo PCM capture verified on
  AM9 Pro (97.47 s v1, 158.63 s v2). Producer binaries/identities saved. Home stops
  capture. No drop occurred in either trace; CPU ~3.8% in capture-only tests.
- Device evidence found low-note chroma binning error; failed regression test
  fixed by interpolated spectral peaks. Nonaligned discontinuity detail cadence
  also has a failed-then-passing regression. Later DC-offset spectral correction
  passes unit tests but is not part of the frozen v2 physical trace.
- CLI output containment tests prohibit symlink writes and directory creation
  in frozen corpus. Manifest identity includes attribution and all source IDs.
- User identified their owned `johnneerdael/Milkbeat` repo as the intended player.
  Verified its current source already receives continuous Media3 i16/f32 PCM
  before its mono visualizer downmix. Direct stereo host integration is preferred;
  planning notes in `docs/MILKBEAT_INTEGRATION.md`. Sibling sources unchanged.

Next: ordered bounded equation IR/VM and semantic controls; measured legacy HLSL
route; native wgpu translation/adaptation renderer; direct Milkbeat PCM integration;
comparisons and physical 4K qualification across all 100. U0/U1/U3/U8 are partial;
U2/U4–U7/U9 remain incomplete. This is meaningful implementation progress, not
goal completion or a blocker. Capture evidence does not establish 4K30 success.

Verification checkpoint: `cargo test --workspace` passes 20 Rust tests;
`cargo clippy --workspace --all-targets -- -D warnings` and rustfmt check pass.
Android build script passes four JVM tests and produces the arm64 APK, installed
on the AM9 Pro. Frozen v1/v2 trace hashes were checked against saved identities.
The v2 producer and binary remain unchanged after later DC/diagnostic fixes.
Inspector was rerun against the unchanged corpus: 100 parsed, zero converted or
rendered. No live capture/service/process was left running after physical tests.

2026-10-08 latest steering: prioritize fidelity/performance over exact likeness,
avoid thin 1px lines and any low-resolution authored feedback canvas. Use the
user's ProjectM-TV Android app as visual/performance reference. The user then
selected emulated Android TV on their M4 Mac; physical-TV certification is
deferred and subsequent work must use an isolated emulator. Requirements R7/R18
and the plan's override section record the current completion criteria.

Equation progress: all 744 equation sections (94,184 bytecode instructions)
compile across all 100; all 100 pass a four-frame host state/mesh/wave/shape smoke
with fixed seed and synthetic controls. Generated equation artifacts are marked
non-playable pending shader/effect-graph compilation. Star Forge's authored
200,000-cell clear loop exposed an initialization/per-frame budget mixup; a
failed-then-passing real-preset regression now preserves the 8M initialization
allowance and 1M per-frame invocation limit. No shader/render/4K30 success claim.

Emulator setup: created owned AVD `PresetPocTV4K_API36`, `emulator-5660`, exec
session 62511. Boot complete; M4 Pro host GPU is visible through GLES/Metal and
Vulkan. Native display 3840×2160, but TV UI override remains 1920×1080 even after
wm commands; require an explicit 4K native app surface. Device profile saved.
Do not restart merely because a tool observation times out; inspect live device
and process/session first. Other emulators/physical devices are unrelated.

Current checks: 31 Rust tests pass, including all-corpus equation source mapping,
scope reset/persistence and Star Forge initialization. Clippy passed before the
last budget/CLI identity edits and will be rerun. Equation artifacts and report
include compiler executable/lock/source identity; short smoke 100/100. The extended
480-frame sweep is running as exec session 86914; poll that handle before deciding
whether to restart. Renderer, shaders, native stroke geometry and Milkbeat stereo
integration remain incomplete; the goal remains active with real progress.

Extended sweep finished: exec session 86914 exited successfully, 100/100 equation
bundles compiled and 100/100 passed 480 frames with enabled custom waves/shapes
and the full 48×32-cell mesh. `reports/equations-480.json` records 223.04 total host
wall-clock seconds, peak frame instructions 1,060,041, max equation RAM 3,211,264 bytes and
explicit nonfinite assignment cleanups. All 100 current artifact hashes and
non-playable flags checked. Final clippy and 31-test workspace run pass. The
emulator remains live (PID 84217 at last check); shader/render work is next.


2026-10-08 shader/native renderer progress:
- Added wgpu/Naga 30.0.1, portable shader binding metadata and native RGBA8
  feedback/output. GPU test verifies both histories/final at 3840×2160 and
  measures three-pixel stroke coverage.
- Real development rank-3 CPU equations/custom geometry render at native 4K on
  M4 Pro Metal; renders/ retains initial and energy-adjusted captures/identities.
  Base wave/warp kernels remain preliminary; shader presets cannot use this
  preview as a fallback. No visual acceptance or emulator 4K30 claim exists.
- Direct Slang route: 83/170. Legacy HLSLParser → GLSL → glslang: 170/170.
  Direct SPIR-V → Naga rejects combined-image loads. SPIRV-Cross HLSL50 → Slang
  WGSL → Naga unique bindings validates all 170. Reports/intermediates saved.
  Probe orchestration remains Python; port to the Rust converter before packages.
- Vendored only the MIT HLSLParser dependency; license, attribution and
  original/current hashes retained, with a marked stderr diagnostic change.
  No sibling engine/evaluator sources or historical workers were modified.
- Tested explicit native-coverage-density-v1 on the development preview: keep
  ≥3-pixel coverage and scale deposited alpha by footprint. No holdout fitting.
- Renderer compiles for arm64 Android. Native-surface/presentation bridge,
  authored shader/resource graph and direct Milkbeat stereo integration remain
  incomplete. No new physical device operation occurred.
- Shader probes and host captures exited successfully. The owned emulator
  remains booted. Shader-control tests: exec session 24504; revalidate it before
  restarting. All other compile/check handles returned successfully.
Goal turn is concrete progress with native frames/full-corpus shader portability,
not completion. Shader execution semantics/deadlines still need proof.


Final checkpoint for this turn: 37 Rust tests pass (including actual native 4K
GPU allocations/coverage and shader binding/failure controls). Clippy passes;
rustfmt has been applied. Renderer checks compile for aarch64 Android. Both
preview image hashes verified. All 170 portable shader hashes and compiler
producer identity recorded. No active shader/build jobs remain; emulator is live.
This turn made implementation/evidence progress. The goal remains active;
Android native presentation, complete authored shader graph, Milkbeat integration,
all-100 visual/audio/performance verification and comparison are still required.


2026-10-08 Android native playback milestone:
- Rust package builder verifies the frozen source/artifact identities, produces
  all 100 procedural preset packages and uses Naga IR to bound every shader loop
  with a shared 64-iteration budget per fragment. Real authored fragment output
  has a GPU regression test; no fixed-function substitute for shader presets.
- Native resource graph now binds actual history/blur/noise/shared textures,
  point/linear clamp/wrap samplers and separate stage texture-size parameters.
  Half/quarter/eighth blur is auxiliary; primary/final/surface remain 4K.
- Added Android NDK Surface ownership and wgpu Vulkan presentation. Java render
  thread exclusively owns native state; Surface drops before acquired window.
  Source switching is transactional and clears history. All assets/notices staged.
- Built/installed app on emulator-5660. SurfaceFlinger independently reports
  3840×2160 SurfaceView buffers. Full 100-preset, 60-frame-per-preset sweep completed
  with zero reported render failures. Raw/summary evidence retained under
  benchmarks/emulator/native-poc/. This is not performance/visual acceptance.
- Rank 65 misses the pacing target. Added preparation/submission/acquisition
  timings, and removed repeated validated symbol-table comparisons from the
  immutable PresetState hot path; instruction/memory/stack limits remain intact.
  Further measurement/optimization still required. Diagnostic miss threshold36ms
  is not a strict 30-fps gate. Do not replace this outlier or lower feedback size.
- Real AudioPlaybackCapture from an own-UID AudioTrack fixture delivers 48k stereo
  i16 into the Rust worker/native renderer. Stereo window/gap reset tested; live
  generation/feature age logged. pcm-live evidence: ~76s, independent channels,
  one native queue drop. No fabricated musical input or Visualizer fallback.
- Milkbeat debug installed on owned emulator; launches empty provider profile.
  Need music/provider setup or direct PCM integration. No sibling source modified.
- Build and four JVM tests pass; 41 Rust workspace tests and clippy pass. Latest
  captures/sweeps are terminal; no benchmark/capture process intentionally left
  running after Home. Owned emulator remains live.

Goal is active, with concrete source-derived 100-preset Android playback progress.
Still required: Milkbeat music/PCM qualification, all-100 visual and longer
stability/audio tests, performance outlier fixes and meaningful ProjectM-TV
comparison. No completion or broad 4K30/fidelity claim is made.


2026-10-08 latest verification work:
- Isolated Milkbeat instrumentation module calls existing public local playback
  APIs via target classloader. Generated 48k stereo WAV plays in actual Milkbeat
  (no provider/login needed); external PoC capture excludes its own UID and got
  independent nonzero channels. Evidence under native-poc/milkbeat-pcm/. No
  Milkbeat production source or player/capture-policy changes. Controlled-source
  validation, not arbitrary network music/catalog claims.
- Dense geometry adaptation executes every original shape instance/state/RNG/
  memory invocation, but emits a uniformly sampled max256 geometry instances.
  Test watched fail then pass, preserving full original index range and state.
  Rank65 emitted vertices 111132→30492, sampled CPU frame ~50ms→~17ms. Native
  primary/final/surface sizes and substantial stroke widths stay unchanged.
  This policy was selected from development rank65, not holdout fitting.
- New LGPL2.1-noticed reference module uses unmodified published ProjectM-TV AAR.
  Local build AARs were stale against current source; inspected actual bytecode
  and selected cached published artifact exposing required APIs. First wrong-
  preset attempt is invalid comparison evidence. Numeric custom-pack storage
  convention fixed; later logs verify exact custom source name for rank65, native
  3840×2160 surface, trails Off. Matched screenshot/logs saved separately. This
  is real-clock practical reference, not deterministic Windows MilkDrop equality.
- Added explicit native JPEG92 preview readback after sweep timing samples. The
  100-preset/120-frame visual sweep is running on emulator-5660: process PID6250
  verified live, last observation 52 captures, zero reported capture errors.
  Do not restart it because a tool observation expires. Native captures are
  files/captures/NNN.jpg in the app; pull all after terminal sweep before visual
  review. Identity in visual-sweep-identity.json; capture timing not benchmark.
- Latest built APK includes geometry policy tag and current package catalog;
  tests/clippy pass. Full all100 final review/performance/audio/reference audit
  still incomplete. Goal remains active, meaningful implementation/evidence progress.


## Native Android PoC v4 verification (2026-10-08)

All100 packages / 170 authored shaders / 974 nonuniform bindings have stable
resource identities; GPU regressions catch wrong main/blur aliases, wrap versus
clamp, volume attenuation, textured shape feedback, native3px exact-origin seeds,
border lifetime, spectrum-versus-silent-PCM and circular source geometry.
Original corpus hashes remain unchanged. Early alias-bug sweeps are superseded.

native-detail-v4: all100 complete180 frames; native3840×2160 feedback/output/
surface; no renderer failures or texture substitutions; actual presentation
roughly59–60fps each. CPU hot p95 7.89ms/p99 12.72ms, one >33.333ms sample;
five presentation gaps >33.333ms (max39.19ms). This is emulator evidence, not
physical certification or a zero-miss guarantee. Contact-sheet low-variation
snapshots remain thirteen; motion/input/reference review remains active.

Capture from external Milkbeat controlled local playback is verified by earlier
milkbeat-pcm evidence. Updated combined capture/render verification is underway.
Source-mode geometry/reference test comparison controls are available. Mode8
fWaveScale omission is a minor follow-up outside the frozen corpus (no mode8
preset in the sample); do not silently claim exact source geometry/normalization.


## Extended Beatport review recordings

User supplied /Users/jneerdael/Music/Beatport.434 verified16-bit stereo44100 FLAC
files; nine tracks selected into an exact50-minute shared playlist. Source files
stay unchanged; decoded PCM, reference mono8 derivative and causal replay data
have recorded hashes in benchmarks/music-review/playlist.json. All100 preset
ranks are scheduled30seconds each at the identical music source offsets.

Guest codecs reject4K encoding; native4K RGBA readback streams bounded ordered
JPEG92 to Mac hardware HEVC24Mbps. Actual3840×2160 dimensions and exact frame
counts verify each chapter. First900-frame reference chapter completed; run
resumes valid chapter checkpoints. Capture lock prevents concurrent emulator
jobs. Full runs write chaptered50-minute masters with identical copiedAAC audio.
The initial JPEG100 probe demonstrated4K output but was slower; JPEG92 lowers
transport cost. Videos remain comparison jobs, never playback FPS evidence.
GLES comparison readback is optional, independently identified; Vulkan remains
the candidate production/playback path. Original reference wallclock/RNG remain
uncontrolled. Live Milkbeat stereoPCM capture evidence is separate.


Beatport comparison delivery completed:
- Both native3840×2160 movies contain90,000 encoded frames,100 named chapters
  and exactly50minutes. All nine original FLAC and four derived input hashes verify.
- Every adapted trace confirms native feedback/output, expected preset and audio
  positions, Vulkan backend and no texture substitutions. All100 reference traces
  identify the matching source and native capture surface.
- Both masters have identical compressed AAC payloads, identical decoded50minute
  PCM and matching start/priming metadata. Movie capture remains separate from
  playback performance; visual review belongs to the user.
- Disk-bounded fragmented assembly copied every video packet and timestamp. A
  final standard faststart remux preserves all90,000 packet signatures exactly.
  Rational track ticks replace an erroneous rounded-seconds boundary check.
- Reference recorder lifecycle race fixed in source; host Java compilation passes.
  Capture-controller orphan/disconnected-ADB regression reproduces pre-fix failure
  and passes with cleanup fixes. Installed recording APKs remain unchanged.
- User selected one independent `.beat` format for2D/3D/mixed effects, with no
  `.milk`/ProjectM runtime dependency or legacy limits. `docs/BEAT_FORMAT.md` and
  PLAN U10 record the design; native `.beat` execution is not implemented yet.
- Inspected the evolving local predictor worktree at587e3f53c0d9972b80251b5186690dd547061a98;
  relevant file hashes/findings are under benchmarks/analyzer-research. No source
  workers, helpers, historical evidence or sibling files changed.
- Recording artifacts, playlist/chapter cues and verification records are in
  benchmarks/music-review. Optional HTML viewer is syntax-checked but untested
  because browser URL policy rejected file URLs. Both recording apps are stopped.
