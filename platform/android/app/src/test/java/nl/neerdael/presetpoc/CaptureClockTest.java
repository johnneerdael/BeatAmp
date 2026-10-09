package nl.neerdael.presetpoc;

import org.junit.Test;
import static org.junit.Assert.*;

public class CaptureClockTest {
    @Test public void timestampMapsReadPositionWithoutInventingReceiptLatency() {
        assertEquals(1_010_000_000L, CaptureClock.endNanos(480, 0, 1_000_000_000L, 48000));
        assertEquals(990_000_000L, CaptureClock.endNanos(0, 480, 1_000_000_000L, 48000));
        assertEquals(1_000_000_000L, CaptureClock.endNanos(441, 441, 1_000_000_000L, 44100));
    }
    @Test public void missingTimestampUsesExplicitReceiptEstimate() {
        assertEquals(9_999L, CaptureClock.estimatedEndNanos(9_999L));
    }
    @Test(expected = IllegalArgumentException.class) public void invalidRateCannotMakeTimestamp() {
        CaptureClock.endNanos(0, 0, 0, 0);
    }
}
