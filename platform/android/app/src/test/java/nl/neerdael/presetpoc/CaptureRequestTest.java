package nl.neerdael.presetpoc;

import org.junit.Test;
import static org.junit.Assert.*;

public class CaptureRequestTest {
    @Test public void consentDialogBackgroundDoesNotCancelRequest() {
        CaptureRequest request = new CaptureRequest();
        assertTrue(request.stopOnBackground());
        assertTrue(request.begin(true));
        assertFalse(request.stopOnBackground());
        assertTrue(request.isPending());
        assertTrue(request.fixture());
        request.finish();
        assertTrue(request.stopOnBackground());
        assertFalse(request.isPending());
    }
    @Test public void repeatedStartDoesNotReplaceThePendingSource() {
        CaptureRequest request = new CaptureRequest();
        assertTrue(request.begin(false));
        assertFalse(request.begin(true));
        assertFalse(request.fixture());
        request.finish();
        assertTrue(request.begin(true));
        assertTrue(request.fixture());
    }
}
