# Native renderer progress

Rust wgpu now allocates two 3840×2160 RGBA8Unorm primary feedback textures and
one 3840×2160 final output texture. The GPU test inspects all three allocations,
renders a native three-pixel stroke and measures its pixel coverage. Comparison
readback is separate from normal render submission, which performs no hashing,
image encoding or readback.

The fixed-function vertical slice executes source CPU state/mesh and custom
wave/shape equations, deposits triangle geometry into native feedback, and uses
native warp/final passes. Base waveform modes/warp math and shape texturing are
preliminary; source equivalence is not claimed. Shader-bearing presets are
rejected by the preview command rather than replaced by generic fallback output.

Rank 3, Shreyas & Unchained's Deeper Aesthetics remix, rendered after 180 logical
frames on the M4 Pro Metal backend. Initial capture: renders/fixed-003.png and
JSON identity. Revised capture: renders/fixed-003-density.png and JSON identity.
The named native-coverage-density-v1 policy retains minimum three-native-pixel
coverage and scales line alpha by width and dot alpha by area, clamping RGBA8
alpha before the explicit footprint adjustment. Filled shapes retain source
alpha. This addresses accumulated feedback energy without reducing resolution
or silently normalizing comparison images.

Both captures retain source identity, native sizes and synthetic stereo-control
labels. They are host development captures, not Android emulator surface proof,
live Milkbeat audio evidence or 4K30 certification. Rank 3 is in the development
split; no holdout fitting occurred. Visual quality still needs broader review.

The required target remains emulator-5660. Connect the validated authored
shaders to resource/blur/texture passes, finish geometry/audio bindings, add the
Android native-surface bridge, then measure emulator pacing and all 100 outcomes.
Preserve the corpus and compare against labeled ProjectM-TV references.
