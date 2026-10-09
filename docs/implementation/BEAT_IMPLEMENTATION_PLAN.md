# Independent `.beat` implementation plan

> Implement inline using the executing-plans skill. The user explicitly authorizes
> autonomous work without approval rounds. This workspace is not a Git checkout;
> preserve the existing baseline artifacts instead of creating a worktree/commit.

**Goal:** Ship procedural `.beat` playback for the frozen 100-preset corpus using
an independent native graph, while retaining the completed comparison baseline.

**Architecture:** A typed manifest describes resources, arbitrary named parameters,
versioned operations and same/previous-frame connections. A maintained archive
library loads hashed programs/assets. Rust/wgpu executes the validated graph;
offline import/prediction supplies source-derived graph/control programs.

**Tech stack:** Existing Rust1.95/serde/Naga/wgpu30.0.1, SHA256 and zip8.6.0.
**Spec:** `docs/BEAT_FORMAT.md`, REQUIREMENTS R6/R9/R12–R21.

## Constraints and review focus

- Keep original presets/textures, sibling apps/predictor, baseline APKs and movies.
- Use one `.beat` contract for 2D/3D/mixed graphs; actual 3D execution stays future.
- No `.milk`, ProjectM, predictor process or legacy fixed-register ABI in playback.
- Native primary feedback/final output remain3840×2160. No recording in benchmarks.
- Preserve generic geometry/compute/program extensibility and author/asset notices.
- Reject missing assets, unsupported required operators and mismatched port types.
- Distinguish previous-frame feedback from invalid same-frame cycles/read hazards.
- Bound file/resource/work budgets by caller/device limits, not MilkDrop counts.
- Preserve unsupported/unknown outcomes in the full100 denominator.
- The user judges visual quality; source predictions/parse success cannot pass it.

## Task1 — Manifest, graph validation and executable plan

**Files:** `crates/effect-model/src/beat.rs`, `crates/effect-model/tests/beat.rs`;
update effect-model dependencies/exports. Schema/fixtures under `effects/beat/`.

**Interfaces:** `Manifest::validate(&Capabilities)->Result<ExecutionPlan>`;
capabilities contain supported operation IDs/features, native extent and budgets.
ExecutionPlan contains topologically ordered node indices, resolved dimensions
and total texture/buffer allocation estimates. Operator-specific execution stays
outside structural validation. Input ports declare expected resource types.

- [x] Test a native feedback graph: history doubles allocation, input order is
  irrelevant, previous-frame reads do not introduce same-frame dependency cycles.
- [x] Test mixed color/depth/buffer descriptions round-trip with arbitrary named
  parameters; an unsupported3D operation gets a diagnostic, not fallback output.
- [x] Test cycle, duplicate writer, undefined resource/asset/program, wrong input
  type, nonfinite parameter, invalid history read and undersized resource budget.
- [x] Run `cargo test -p effect-model --test beat`, observe missing implementation.
- [x] Implement typed manifest/plan validation and rerun the test through green.

## Task2 — Hashed single-file `.beat` packaging

**Files:** new `crates/effect-package/{Cargo.toml,src/lib.rs,tests/archive.rs}`;
workspace wiring and CLI export/inspect commands in `apps/cli/src/beat.rs`.

**Interfaces:** `Package::read(Read+Seek,&Limits)->Result<Package>`;
`Package::write(Write+Seek)->Result<()>`. Package owns Manifest and declared
asset bytes. Program assets use WGSL. Entry hashes and roles are in the manifest;
the reader never extracts an arbitrary path or requires source files.

- [x] Test a complete shader effect round-trip after original input files are
  absent, byte-identical repeat export and a tampered asset failing hash validation.
- [x] Test duplicate/traversal/absolute/symlink/undeclared entries, oversized entry
  and unsupported version/compression diagnostics with small isolated archives.
- [x] Observe failures, implement bounded stored-ZIP reading/writing using the
  declared library API, then pass tests. Compression is a measured future option.
