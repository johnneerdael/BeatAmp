# Current dependency route

Toolchain: Rust 1.95.0, edition 2024, locked in `rust-toolchain.toml` and
`Cargo.lock`. `cargo test --workspace` includes the explicitly registered
`tests/integration` package. macOS and arm64 Android compilation are verified;
Android native linkage uses NDK 27.3.13750724 / API 29.

Current direct libraries: RustFFT 6.4.1 for planned FFTs; serde 1.0.229 and
serde_json 1.0.151 for contracts; SHA2 0.10.9 for corpus identity; encoding_rs
0.8.42 for legacy source decoding; clap 4.6.7 for the CLI; crossbeam-channel
0.5.17 for bounded queues; JNI 0.21.1 for the platform bridge; libc 0.2.190
for Android thread CPU timing; tempfile 3.27.0 for isolated tests.
Most use permissive MIT/Apache licenses (see each dependency's metadata);
encoding_rs also declares BSD-3-Clause. A redistributed artifact still needs
complete generated dependency notices. Original preset author names and
`LICENSES/CC0-1.0.txt` remain intact. No MilkDrop/projectM evaluator/engine source
has been copied into the runtime.

Android shell: framework Java 17, compile/target SDK 35, min SDK 29; Android
Gradle Plugin 8.12.0 and Gradle 8.14.2. Gradle wrapper files came from the sibling
checkout, retaining their upstream notices and distribution checksum.
`python3 tools/build-android.py` builds the release Rust library into the debug
test APK and runs JVM tests. APK debug signing is strictly a local PoC route.

Logos 0.16.1 now owns equation tokenization; a bounded parser/lowering layer
emits typed source-mapped bytecode. rand_mt 6.1.0 provides MT19937 for per-preset
ordered randomness. Both crates have MIT/Apache license metadata. Equation
compiler/runtime tests and all-corpus short smoke pass.

The renderer and offline shader dependencies are now selected; see the measured
route below. Native Android presentation and rendered shader semantics remain
pending.


Renderer/compiler additions: wgpu/Naga 30.0.1, bytemuck 1.25.2, image 0.25.10,
pollster 0.4.0. Host Metal GPU execution and arm64 Android compilation are proven;
Android surface/presentation is not yet implemented. Offline compiler route uses
Clang + MIT HLSLParser + glslang 16.6.0 + SPIRV-Cross 1.4.363.0 + Slang 2026.19.
Tool/source artifact identities and limitations belong in the shader route record.
