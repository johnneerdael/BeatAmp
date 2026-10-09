package nl.neerdael.presetpoc;

/** Consent dialogs may stop the Activity; they must not cancel their own request. */
final class CaptureRequest {
    private boolean pending;
    private boolean fixture;

    boolean begin(boolean fixture) {
        if (pending) return false;
        this.fixture = fixture;
        pending = true;
        return true;
    }
    boolean isPending() { return pending; }
    boolean fixture() { return fixture; }
    void finish() { pending = false; }
    boolean stopOnBackground() { return !pending; }
}
