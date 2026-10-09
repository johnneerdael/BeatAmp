# Milkbeat PCM integration

The user owns `https://github.com/johnneerdael/Milkbeat` and identified it as the
intended audio source on 2026-10-08. The local `../milkbeat` origin resolves to
that repository; its default branch is `main`. Inspected checkout HEAD:
`ce3d3e31` (audio-output stall-recovery merge). The source inspection is read-only;
no Milkbeat production files, branches, APKs or historical evidence were changed.

The CRG database is empty and older than the latest commit. Graphify's query
returned provider URL-resolution nodes rather than the PCM sink. Current source
was read to establish the facts below; graph results were not used as proof.

## Existing source path

- `app/src/main/java/io/github/aedev/flow/service/Media3MusicService.kt` installs
  `equalizer` then `VisualizerTapProcessor(visualizerTap)` in Media3's
  `DefaultAudioSink`. This is decoded continuous player PCM, not Android
  Visualizer's uint8 snapshots. The installed dependency catalog specifies
  Media3 1.11.0.
- `player/audio/visualizer/VisualizerTapProcessor.kt` accepts PCM i16/f32, but
  converts its capture to an averaged **mono i16** array. Reusing that output
  would lose stereo and float precision. The new renderer must receive the
  interleaved PCM before this downmix, while keeping the audible output intact.
- `VisualizerAudioTap.kt` stores PCM in `PcmTimeline`, then `readAudible` selects
  a window using player-period identity, anchored playback position and lead
  time. Merely analyzing newest decoded samples would run ahead of sound.
- `VisualizerClockListener.kt` resets anchors on seeks/transitions and follows
  playback speed/state. Preserve these boundaries and distinguish elapsed-
  realtime versus monotonic clocks in the native ABI.
- `Media3MusicService.observeEqualizer` disables audio offload when equalizer
  processing or an active visualizer needs processors. The new listener must
  participate in that rule; an offloaded stream cannot silently bypass analysis.
- `ui/tv/music/TvVisualizerViewModel.kt` currently feeds ProjectM through the
  mono audible-window API. The existing renderer remains a separate comparison
  reference. Do not replace its audio semantics covertly.

## Selected direction

Keep the converter and runtime independent of Milkbeat. Produce compact
source-derived procedural packages offline and package the Rust runtime as an
Android library when the rendering slice is ready. Feed a bounded Rust analysis
worker genuine interleaved PCM from the Media3 processor path; retain native
rate/channels/encoding and stream-generation identity. Publish sample-position-
keyed features and select them for the audible playback position, rather than
treating decode time as presentation time. Reset on seeks, discontinuities,
source/format changes and listener release. Avoid FFT work or renderer waits in
the sink processor.

A Milkbeat integration change must be made in its isolated task worktree and
follow its AGENTS.md validation/review/merge workflow. It must retain Material 3
UI conventions, the existing visualizer choice, offload/output-recovery behavior,
and the independently identified ProjectM reference. No such change is made yet.
The user's ownership permits considering a native host integration; it does not
prove that an integration already exists or that a player capture policy passed.

Standalone `AudioPlaybackCapture` is the delivered PoC source route. Fresh
external-source evidence under `benchmarks/emulator/native-poc/milkbeat-pcm/`
records Milkbeat debug playing a controlled local stereo WAV through its public
player manager, with continuous 48 kHz stereo i16 capture into the Rust analyzer
and native renderer. The isolated `platform/android/milkbeat-proof` adapter
starts local playback through the existing APIs; it changes no production source.
The test proves that source/route/profile, not arbitrary network providers.
Direct decoded float32 host integration remains future work; it is not needed
to substitute a lower-quality Visualizer input. Visualizer snapshots remain rejected.
