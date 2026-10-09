# Equation compiler and state runtime

The Rust compiler emits typed stack bytecode for all **744 equation scopes**
in the frozen 100 presets. Shared symbol layouts permit separate main, warp-
mesh, wave-frame, wave-point and shape environments. Original source line
numbers remain on instructions and diagnostics. HLSL sections are excluded from
this route and remain pending; the equation artifacts are explicitly non-playable.

Run `cargo run -p preset-cli --release -- compile-equations --artifacts --output
reports/equations.json`. The report retains every preset, compiler/executable and
lockfile identity, artifact hash, instruction/memory high-water marks and runtime
failures. Packages remain under `effects/<source-id>/<artifact-hash>/`; originals
and the frozen corpus manifest are unchanged. Current compact JSON equation
artifacts total approximately 6.26 MB across all 100, with shared-symbol tables.

## Characterized behavior and boundaries

- Ordered f64 arithmetic and assignments; `int` aliases floor, including negative
  values; `sqrt` uses absolute input as observed in MilkDrop2's x86 implementation.
- Lazy `if`, `&&` and `||`; eager `band`/`bor`; sequence expressions; assignment
  operators; numeric comparisons and bitwise integer operators; bounded counted
  loops and local/shared memory lvalues. Every function currently used by the
  corpus compiles. Unknown functions or malformed input produce diagnostics.
- Assignment flushes NaN/infinity and subnormal values to zero, matching the
  inspected exponent-class cleanup in NS-EEL's assignment template. Nonfinite
  assignment cleanups are counted. Unassigned nonfinite results are errors.
- `rand` uses library MT19937, one ordered stream per preset with an explicit
  seed, scaled by floor(max-bound) / u32::MAX. Source references use the same
  MT19937 recurrence but an uncontrolled process stream/default seed; cross-
  engine RNG equality is not claimed. EEL1 integer-rand mode is not implemented.
- `%` uses magnitude, truncated unsigned integer operands and zero for a zero
  denominator. Exact fractional rounding/overflow behavior of the original x87
  route still needs reference characterization; this boundary is explicit.
- Main `q1`–`q32` values reset from init each frame. Ordinary user variables and
  monitor persist. Mesh q values inherit main frame results once before a mesh;
  mutable warp parameters reset per vertex. Custom frame q values inherit main
  state; t1–t8 reset from custom init. Wave-point q/t transfer once per wave and
  evolve across points. Custom geometry parameters reset at their source-defined
  invocation boundaries. Separate presets cannot share banks/RNG/state.

These rules were checked against `../milkdrop2/src/vis_milk2/state.cpp`,
`milkdropfs.cpp`, and NS-EEL's `nseel-eval.c`, `nseel-cfunc.c`, compiler table and
x86 assignment/modulo/sqrt templates. This is source-grounded implementation,
not an executed original-engine trace equivalence claim. No engine/evaluator
source was copied. ProjectM-TV remains a distinct practical reference.

## Containment and measurement

Source cap 512 KiB, ≤131,072 tokens/instructions, ≤4,096 symbols and ≤128
parser/AST/loop nesting; value stack ≤512 entries. Each sparse memory bank has
1,048,576 f64-addressable cells, at most 8 MiB, with reads not allocating. Global
memory is explicitly shared only within one preset. Four waves/four shapes plus
main/mesh states bound the number of banks; malicious indices fail visibly.
Initialization permits 8M total instructions; steady frames permit 4M total and
1M per invocation. Each failed invocation carries its source scope and line.

The four-frame smoke runs a 48×32-cell mesh, authored enabled wave/shape counts
within declared caps, fixed seed 12345 and synthetic bands/waveforms. All 100
passed. It does not qualify live PCM response, geometry/color rendering, shaders,
visual quality or emulator frame deadlines. The separate 480-frame sweep also
passed all 100 (`reports/equations-480.json`), totaling 223.04 host wall-clock seconds
across the sequential run. Peak observed frame instructions: 1,060,041; largest
allocated equation memory: 3,211,264 bytes. These are host equation measurements,
not rendered fps. Pixel radial/aspect geometry still needs renderer controls.

The compiler/parser, lvalue/branch/loop semantics, state isolation and actual
Star Forge initialization have regression tests. Full source/reference replay
and visual/audio interventions remain part of renderer work.
