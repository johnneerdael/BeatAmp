# Engineering target and current evidence

Current PoC target (user correction): a dedicated native-4K Android TV emulator
on the user's M4 Mac. Physical-TV operation/certification is no longer required
for this PoC and subsequent work must use emulation. Host/backend/surface and
deadline measurements will be recorded once the renderer is running. Do not
label emulator results as physical-TV performance.

Created AVD `PresetPocTV4K_API36`, serial `emulator-5660`, Android TV API 36
arm64-v8a, 8 emulated CPU cores / 4 GiB RAM, native display 3840×2160 at 60 Hz.
Host: Apple M4 Pro (`Mac16,7`), 48 GiB RAM. Emulator 37.1.11.0 (15917651),
host-GPU mode with Vulkan enabled. GLES reports the M4 Pro Metal translator;
Vulkan reports the Apple M4 Pro device. Saved profile/config/vkjson/log are in
`benchmarks/emulator/`. The long-lived emulator was started through exec session
62511; revalidate its process/device before reusing or restarting it.

Android TV reports an enforced 1920×1080 UI override even when the native display
is 4K. The renderer must request an explicit 3840×2160 SurfaceView/native window,
verify its buffer size plus feedback allocations and capture native detail.
Neither display mode nor an upscaled system UI proves native preset rendering.

## Historical capture-only device evidence

Initial physical target: Ugoos AM9 Pro, Amlogic AMLS905X5 (`s6`), Android 14.
Fingerprint: `Droidlogic/franklin/franklin:14/UD2A.240505.001.W1/eng.ugoos.20260903.151956:userdebug/test-keys`.
ADB serial used: `192.168.51.53:5555`. Intended player: user's Milkbeat.

Android reports physical 3840×2160 display modes, including 60 and 30 Hz; the
system UI/logical display is 1920×1080. The capture-verification shell currently
uses that ordinary UI surface. A native 4K renderer/surface has **not** been
created or qualified. GPU adapter/limits/formats/timestamp support are unmeasured.
This engineering target is not a certified minimum-device claim.

Two controlled playback-capture runs are saved in `benchmarks/audio/` with
actual APK/native-library hashes, producer binaries, PCM feature traces and
capture identity. The second run delivered 158.63 seconds of 48 kHz stereo i16
PCM, with zero native/queue drops and no receipt-only timestamp reads. Analysis
thread CPU/wall ratio was 3.79% of one core; mean analysis cost was 186.91 µs,
maximum 1.952 ms. Telemetry samples showed 65.56–85.46 ms age from sample-window
end to report receipt. This is neither end-to-end visible latency nor a p95
capture/analysis benchmark. No renderer ran concurrently.

The fixture alternates left 110 Hz, right 880 Hz, right 1046.502 Hz and silence,
four seconds per phase. It proves real stereo/tonal changes through AudioRecord.
It is not a music-player policy or direct Milkbeat PCM integration qualification.
Pressing Home ended capture and left no active MediaProjection or capture
service. Full revocation/resume/seek/stall coverage is still pending.
