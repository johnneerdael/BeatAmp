# Preset Convertor contributor instructions

- Read `REQUIREMENTS.md`, `PLAN.md` and `docs/COMPARISON_PROTOCOL.md` before work.
- Read `docs/BEAT_FORMAT.md` for the independent unified 2D/3D format contract.
- Read `docs/AUDIO_DESIGN.md`. Real-time musical response and detailed analysis
  are critical requirements; preserve legacy comparison features alongside rich
  timestamped features. Initial delivery excludes stem separation and reliable
  voice/instrument isolation.
- Build the converter/runtime in Rust; allow a minimal Android platform shell
  for surfaces, lifecycle, permissions and audio delivery.
- Preserve native 3840×2160 primary feedback and final output in certification
  runs. Reduced-resolution rendering is a separately labeled mode.
- Treat 30 fps as a deadline requirement on specified physical hardware, not
  an average or a desktop/emulator result.
- Keep original source translation and 4K adaptation separately selectable.
- Use `.beat` as one independent format for 2D, 3D and mixed effects, sharing the
  audio ABI and effect graph. This supersedes the former `.milk3d` reservation.
  Keep `.milk` and prediction tooling offline; shipping playback must not depend
  on MilkDrop or ProjectM. Do not build a general game engine in the first milestone.
- Freeze `corpus/presets/`, `corpus/textures/`, `corpus/manifest.json` and
  `corpus/population.jsonl`. Never replace sampled failures or retune holdout
  presets to improve reported coverage. Derived packages belong under `effects/`.
- Use original source and ProjectM-TV as distinct semantic and visual references.
  Label every reference engine/artifact, input protocol and capture backend.
- Do not modify frozen ProjectM-TV workers, helpers or historical evidence.
  Create new adapters and isolated instrumentation with new identities.
- Keep comparison readback/hashing separate from performance benchmarking.
- Prefer declared dependency APIs to custom shader/compiler/graphics machinery.
  Verify version-specific behavior and record compiler route limitations.
- Preserve author attribution and CC0 asset notices. Copying engine or evaluator
  source requires its own applicable attribution/license review.
- Current contents are a planning/corpus handoff, not a runnable Rust project.
