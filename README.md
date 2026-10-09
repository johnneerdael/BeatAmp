# Beatamp

Beatamp is a Rust proof of concept in development for programmatic premium audiovisual adaptations
on Android and Android TV. PoC target: Android TV emulation on the user’s M4 Pro Mac, native
3840×2160 feedback/output, recognizable
preset identity and detailed musical response. Offline conversion into compact
procedural packages is allowed; exact recreation of old resolution-dependent
rendering is not the goal. The new independent format is **`.beat`**, shared by
2D, 3D and mixed effects. Its [format contract](docs/BEAT_FORMAT.md) separates
offline prediction/import from standalone playback and removes MilkDrop's fixed
limits. The older source-derived PoC packages and movies remain a comparison
baseline. Standalone `.beat` packaging, generic GPU graph execution and an initial
programmatic scene compiler are implemented. See the [native adaptation workflow](docs/BEAT_ADAPTATION.md).

The baseline app plays all 100 source-derived packages at native 4K on
the M4-hosted Android TV emulator. Final visual/performance validation is in
progress; strict all-100 4K30 qualification is not claimed. Current implementation includes:

- A Rust inspector/parser that verifies the frozen manifest, seeded population,
  all **100 presets**, all **74 textures**, their exact paths/hashes and the 80/20
  development/holdout split. Source-mapped equation/shader sections are retained.
- A Rust equation compiler and bounded f64 runtime. All 744 equation scopes
  compile and all 100 presets pass a short state/mesh/wave/shape smoke.
  Derived procedural packages under `effects/` include bounded authored shaders
  and run in the independently installed Android player.
- Native wgpu feedback/output allocations and a real development-preset
  3840×2160 preview with substantial strokes. See [render progress](docs/RENDER_PROGRESS.md).
- All 170 authored shader sections compile/validate and are connected to native
  resource passes in the Android PoC. See [shader route](docs/SHADER_ROUTE.md).
- A Rust continuous-PCM analyzer and bounded worker: stereo envelopes/power,
  1024/4096 FFTs, 64 spectral bands with resolution metadata, bass flux and
  confidence-bearing chroma. Independent fast/detailed per-channel spectra now cover DC through the actual Nyquist limit. Legacy analyzer equivalence/tempo remain pending.
- An independent Android/TV capture test app with genuine AudioRecord playback
  PCM, platform consent, sample timestamps, lifecycle cleanup and diagnostics.
- Physical AM9 Pro evidence for 48 kHz stereo 16-bit PCM with no drops over
  controlled 97- and 159-second tests. These tests used an explicit AudioTrack
  fixture; they do not qualify Milkbeat integration or native 4K performance.
- A separate `.beat` Android JNI route and adapted D-pad UI. The new authoring
  pilot includes43 audited prediction packages and100 frozen source-guided
  packages, whose missing predictions remain pending. Device playback, premium
  texture application and visual acceptance of this new route remain under test.

The intended player is the user's [Milkbeat](https://github.com/johnneerdael/Milkbeat).
Its Media3 audio sink already receives decoded PCM, but its existing visualizer
tap downmixes to mono. Final integration should preserve stereo before that
downmix and synchronize analysis with audible playback. See
[Milkbeat integration](docs/MILKBEAT_INTEGRATION.md). Visualizer uint8 snapshots
are explicitly rejected as the input tier; microphone substitution is excluded.

## Build and inspect

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p preset-cli -- inspect --output reports/inspect.json
cargo run -p preset-cli --release -- compile-equations --artifacts --output reports/equations.json
python3 tools/build-android.py
```

The Android build requires Java 17, Android SDK 35 and NDK 27.3.13750724. Set
ANDROID_HOME when the SDK is not at ~/Library/Android/sdk. The build script
creates an arm64 debug APK at
platform/android/app/build/outputs/apk/debug/app-debug.apk, with a release Rust
library. Install it on a chosen device; **Capture music** requests playback
capture consent, while **Capture test tones** selects a labeled stereo
fixture. Home/Stop ends capture. Audio availability depends on player policy.

## Evidence and next work

[Inspector outcomes](reports/inspect.json) keep all 100 in the denominator and
separate parsed, converted, rendered, accepted and qualified states.
[Target hardware](docs/TARGET_HARDWARE.md) records the M4-hosted emulator target
and historical capture-only device evidence. Physical-TV certification is
deferred by the user; no further physical-device operation is needed. Raw traces and identified producers are in
benchmarks/audio/. The latest source also fixes DC leakage into spectral bass;
that correction has unit evidence but is not part of the frozen v2 device trace.

The corrected binding sweep completed all 100 presets at native 4K without
reported renderer failures. Earlier sweeps with incorrect resource aliases are
explicitly superseded. Controlled external Milkbeat local playback PCM is verified.
Visual review and presentation timing remain in progress. See [Android PoC](docs/ANDROID_POC.md).
Next: complete music/PCM integration, fidelity review, ProjectM-TV comparisons
and performance validation across all 100.
Native 4K detail, substantial stroke coverage, musical response and performance
take priority over exact likeness. Thin 1px strokes and an upscaled authored
feedback canvas are excluded from the adapted path.
Read [requirements](REQUIREMENTS.md), [plan](PLAN.md),
[audio design](docs/AUDIO_DESIGN.md) and
[comparison protocol](docs/COMPARISON_PROTOCOL.md). The implementation
ledger is [PROGRESS.md](docs/implementation/PROGRESS.md).

Original source and ProjectM-TV remain separately labeled references. Preserve
the immutable corpus, original authors/filenames and [CC0 notice](LICENSES/CC0-1.0.txt).
Sampling provenance is in [corpus/SAMPLING.md](corpus/SAMPLING.md). Derived packages
belong under effects/; fallback visuals are not counted as converted presets.

Generated effects, measurements, emulator disks, premium texture archives and licensed music remain local and are excluded from the source repository. Regenerate the native pilot with the commands in the adaptation workflow.
