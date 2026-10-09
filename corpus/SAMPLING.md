# Frozen random corpus

## Selection

Source: `../Projectm-TV/core/src/main/assets/presets/`.
Population: 9,606 files; 114,947,108 bytes. Inventory:
`population.jsonl`, one canonical JSON row per exact relative path in sorted
order, recording bytes and SHA256. Its whole-file hash is in `manifest.json`.

A newly generated random seed was frozen before rendering or feature analysis:
`42f428dc26a55bdd24b5d74c0db1c0f4`.

For each exact UTF-8 path, compute SHA256 over these concatenated bytes:

1. UTF-8 text `preset-convertor/sample-v1`.
2. One NUL byte.
3. ASCII text of the 32-character seed shown above (not decoded seed bytes).
4. One NUL byte.
5. UTF-8 relative preset path, with `/` separators, unchanged case and Unicode.

Sort by ascending digest, then exact UTF-8 relative path to break a tie. Select
the first 100 without replacement. No filtering by shader features, conversion
success, visual appearance or duplicated content was applied. Selection ranks
1–80 are development fixtures; ranks 81–100 are holdout fixtures. The manifest
records the exact selected files, so a future implementation need not depend on
a language-specific random-number generator.

Copied paths preserve original filenames and relative directories. Every copied
preset and texture was verified against its source SHA256. The immutable corpus
is identified by the manifest, population inventory and all recorded asset hashes.
The source checkout HEAD is context, not a substitute for those hashes.

## Textures

All 74 packaged textures are copied: 70 JPEGs and four PNGs, 8,022,243 bytes.
Retaining the full catalog preserves candidate sets for dynamic/random selectors;
it does not establish that every externally referenced texture is available.
The importer must diagnose missing and ambiguous references explicitly.

## Holdout handling

Feature inventory can inspect all 100 files. Fit numeric adaptation policies
and preset-specific overrides only on development fixtures. Freeze policy
identity before holdout rendering. Fix genuine parser/compiler defects across
the corpus under a new identity, but disclose any policy adjustment influenced
by holdout output; that output is then no longer an untouched holdout result.
Do not reroll the sample or remove failures from its denominator.

## Attribution

The source ProjectM-TV README attributes the presets to Jason Fletcher
(ISOSCELES)'s Cream of the Crop collection, via
<https://github.com/projectM-visualizer/presets-cream-of-the-crop>.
It states that bundled presets and textures are distributed under CC0 1.0.
The source notice is preserved in `../LICENSES/CC0-1.0.txt`. Preserve filenames
and any author information inside presets in all derived package metadata.

These asset notices do not assign a license to future Rust code or to copied
MilkDrop/projectM engine implementations. No engine implementation is copied here.
