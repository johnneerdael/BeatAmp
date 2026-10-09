package nl.neerdael.presetpoc;

import java.util.Locale;

final class CaptureText {
    static String status(String state, boolean fixture) {
        return switch (state) {
            case "pcm-signal" -> fixture ? "Test playback PCM received" : "Playback PCM received";
            case "no-signal" -> fixture ? "Test silence" : "No signal · player silent or not capturable";
            case "requesting-consent" -> "Waiting for capture consent";
            case "waiting-for-pcm" -> "Waiting for continuous playback PCM";
            case "revoked" -> "Capture consent ended";
            case "permission-denied" -> "Playback audio permission denied";
            case "consent-denied" -> "Capture consent declined";
            case "failed" -> "Playback capture unavailable";
            case "stopped" -> "Capture stopped";
            default -> "Capture state unavailable";
        };
    }

    static String optionalNumber(double value, int decimals) {
        if (!Double.isFinite(value)) return "unavailable";
        return String.format(Locale.US, "%." + decimals + "f", value);
    }
}