- [x] Add CLI export/inspect commands and an independently authored `.beat` fixture.

## Task3 — Generic native GPU graph execution

**Files:** `crates/effect-render/src/beat.rs`, tests in
`crates/effect-render/tests/beat_gpu.rs`, `effects/beat/aurora/`.

**Interfaces:** `BeatRenderer::new(Gpu,Package,extent)->Result<Self>`;
`update(&BeatInput)`; `render()`; `output_view()`; `allocation_report()`;
opt-in `capture()`. BeatInput preserves audio descriptor/generation/timestamps and
named feature/parameter values. Program bindings specify resources and samplers.

- [ ] Test arbitrary multi-pass render/compute ordering, history isolation/reset,
  native4K allocation and no hidden reduced primary feedback on a real GPU.
- [ ] Test an original Aurora-inspired graph that uses named bounded envelopes,
  native thick geometry/fields and feedback without imported equation state.
- [ ] Observe failures, implement supported generic graph operations through wgpu,
  validate WGSL with Naga and reject remaining required capabilities explicitly.
- [ ] Measure changed/silent/bass/tonal controls and capture-free pacing separately.

## Task4 — Programmatic prediction-guided native scene generation for all100

**Files:** `crates/milk-compile/src/beat.rs`, a typed prediction admission module,
`crates/milk-compile/tests/beat_corpus.rs`, `effects/beat/catalog.json`.

Latest user direction: premium adaptive visuals and beautiful full-spectrum
musical response. Likeness is optional. Build a deterministic compiler from
supported evidence and source hints into a compositional scene vocabulary;
do not manually rewrite every prediction or embed a legacy PresetPackage blob.

- [ ] Admit/version/check available47-field records with full context/units,
  exact Python-reference identity verification, nullable unknowns and terminal
  failure outcomes preserved. Pair source inputs by SHA256, not filenames alone.
- [ ] Define a bounded vocabulary of native fields, ribbons/strokes, instanced
  elements, angular folding, feedback, materials and compositing. Compose and
  parameterize it programmatically from supported targets and explicit source
  evidence. Keep uncertain source family interpretations labeled as such.
- [ ] Generate native3840×2160 graphs and portable WGSL using shared components;
  variation comes from evidence and deterministic seeds, not hand-authored files
  for each preset. A native scene independently evaluates its geometry/fields.
- [ ] Generate live audio bindings over full fast/detail channel spectra,
  waveform/envelopes, transients and tonal/stereo information. Recorded brightness
  events are timeline evidence, not reusable beat triggers. Add deliberate
  attack/release, damping, bounded nonlinear response and continuous-time motion.
- [ ] Resolve premium texture assets by declared role, channel/precision contract
  and sampler state when appropriate. Preserve author/asset notices; cook
  role-aware mipmaps and numeric conversions explicitly, without aRGBA8-only path.
- [ ] Verify deterministic standalone export/loading and genuine live response
  to controlled bass/mid/high/tonal/stereo inputs. Retain the frozen100 denominator
  and explicit prediction availability/adaptation choices. User video review
  assesses final visual quality; no source forecast score is reused as proof.

## Task5 — Android/TV integration and final100 evidence

**Files:** `crates/android-bridge/src/render.rs`, Android NativeRender/RenderView,
`tools/build-android.py`, separate new producer/evidence directories.

- [ ] Preserve baseline APKs and identities before building a new producer.
- [ ] Wire `.beat` loading and native presentation to the minimal platform shell;
  keep actual live higher-fidelity PCM capture and source diagnostic mode separate.
- [ ] Run host checks, build/install the isolated new app on emulator-5660, verify
  live PCM reaction, lifecycle/switching, all100 native allocations and pacing.
- [ ] Record a separately identified all100 `.beat` movie with the same frozen
  Beatport timeline for the user's review, preserving the original paired movies.
- [ ] Audit every requirement against exact current producer/artifact evidence.

Full workspace tests, rustfmt and clippy are required after code changes. No
format/renderer completion claim follows solely from model/archive validation.
