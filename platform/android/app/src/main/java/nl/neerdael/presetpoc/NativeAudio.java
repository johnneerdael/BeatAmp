package nl.neerdael.presetpoc;

import java.nio.ByteBuffer;

final class NativeAudio {
    static { System.loadLibrary("preset_android"); }
    static native long create(int sampleRate, int channels, long generation);
    static native boolean submit(long handle, ByteBuffer pcm, int bytes, long startFrame, long captureEndNanos, boolean timestampEstimated);
    static native String report(long handle);
    static native void destroy(long handle);
}
