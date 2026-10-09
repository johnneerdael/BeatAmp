# Real-time audio design

Status: partially implemented. The independent Android shell captures continuous
48 kHz stereo i16 playback PCM; a Rust worker computes envelopes, spectra,
bass flux and confidence-bearing chroma. Controlled AM9 Pro evidence is under
`benchmarks/audio/`. Milkbeat player integration and complete analyzer/visible
response qualification remain pending. Read with PLAN units U0/U1/U2/U6/U8.

## Input quality comes first

The required source is continuous playback PCM at its actual negotiated rate.
The user explicitly rejected Visualizer snapshots on 2026-10-08. Target stereo
48 kHz signed 16-bit or float32; report negotiated 44.1 kHz or mono explicitly.
Accept i16 or f32 input through a single
documented decoder; preserve gain and channel identity. An input descriptor
records encoding, sample rate, channels, capture timestamps, source identity,
quality tier, discontinuities and any platform normalization.

| Input tier | Useful capability | Limitation |
|---|---|---|
| Full PCM from a host/player, fixture or permitted playback capture | Continuous spectral analysis, transients and stereo features if channels exist | Requires an actual available PCM source |
| Android Visualizer waveform (reference only; rejected for PoC input) | Historical reference context | Partial unsigned 8-bit mono snapshots; fails the required input gate |
| No usable input | Clear waiting/permission/source state | No fabricated musical response or fake stereo |

Android `AudioPlaybackCapture` requires Android 10+, RECORD_AUDIO permission,
MediaProjection user consent, compatible player capture policy/usage and the
same Android user profile. Availability on the exact TV/player must be proven;
do not promise capture of arbitrary other applications. Permission revocation
and source changes are normal lifecycle events.

Android `Visualizer` provides partial, low-quality audio for visualization. It
can support the existing style of reaction but does not justify a claim of
high-resolution continuous stereo analysis. Run the richer analyzer only on
features the source actually supports. Never silently substitute microphone
input. Test the real input source before investing in large FFT configurations.

The existing application really does attach to the player's audio session:
`../Projectm-TV/app/src/main/java/com/example/projectm/visualizer/PlayerSessionFinder.java`
probes sessions with short-lived Visualizer instances, and `MainActivity.java`
creates `new Visualizer(audioSession)`. Its capture listener enables waveform
delivery and disables FFT delivery; `onFftDataCapture` is empty. Waveform bytes
go through `ProjectMJNI.addWaveform` into the engine, which derives spectrum/bands.
Session access therefore includes useful frequency information through the
waveform; it does not mean the app receives the player's full PCM stream.

Use `AudioPlaybackCaptureConfiguration` with `AudioRecord` as the initial
cross-app PCM provider. Request RECORD_AUDIO and the MediaProjection platform
consent flow; use a mediaProjection foreground service. Match media/game/unknown
playback usages, with optional explicit player UID selection. If the intended
player forbids capture, require a permitted PCM player/host integration. Do not
downgrade to Visualizer or silently open a microphone.

The user identified their own `johnneerdael/Milkbeat` app as the intended player.
Prefer direct interleaved decoded PCM in that host for the final integration;
the existing Media3 sink already has an audio-processor tap, but its current
visualizer API downmixes to mono. Tap before that downmix and retain i16/f32
encoding, channel identity and audible presentation timing. See
`docs/MILKBEAT_INTEGRATION.md`. Keep standalone mix capture as a separately
identified PCM provider; its fixture test does not qualify Milkbeat compatibility.

Read bounded interleaved PCM blocks on a dedicated capture thread and pass
sample positions, monotonic capture timestamps and generation identity to Rust.
Use `AudioRecord.getTimestamp` when supported; label receipt-based timestamp
estimates otherwise. Record actual format/buffer, read cadence, timestamp
availability, nonzero input, overruns and revocation. An initialized recorder
or stream of zeros alone does not prove working audio.

The historical Visualizer route informs reference-engine comparisons only.
Never concatenate its snapshots into invented continuous FFT history or use
its apparent detail as evidence for the new playback PCM requirement.

