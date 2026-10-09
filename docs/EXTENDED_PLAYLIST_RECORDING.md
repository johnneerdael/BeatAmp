# Beatport extended A/B recordings

The user supplied /Users/jneerdael/Music/Beatport for identical-music videos.
The library contains434 valid stereo44100Hz16-bit FLAC files. The selected nine
tracks form50minutes, with whole tracks and a trimmed final track. Source files
remain unchanged. playlist.json records file hashes, titles, source formats and
exact cue times; playlist.m3u8 preserves source playback order.

Both producers process the same sample ends, with100 original preset ranks each
assigned30seconds in the frozen order. Candidate replay preserves stereo signed
PCM and precomputed causal1024/4096 FFT features. ProjectM receives a documented
mono unsigned8-bit conversion of the same source through its existing API. The
new renderer never accepts that reference grade as its live-capture tier.

Native primary/final buffers remain3840×2160. The emulator codec probe rejects4K encoding. Capture nativeRGBA8 directly and
stream orderedJPEG92 frames through a bounded three-slot queue to the M4 host
VideoToolbox HEVC encoder. Reference GL rows reverse on the host. The1920×1080
TV compositor is excluded. Validate actual dimensions/frame counts before runs.
Each chapter has900 encoded frames at30 logical fps. Record both producers in
separate runs; attach the identical AAC soundtrack, derived once from the shared
PCM. Preserve native4K stills and producer/codec/input identities.

Recordings are comparison jobs. GPU copying/encoding, frame PTS and wall-clock
capture speed do not certify real-time playback. The unmodified reference core
retains uncontrolled clock/RNG; identical music does not imply pixel equality.
A GLES-specific comparison readback uses declared wgpu HAL context/texture
handles and restores GL state; it is independently identified from the Vulkan
playback path. This avoids routing recording buffers through display capture.

The earlier capture-free59–60fps emulator measurement applies to its identified
v4 producer. Remeasure changed playback separately.

Store chapter files temporarily and verify count/dimensions/decodability. The
reference movie is assembled and verified before deleting its temporary clips.
Actual remaining host space cannot hold both all adapted clips and a duplicate
master. Its assembly therefore uses FIFO inputs into a fragmented MP4, with
delayed initial metadata to preserve AAC priming. Compare every encoded packet's
SHA256 payload, PTS and DTS against the original chapter before deleting that
chapter from temporary storage. Keep the growing verified movie and all chapter
logs/hashes. Final verification checks all90,000 packets,100 chapters, native4K,
the shared audio packet hash and input offsets. No video/audio re-encoding occurs
during assembly. AAC terminal padding can extend the fragmented audio track by
at most one1024-sample packet; the visual timeline remains exactly3000seconds.
Music stays local; texture CC0 notices do not license the Beatport recordings.

ProjectM reference master: `benchmarks/music-review/projectm-beatport-50min-4k.mp4`.
Verified90,000 encoded packets,100 chapters and identical shared AAC payloads.
Both masters are complete and verified; `adapted-verification.json` is present.
The delivered adapted master is a standard faststart MP4 produced by stream-copy
remux after capture. All90,000 video packet payloads and timestamps match the
fragmented intermediate. Both final audio/video tracks last exactly3000seconds,
and decoded audio plus AAC priming matches the single shared soundtrack.
Installed APK hashes match `recording-producer.json`; a later source-only recorder
cancellation fix does not change the producer of these recordings.

Visual quality review belongs to the user. Agent verification covers file integrity,
native dimensions, complete frame/chapter counts and synchronized identical audio.
Do not derive fidelity scores or make subjective visual acceptance claims from these videos.
