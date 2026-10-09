# Measured shader route

All 170 authored shader sections (83 warp, 87 composite) in the frozen corpus
compile and validate as WGSL. Identities are in reports/shader-portability.json;
intermediates are under tools/shader-spike/output/. This is offline compiler
and validation evidence, not shader rendering, fidelity or 4K30 acceptance.

The measured route is: supplied MilkDrop ABI → MIT legacy HLSLParser → GLSL
single-color output ABI → glslang 16.6.0 SPIR-V → SPIRV-Cross 1.4.363.0 typed
HLSL50 → Slang 2026.19 WGSL → Naga 30.0.1 unique-binding legalization and
validation → WGSL emission and repeat validation. Macro preprocessing uses
Clang, with shader parsing/code generation performed by dependency APIs.

Direct Slang legacy HLSL compiled only 83/170. Direct generated SPIR-V → Naga
failed on combined-image loads, including after standard inlining. Direct GLSL
→ Slang rejected vector/scalar casts and std140. The typed HLSL bridge resolves
legacy casts and sampling into modern operations; scalar color-output interface
normalization resolves the generator's one-element output array, which WGSL does
not accept as a fragment output. These failed routes are not advertised working.

Only the MIT HLSLParser dependency is vendored in third_party/hlslparser/. Its
LICENSE and original/current file hashes are preserved. The stderr-diagnostic
change is explicitly marked. No ProjectM renderer/evaluator source was copied;
sibling sources and historical evidence remain untouched. The isolated C++
compiler adapter calls public parser/generator APIs.

The Python scripts are feasibility probes, not the delivered Rust converter.
Port ABI/declaration/source-map/subprocess orchestration into Rust before
publishing playable packages. Identity rotation matrices and provisional roam
frequencies are prototype bindings; replace them with recorded runtime state.
Mutable q/texsize aliases are per-fragment copies, not CPU state mutation.

Sampling/wrap, noise construction, external/random textures, mip/LOD, blur,
private globals, derivatives and loop bounds need renderer controls. Unknown
GPU loops require containment before execution. Missing assets need diagnostics
or a named adaptation. Validation alone does not prove these capabilities.
Connect authored shaders to real native resources before declaring shader
presets playable; never count fixed-function fallback output as conversion.
