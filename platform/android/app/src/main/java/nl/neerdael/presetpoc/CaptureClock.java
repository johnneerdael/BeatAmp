package nl.neerdael.presetpoc;

/** Maps AudioRecord frame positions onto the monotonic sample clock. */
final class CaptureClock {
    static long endNanos(long framesRead, long timestampFrame, long timestampNanos, int rate) {
        if (rate <= 0 || framesRead < 0 || timestampFrame < 0 || timestampNanos < 0)
            throw new IllegalArgumentException("Invalid sample clock");
        long delta = Math.subtractExact(framesRead, timestampFrame);
        long seconds = delta / rate;
        long remainder = delta % rate;
        return Math.addExact(timestampNanos, Math.addExact(Math.multiplyExact(seconds, 1_000_000_000L), remainder * 1_000_000_000L / rate));
    }
    static long estimatedEndNanos(long receiptNanos) { return receiptNanos; }
}
