# Beatamp native scene authoring

`beat-authoring` builds standalone `.beat` packages offline. It composes two or
three of five independently authored procedural components: curved luminous
filaments, radial crowns, lit cell diamonds, interference arcs and angular folded
planes. Source SHA256 deterministically selects composition, geometric phases,
palette, thickness and optional feedback. It generates distinct WGSL and graph
work without maintaining one handwritten effect per preset.

Static source hints and supported prediction scalars guide bounded artistic
choices. They do not prove a source scene family, executable geometry or live
musical causality. Unknown geometry remains unknown. Recorded brightness events
are retained as evidence and never replayed as live beat triggers.

## Offline API

```rust,ignore
let source = SourceHints::from_bytes(&original_source_bytes)?;
let prediction = Prediction::read(prediction_file)?; // optional
let generated = synthesize(&SynthesisRequest {
    name: original_source_name,
    source: &source,
    prediction: Some(&prediction),
    capabilities: &device_capabilities,
})?;
generated.package.write(output_archive)?;
```

Admission preserves exact original JSON bytes, full opaque context, source
association, run identity, f64 values, units and unknown/failure outcomes. It
checks the47-field v1 envelope and source SHA association. The external Python
reference consumer verifies Python-canonical context/record/domain hashes;
Rust reserialization does not establish those identities. Missing records and
terminal predictor outcomes produce explicitly labeled source-guided original
adaptations with their statuses retained.

Packages contain programs, adaptation metadata and optional evidence. They
contain no `.milk` input, executable legacy equations or legacy package blob.
Source names and attribution remain in the manifest and metadata. Runtime
playback does not need the original source or predictor installation.

## Runtime controller

Depend on `beat-authoring` with `default-features = false` in the shipping runtime.
This includes only `Controller`/`ControlFrame`, audio analysis and their declared
dependencies. The optional default `offline` feature adds source/prediction
parsing and package authoring for build tools.

`Controller::new([width,height])` creates bounded causal control state.
`update(&Snapshot, now_ns)` returns named vectors and exact-size buffer writes.
Use the same clock origin as `Snapshot.capture_end_ns` for `now_ns`. Map the
returned audio generation/frame/timestamp descriptor to `BeatInput.audio` and
its parameter/buffer maps directly to the renderer input. Reset on discontinuity
through a changed audio generation; reject backward control clocks.

The two read-only storage buffers are `audio_fast` (513rows,8208bytes) and
`audio_detail` (2049rows,32784bytes). Each row is little-endian `vec4<f32>`:
raw left power, raw right power, smoothed left amplitude, smoothed right
amplitude. Powers come from independent channels, retaining antiphase input and
the complete DC–Nyquist range. Mono right values are zero and right availability
is separately reported. The amplitude curve is `10sqrt(power)/(1+10sqrt(power))`;
attack is10ms and release180ms. Fast1024 analysis does not wait for detail4096.
Fast availability expires after250ms; detail after500ms. Stale controls decay.
No AGC, stem separation or instrument naming is introduced.

Pass parameters use this exact order and independent vec4 alignment:

| Parameter | f32 count | Meaning |
|---|---:|---|
| `scene_time` | 4 | elapsed seconds,dt,native width,height |
| `audio_energy` | 4 | RMS,bass flux,bass energy,tonal change envelopes |
| `audio_regions` | 8 | DC,<90Hz,<350Hz,<2500Hz,<10kHz,through Nyquist,2reserved |
| `audio_chroma` | 12 | tonal-valid chroma envelopes |
| `audio_space` | 4 | balance,correlation,tonal confidence,available channel count |
| `audio_windows` | 4 | available fast/detail bin counts,feature ages(-1absent) |
| `audio_frequency` | 4 | fast/detail binHz,actual Nyquist,right-spectrum availability |
| `scene_style` | 4 | deterministic variation,motion rate,native width,history decay |
| `palette_a/b/c` | 4 each | RGB palettes;palette_a.w exposure |

`Controller` updates only time/audio values; generated style/palette defaults
remain in the package. Full-spectrum spatial sampling and CPU region summaries
give high frequencies and narrow peaks visible influence without reducing2049
bins per screen pixel. Low-frequency energy and transients, chroma/tonal change
and stereo modulation have independent controls. Future FFT sizes require an
explicit authoring/ABI extension rather than invented additional detail.

## Graph and verification

All passes use `vertex_main`/`fragment_main`, full-screen triangles and declared
bindings. The scene independently evaluates native pixel geometry in
linearRGBA16Float. Optional coherent previous-frame feedback also allocates
nativeRGBA16Float. Display tone mapping writes nativeRGBA8UnormSrgb with one
hardware transfer encoding. Thickness defaults deliberately cover multiple
native pixels. No authored low-resolution canvas or frame upscaling is used.

Tests verify deterministic archive bytes, full source association and unknown
preservation, structurally different compositions, all100frozen sources,
every generated WGSL through Naga, actual native4K GPU execution for both graph
shapes, visible live-audio changes, DC through Nyquist, independent stereo,
fast/detail availability, bass/tonal response and generation reset.

These are correctness checks. Native4K/30 pacing on the declared Android
emulator, actual playbackPCM capture, switching costs and visual review remain
application integration evidence. No physical-TV claim follows from them.
