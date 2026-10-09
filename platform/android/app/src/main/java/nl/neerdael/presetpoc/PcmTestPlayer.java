package nl.neerdael.presetpoc;

import android.media.*;

/** Explicit test fixture played through Android and recaptured by AudioRecord. */
final class PcmTestPlayer implements AutoCloseable {
    private volatile boolean running = true;
    private final Thread thread;
    private final AudioTrack track;

    PcmTestPlayer() {
        int rate = 48000;
        track = new AudioTrack.Builder()
                .setAudioAttributes(new AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                        .setAllowedCapturePolicy(AudioAttributes.ALLOW_CAPTURE_BY_ALL).build())
                .setAudioFormat(new AudioFormat.Builder().setSampleRate(rate).setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                        .setChannelMask(AudioFormat.CHANNEL_OUT_STEREO).build())
                .setTransferMode(AudioTrack.MODE_STREAM).setBufferSizeInBytes(8192).build();
        if (track.getState() != AudioTrack.STATE_INITIALIZED) { track.release(); throw new IllegalStateException("Test playback unavailable"); }
        thread = new Thread(() -> {
            short[] pcm = new short[512]; long frame = 0;
            try {
                track.play();
                while (running) {
                    for (int i = 0; i < 256; i++, frame++) {
                        // Four phases: left bass / right A / right C / silence.
                        int phase = (int)(frame / (rate * 4L) % 4);
                        double left = phase == 0 ? 0.45 * Math.sin(2 * Math.PI * 110 * frame / rate) : 0;
                        double hz = phase == 1 ? 880 : 1046.502;
                        double right = phase == 1 || phase == 2 ? 0.30 * Math.sin(2 * Math.PI * hz * frame / rate) : 0;
                        pcm[i*2] = (short)(left*32767); pcm[i*2+1] = (short)(right*32767);
                    }
                    int offset = 0;
                    while (running && offset < pcm.length) {
                        int written = track.write(pcm, offset, pcm.length-offset, AudioTrack.WRITE_BLOCKING);
                        if (written <= 0) { running = false; break; }
                        offset += written;
                    }
                }
            } catch (RuntimeException e) { running = false; }
        }, "pcm-test-playback");
        thread.start();
    }

    @Override public void close() {
        running = false;
        try { track.pause(); track.flush(); } catch (IllegalStateException ignored) { }
        boolean interrupted = false;
        while (thread.isAlive()) { try { thread.join(100); } catch (InterruptedException e) { interrupted = true; } }
        track.release();
        if (interrupted) Thread.currentThread().interrupt();
    }
}
