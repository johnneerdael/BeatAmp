# Rust MilkDrop adaptation requirements

Created: 2026-10-08. Origin: the user's conversation and final naming correction.
Target project: `preset-convertor`, alongside `Projectm-TV` and `milkdrop2`.

## Purpose

Migrate existing `.milk` programs into a modern Rust effect system. Preserve a
recognizable visual identity and musical response while deliberately adapting
resolution-dependent behavior. Target actual native 3840×2160 rendering at a
minimum of 30 fps on the declared M4-hosted Android TV emulator for the PoC.
Physical-TV certification is deferred by the user's explicit instruction. The user reports that the
existing ProjectM-TV renderer cannot meet that 4K performance requirement; this
handoff does not independently measure that report.

## Current product direction (2026-10-09)

The application is **Beatamp**, published to `johnneerdael/BeatAmp`. Reuse the
ProjectM-TV interface and D-pad conventions with controls adapted to this engine.
The main product is programmatically generated premium audiovisual scenes,
guided by prediction evidence and source semantics; pixel likeness to MilkDrop
is not an acceptance requirement. Native geometry, lighting/compositing, motion
and musical response take precedence. Use the premium4K textures where their
roles suit the generated scene. Keep source translation as a diagnostic route.

The compiler must select/compose reusable understood scene components and derive
parameters/audio bindings automatically, without a manual rewrite for every
prediction. Preserve provenance and unknowns; source-field statistics alone do
not prove scene topology or causal live-audio mappings. Rich continuous stereo
PCM must expose its full fast/detailed spectrum through the actual Nyquist limit,
with real timestamps, channel identity and explicit analysis windows. Existing
comparison band features remain available separately. The user's selected PoC
hardware is the M4-hosted Android TV emulator; physical-TV qualification is later.

## Requirements

| ID | Required outcome |
|---|---|
| R1 | Rust converter and runtime, with a minimal platform shell where necessary. |
| R2 | Exactly 100 random source presets copied from the bundled collection; original bytes, names, assets and selection provenance preserved. |
| R3 | Translation-only execution establishes what the importer/compiler does before optional adaptations change behavior. |
| R4 | Explicit, reversible 4K adaptation preserves visual character and audio response, with changes and unsupported constructs reported. |
| R5 | Easy synchronized original/translated/adapted comparisons using ProjectM-TV as the practical oracle; original MilkDrop remains a separately labeled reference. |
| R6 | Primary feedback and final render output at 3840×2160; no whole-image upscaling disguised as native rendering. |
| R7 | Responsive native 4K playback targeting at least 30 fps on the declared M4-hosted Android TV emulator. Performance is critical: record pacing, timing, frame misses and switching costs. Emulator results do not establish physical-TV performance; physical certification is not a PoC completion gate. |
| R8 | Compact procedural preset packages; shared assets and reproducible derived identities, rather than baked video or per-preset executable applications. |
| R9 | Use one `.beat` format for 2D, 3D and mixed effects, with shared audio, timing, resources and compositing. The latest naming decision supersedes `.milk3d`. |
| R10 | The sample stays fixed. Unsupported, failed, unstable and unmeasured presets remain in the 100-preset denominator. |
| R11 | Engineers receive concrete phases, file boundaries, test scenarios, gates and unresolved technical decisions. |
| R12 | Audio reactivity is critical: timestamped real-time analysis offers fine frequency detail, fast transients, envelopes, rhythm and available stereo information. |
| R13 | Legacy bands remain available for comparisons; richer feature bindings are explicit adaptations, evaluated for both response strength and latency. |
| R14 | Initial delivery does not require real-time stem separation. Spectral features are not labeled as isolated vocals, drums or instruments. |
| R15 | Electronic-music bass response is critical: fast low-frequency transients and sustained/modulated bass energy have independent tests and bindings. |
| R16 | Melodic movement also matters: expose tonal/harmonic changes and suitable spectral envelopes without claiming isolated lead/vocal tracks. |
| R17 | Capture continuous higher-fidelity playback PCM. Android Visualizer snapshots are explicitly rejected as the PoC input tier. Preserve the actual rate, encoding, channels and sample timestamps; target stereo 48 kHz signed 16-bit or float32, with explicitly reported PCM format negotiation. |
| R18 | Adapted playback must look good at native 4K: no thin 1-pixel strokes and no low-resolution authored feedback canvas upscaled to 4K. Use deliberate native-pixel stroke coverage and native feedback. Prioritize visual/audio fidelity and performance above exact likeness. Compare against the user's ProjectM-TV Android player. |
| R19 | Shipping `.beat` playback has no dependency on `.milk` files, a MilkDrop parser or ProjectM. Import/prediction happens in offline conversion tools; original effects need no legacy source. |
| R20 | The format must support future functionality without MilkDrop's fixed registers, stage count or geometry limits. Use an extensible typed graph and explicit capabilities/resource budgets. |
| R21 | Use the evolving local milk-analyzer/predictor as a build-time source of behavioral evidence for high-quality adaptation. Preserve its version, evidence scope and unknowns. The user reviews visual quality after the paired recordings. |

