# Standalone `.beat` packaging

Own a typed graph manifest and all declared asset bytes. Read/write a single
deterministic Stored ZIP archive; no `.milk`, source directories, predictor or
ProjectM installation is required to load it. Files are never extracted by the
reader. Asset hashes, kinds and original notices remain in the manifest.

Version 1 accepts bounded ZIP32 containers with UTF-8 portable relative names,
no encryption, comments, split volumes or ZIP64. The caller controls entry,
directory, manifest, asset, aggregate payload and archive-size budgets. A small
footer admission guard bounds metadata before zip8.6.0 builds its directory;
that library performs the actual ZIP/member/CRC reading. Missing, extra,
duplicate, symlink or hash-mismatched entries fail explicitly. The writer requires
an empty seekable output and validates all content before emitting bytes.

`Package::read(reader, &Limits)` validates the container and owned data. Runtime
graph/device and WGSL/binding validation are separate stages. `Package::write`
uses default budgets; `write_with_limits` accepts explicit authoring budgets.
WGSL text must be UTF-8; its shader syntax is validated by the GPU loader later.

From the workspace:

```sh
cargo run -p preset-cli -- beat-export --manifest effects/beat/package-fixture/manifest.json --output effects/beat/package-fixture.beat
cargo run -p preset-cli -- beat-inspect --input effects/beat/package-fixture.beat
```

The fixture proves independent packaging, not corpus migration, audio response,
GPU execution or native-4K performance.
