package nl.neerdael.presetpoc;

import android.media.*;
import android.media.projection.MediaProjection;
import android.os.Process;
import org.json.JSONObject;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.concurrent.atomic.AtomicLong;

/** AudioRecord owns capture; all musical analysis is performed by the Rust worker. */
final class PcmCapture implements AutoCloseable {
    private static final AtomicLong GENERATIONS = new AtomicLong();
    private final AudioRecord recorder;
    private final long handle;
    private final Thread thread;
    private final boolean fixture;
    final int sampleRate, channels, bufferFrames;
    final long generation = GENERATIONS.incrementAndGet();
    private volatile boolean running = true;
    private volatile String error;
    private volatile long framesRead, reads, nativeDrops, estimatedTimestamps, timestampedReads;
    private volatile long lastReceiptNanos, maxReadNanos;

    PcmCapture(MediaProjection projection, boolean fixture) {
        this.fixture = fixture;
        AudioPlaybackCaptureConfiguration.Builder capture = new AudioPlaybackCaptureConfiguration.Builder(projection)
                .addMatchingUsage(AudioAttributes.USAGE_MEDIA)
                .addMatchingUsage(AudioAttributes.USAGE_GAME)
                .addMatchingUsage(AudioAttributes.USAGE_UNKNOWN);
        if (fixture) capture.addMatchingUid(Process.myUid());
        else capture.excludeUid(Process.myUid());
        AudioPlaybackCaptureConfiguration config = capture.build();
        AudioRecord chosen = null;
        String failures = "";
        for (int rate : new int[]{48000, 44100}) {
            AudioRecord candidate = null;
            try {
                int minimum = AudioRecord.getMinBufferSize(rate, AudioFormat.CHANNEL_IN_STEREO, AudioFormat.ENCODING_PCM_16BIT);
                if (minimum <= 0) throw new IllegalStateException("PCM format unsupported: " + minimum);
                candidate = new AudioRecord.Builder()
                        .setAudioFormat(new AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                                .setSampleRate(rate).setChannelMask(AudioFormat.CHANNEL_IN_STEREO).build())
                        .setBufferSizeInBytes(Math.max(minimum, 256 * 2 * 2 * 8))
                        .setAudioPlaybackCaptureConfig(config).build();
                if (candidate.getState() != AudioRecord.STATE_INITIALIZED) throw new IllegalStateException("AudioRecord initialization failed");
                chosen = candidate; break;
            } catch (RuntimeException e) {
                if (candidate != null) candidate.release();
                failures += rate + " Hz stereo: " + e + "; ";
            }
        }
        if (chosen == null) throw new IllegalStateException("Continuous stereo playback PCM unavailable: " + failures);
        recorder = chosen;
        sampleRate = recorder.getSampleRate(); channels = recorder.getChannelCount(); bufferFrames = recorder.getBufferSizeInFrames();
        if (channels != 2 || recorder.getAudioFormat() != AudioFormat.ENCODING_PCM_16BIT) {
            recorder.release(); throw new IllegalStateException("Unexpected negotiated playback format");
        }
        long created = NativeAudio.create(sampleRate, channels, generation);
        if (created == 0) { recorder.release(); throw new IllegalStateException("Rust PCM worker could not initialize"); }
        handle = created;
        thread = new Thread(this::readLoop, "playback-pcm");
        try {
            recorder.startRecording();
            if (recorder.getRecordingState() != AudioRecord.RECORDSTATE_RECORDING) throw new IllegalStateException("Playback capture did not start");
            thread.start();
        } catch (RuntimeException e) {
            NativeAudio.destroy(handle); recorder.release(); throw e;
        }
    }

    private void readLoop() {
        Process.setThreadPriority(Process.THREAD_PRIORITY_AUDIO);
        ByteBuffer pcm = ByteBuffer.allocateDirect(256 * channels * 2).order(ByteOrder.LITTLE_ENDIAN);
        AudioTimestamp stamp = new AudioTimestamp();
        try {
            while (running) {
                pcm.clear();
                long before = System.nanoTime();
                int bytes = recorder.read(pcm, pcm.capacity(), AudioRecord.READ_BLOCKING);
                long received = System.nanoTime();
                maxReadNanos = Math.max(maxReadNanos, received - before);
                if (!running) break;
                if (bytes < 0) throw new IllegalStateException("Playback PCM read failed: " + bytes);
                if (bytes == 0) { Thread.sleep(2); continue; }
                if (bytes % (channels * 2) != 0) throw new IllegalStateException("Partial stereo PCM frame");
                long start = framesRead;
                framesRead += bytes / (channels * 2);
                boolean estimated = recorder.getTimestamp(stamp, AudioTimestamp.TIMEBASE_MONOTONIC) != AudioRecord.SUCCESS;
                long endNs;
                if (estimated) { endNs = CaptureClock.estimatedEndNanos(received); estimatedTimestamps++; }
                else { endNs = CaptureClock.endNanos(framesRead, stamp.framePosition, stamp.nanoTime, sampleRate); timestampedReads++; }
                if (endNs < 0 || endNs > received + 100_000_000L) throw new IllegalStateException("Invalid platform sample timestamp");
                if (!NativeAudio.submit(handle, pcm, bytes, start, endNs, estimated)) nativeDrops++;
                reads++; lastReceiptNanos = received;
            }
        } catch (Exception e) {
            if (running) { error = e.toString(); running = false; }
        }
    }

    JSONObject report() {
        JSONObject output = new JSONObject();
        try {
            output.put("source", "audio-playback-capture"); output.put("encoding", "signed-i16-le-interleaved");
            output.put("sample_rate", sampleRate); output.put("channels", channels); output.put("buffer_frames", bufferFrames);
            output.put("generation", generation); output.put("fixture", fixture);
            output.put("uid_filter", fixture ? "own-uid-only" : "exclude-own-uid");
            output.put("frames_read", framesRead); output.put("reads", reads); output.put("native_drops", nativeDrops);
            output.put("timestamped_reads", timestampedReads); output.put("estimated_timestamp_reads", estimatedTimestamps);
            output.put("max_read_ns", maxReadNanos); output.put("receipt_ns", System.nanoTime());
            output.put("last_receipt_ns", lastReceiptNanos);
            JSONObject worker = new JSONObject(NativeAudio.report(handle)); output.put("worker", worker);
            JSONObject snapshot = worker.optJSONObject("snapshot");
            double rms = snapshot == null ? 0 : snapshot.optDouble("rms", 0);
            boolean stale = lastReceiptNanos == 0 || System.nanoTime() - lastReceiptNanos > 100_000_000L;
            output.put("state", error != null ? "failed" : stale ? "waiting-for-pcm" : rms > 0.0001 ? "pcm-signal" : "no-signal");
            output.put("error", error == null ? JSONObject.NULL : error);
        } catch (Exception e) { throw new IllegalStateException("PCM report failure", e); }
        return output;
    }

    boolean isRunning() { return running; }

    @Override public void close() {
        running = false;
        try { recorder.stop(); } catch (IllegalStateException ignored) { /* revoked capture may already be stopped */ }
        boolean interrupted = false;
        while (thread.isAlive()) {
            try { thread.join(100); } catch (InterruptedException e) { interrupted = true; }
        }
        recorder.release(); NativeAudio.destroy(handle);
        if (interrupted) Thread.currentThread().interrupt();
    }
}