## Success means

- Every sampled preset has an inspect/convert/render/review outcome, including
  explicit diagnostic outcomes for unsupported inputs.
- Each claimed successful adaptation has reproducible render/review evidence
  and adaptation provenance. The user permits creative adaptation and asks that
  the PoC look good and work; exact likeness and an additional user approval
  round are not completion gates. Preserve visual identity and inspect native
  detail, audio response and motion. Do not invent a fidelity percentage.
- Audio input quality, feature age, analysis cost and visible response delay are
  measured. Fine analysis runs independently of 30-fps rendering and remains
  bounded under load. Visualizer snapshots and microphone substitution do not
  satisfy playback capture. Unavailable PCM remains a failed input gate.
- A PoC 4K/30 result requires measurement on the declared Android TV emulator
  with all 100 outcomes visible. Label emulator evidence separately; make no
  physical-TV speed claim without later physical qualification.
- First delivery provides a small end-to-end working slice; complete-corpus
  coverage follows measured compiler and device feasibility.

## Scope and decisions

- Build adaptation, not strict pixel equivalence with every original renderer.
- Use program translation as the starting point; do not replace source logic
  with an AI-generated lookalike without identifying that as reinterpretation.
- A minimal comparison viewer is required. A general visual editor, cloud
  service, preset marketplace, music player and full scene engine are deferred.
- Real-time stem separation, automatic reliable vocal isolation and semantic
  instrument recognition are deferred. Multi-resolution spectral analysis is
  included; more FFT bins do not recover detail absent from captured audio.
- Define the shared `.beat` graph now; add actual 3D execution after the initial
  native 2D adaptation milestone. See `docs/BEAT_FORMAT.md`. Neither dimension
  receives a separate extension or a MilkDrop compatibility requirement.
- Use Android/Google TV emulation on the user's powerful M4 Mac. Do not operate
  physical TVs for subsequent PoC work. Record emulator OS, graphics backend,
  host hardware, GPU limits, native surface dimensions and timing. Physical-TV
  deployment coverage remains a later task.
- Build and verify an independent Android / Android TV PoC for all 100 frozen
  presets. Preserve sibling ProjectM-TV production sources and historical evidence.
- The user explicitly permits offline conversion to another procedural format
  and creative adaptation for 4K. Preserve recognizable identity, source
  provenance and selectable translation/adaptation modes; exact recreation is
  not the goal. Do not count generic replacement visuals as converted presets.

## Proposed first-delivery acceptance decisions

An engineer must record the minimum supported device before certification,
confirm the practical reference profile with the user using synchronized clips,
and select the required switching policy. Hard cuts are the initial proposed
baseline; smooth dual-preset transitions are separately budgeted and tested.
These decisions do not block parser, compiler or desktop comparison work.