Sources: [Android playback capture](https://developer.android.com/media/platform/av-capture)
and [Visualizer](https://developer.android.com/reference/android/media/audiofx/Visualizer).

## Full PCM spectrum delivery

The Rust snapshot now exposes independent one-sided `fast_spectrum` (1024 FFT)
and `detail_spectrum` (4096 FFT) channel powers, including DC and the actual
Nyquist bin. Each carries FFT size, bin spacing, Nyquist frequency, ending sample
frame and capture timestamp. Stereo powers are kept independently; antiphase
signals are not lost through mono downmix. Immutable Arc-owned arrays avoid
copying the entire detailed spectrum for every fast snapshot clone.

Existing64-band summaries retain their20Hz–20kHz convention for comparison.
They are not the complete spectral input. Native effects can consume the full
arrays through the negotiated sample-rate Nyquist limit (24kHz at48kHz input,
22.05kHz at44.1kHz). Bass/tonal features still remove DC before measurement;
the full PCM spectrum restores the raw DC contribution analytically using the
periodic-Hann transform. This preserves offset evidence without creating false
bass onsets or requiring another FFT. Encoder/generator and GPU parameter
conversion must preserve these window/timestamp and channel distinctions.

## Two feature families

**Legacy compatibility:** retain a separate analyzer reproducing the selected
reference's waveform window, FFT/band aggregation, attack/release smoothing,
long-term normalization and bass/mid/treble plus attenuated values. Original
presets initially bind these names. A generic FFT is not an equivalent drop-in.

**Detailed features:** publish waveform samples, absolute RMS/peak, per-band
energy/envelopes, positive spectral flux, onset events and confidence-bearing
tempo/beat phase. With stereo PCM, additionally expose left/right energy,
balance and correlation. Include confidence-bearing 12-class chroma/tonal-change
features in the initial analyzer because melodic response is an explicit user
priority. Accurate polyphonic transcription and isolated lead identification
remain deferred. All fields declare units,
normalization, channel source, analysis window and sample timestamp.

Do not equate bass energy with a kick drum, high-frequency energy with hi-hats,
or the midrange with vocals. These are overlapping musical signals. Initial
features describe observed audio, not isolated sources. Stem separation and
reliable voice/instrument isolation are explicitly out of scope.

## Multi-resolution analysis

Use `rustfft` for FFT implementation and planned-buffer reuse; verify its
normalization against the legacy analyzer. Avoid copying FFT library code.

Proposed starting windows, to be calibrated on target hardware:

| Purpose | Window / hop | At 48 kHz | Meaning |
|---|---|---|---|
| Fast RMS/peak envelope | 256 / 256 samples | 5.33 ms updates | Immediate level changes, no fine spectral claim |
| Transient spectrum / flux | 1,024 / 256 samples | 21.33 ms window; 5.33 ms updates | Fast spectral changes |
| Detailed spectrum | 4,096 / 1,024 samples | 85.33 ms window; 21.33 ms updates | Better frequency resolution, slower evidence |
| Optional sub-bass refinement | 8,192 / 2,048 samples | 170.67 ms window; 42.67 ms updates | More low-frequency detail with greater delay/cost |

These are design proposals, not achieved timings. Compute durations from the
actual sample rate; do not relabel 44.1-kHz input as 48 kHz. Longer windows have
greater observation delay even when feature updates are frequent. Each feature
has its own timestamp/window; avoid one misleading global latency number.
Use causal trailing windows for live operation. Do not use future samples or
offline centered smoothing to make live results look faster.

Aggregate into 64 logarithmic bands initially, bounded by Nyquist, with
well-defined weight normalization and explicit empty/undersampled-band handling.
At low frequencies, multiple band labels may have insufficient independent FFT
resolution; report this rather than inventing detail. Use fast onset/envelope
features alongside slower fine-frequency features, not a long-window spectrum
as the sole reaction trigger.

Give electronic-music bass two explicit channels: fast low-band spectral-flux
events for kick-like transients and a sustained low-band energy envelope for
basslines, drones and modulation. Proposed initial descriptive frequency ranges
are 20–60 Hz sub-bass and 60–250 Hz bass, limited by the actual capture/filter
response. Keep the fast envelope/short-window onset path independent of optional
long-window refinement; do not delay kick response for better sub-bass resolution.

For melodic movement, retain the full spectral-band envelopes, tonal onset/change
and normalized chroma energy with a confidence/validity indicator. Describe
harmonic information in a mixed signal; do not claim a reconstructed melody or
assume every mid/high change belongs to a synth lead. Chroma near silence or
noise-only passages is unavailable/low confidence, not a confident pitch change.

Proposed detailed feature production rate is 50–100 Hz for the renderer's main
snapshot stream. Keep sample-domain fast analysis separate; publish/discard
redundant intermediate snapshots with a latest-value mechanism. The renderer
consumes timestamped snapshots at its own cadence and maps onset events by
event ID/time and a generation-scoped cursor. Retained in-generation events are
consumed at most once; bounded storage cannot guarantee lossless delivery during
arbitrary renderer stalls.

## Scheduling and bounded work

- Capture callbacks copy into preallocated bounded input buffers, add sample
  timestamps and return. No FFT, file I/O or locks that can wait for rendering.
- A single audio worker owns analyzer state and preplanned FFT workspaces.
  Audio does not wait for GPU completion, shader compilation or video export.
- The renderer reads a coherent latest snapshot plus a bounded event sequence.
  It does not run analysis inside a GPU submission loop.
- Proposed onset retention: at most 128 events and no more than 250 ms of event
  history, per source generation. Evict expired/overflowing events with a dropped
  counter. A resumed/stalled renderer advances its cursor past expired events
  rather than replaying old kicks as a burst. Continuous bass envelopes remain
  latest-value signals. Calibrate retention against useful visible latency;
  preserve the explicit bounded/drop policy even if limits change.
- Initial proposed input-buffer cap: 250 ms of negotiated PCM. Monitor backlog;
  above 40 ms beyond the intentional analysis history, discard stale queued
  work with a recorded discontinuity and reset affected windows. Historical
  window storage is separate from queued unprocessed input.
- Publish feature age, dropped blocks, discontinuity count and per-stage cost.
  Stale/unavailable audio decays to a documented neutral state rather than
  retaining a fake beat indefinitely. Proposed stale boundary: 100 ms without
  new source data, distinct from long-window analysis delay.
- On pause, source switch or capture revocation, end the input generation,
  reset normalization/tempo history as appropriate, and prevent old-source
  snapshots from reaching the resumed renderer.

The numerical backlog/stale boundaries require validation against Android PCM
delivery. Report negotiated quality/timing failures explicitly; a rejected
limited-input tier cannot satisfy the full-PCM gate.

## Latency and resource budget

Measure capture delay, analysis delay, feature age at consumption, render queue
delay and visible presentation separately. The initial engineering target for
full-PCM fast features is an analyzer/queue contribution below 10 ms at p95,
excluding source capture and the declared analysis window. The proposed visible
impulse-response target is no more than 100 ms from the known sample event to
first presented response, using a fast feature. These targets require physical
device evidence; presentation measurement needs a supported timestamp path or
an external audio/video witness with documented uncertainty.

Proposed analyzer budget is no more than 10% of one target-device CPU core
under steady full-PCM input, reported as thread CPU time divided by wall time,
with preallocated steady-state workspaces. Measure it alongside 4K rendering.
If richer features exceed this budget, reduce optional long-window/tempo work
first and document the feature tier. Preserve fast audio response; do not let
an expanding analysis backlog create several-second visual delay.

No hidden AGC, per-clip peak normalization or automatic gain changes are allowed
in comparisons. A user gain setting or richer-feature adaptive normalization
must be explicit and logged. Legacy relative normalization remains separately
reproduced because presets depend on it.

## Tests and evidence

- Sine sweeps and individual tones: correct frequency mapping, predictable
  energy scaling, no energy created in unsupported bands.
- Silence, DC, clipped signals and denormals: finite outputs, stable neutral
  state, no false onset train or confident invented tempo.
- Three-amplitude impulses and repeated kicks: monotonic input-level response
  where the feature definition requires it, event timestamps and measured delay.
- Tempo changes, syncopation and irregular pulses: confidence decreases when
  beat evidence is weak; fast onset reaction does not wait for tempo lock.
- Electronic-music controls: four-on-the-floor kick, offbeat sustained bass,
  moving bassline, simultaneous kick+bass and quiet bass modulation each affect
  their declared feature channel; bass response must not rely on tempo lock.
- Melody controls: a synthetic note sequence/chord change over an unchanged
  kick+bass bed produces harmonic/tonal feature changes and visible melodic
  response; a melody-constant bass intervention does not erase that channel.
- Compare bass-only, melody-only and combined interventions from an identical
  prefix. Evaluate their incremental visual effects independently rather than
  approving one aggregate motion score dominated by bass.
- Stereo pan/correlation fixtures: correct stereo features; mono/Visualizer
  input marks stereo unavailable rather than duplicating false independent data.
- 44.1/48-kHz input, chunk boundaries and callback jitter: sample-clock results
  remain equivalent within declared numeric tolerance for the same samples.
- Render stalls and shader compilation: audio stays current and bounded.
- Event overflow and long stalls: retained events are consumed once, expired
  events increment drop accounting, and resume does not emit a backlog of kicks.
- Source changes/revocation and dropped blocks: generation isolation and
  discontinuity handling prevent stale rhythm/normalization leakage.
- Controlled bass/mid/treble interventions: compare legacy and adapted visible
  response after an identical control prefix; save feature traces and response
  onset/magnitude. Self-running movement is not counted as audio response.

Inspect richer features visually in the comparison viewer. Converted presets
use legacy bindings initially; enhanced mappings to envelopes, bands or onset
events are named adaptations with source maps and separate acceptance records.
New `.beat` effects can consume the richer ABI directly.

Reference: [RustFFT documentation](https://docs.rs/rustfft/latest/rustfft/).
