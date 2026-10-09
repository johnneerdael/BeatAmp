package nl.neerdael.presetpoc;

import org.junit.Test;
import static org.junit.Assert.*;

public class CaptureTextTest {
    @Test public void missingAndRejectedInputCannotLookLikeReceivedMusic() {
        assertEquals("Playback audio permission denied", CaptureText.status("permission-denied", false));
        assertEquals("Capture consent declined", CaptureText.status("consent-denied", false));
        assertEquals("No signal · player silent or not capturable", CaptureText.status("no-signal", false));
        assertEquals("Test playback PCM received", CaptureText.status("pcm-signal", true));
        assertEquals("Playback PCM received", CaptureText.status("pcm-signal", false));
        assertEquals("Capture state unavailable", CaptureText.status("unrecognized", false));
    }

    @Test public void unavailableMeasurementsDoNotAppearAsZeroOrNaN() {
        assertEquals("unavailable", CaptureText.optionalNumber(Double.NaN, 3));
        assertEquals("unavailable", CaptureText.optionalNumber(Double.POSITIVE_INFINITY, 3));
        assertEquals("0.000", CaptureText.optionalNumber(0.0, 3));
    }
}
