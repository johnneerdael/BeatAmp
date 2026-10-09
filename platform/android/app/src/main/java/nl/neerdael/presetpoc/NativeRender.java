package nl.neerdael.presetpoc;
import android.view.Surface;
final class NativeRender {
    static { System.loadLibrary("preset_android"); }
    static native long create(Surface surface,String directory,int rank,int backend);
    static native String frame(long handle,int rank,int fixture,int adapted,long nowNanos);
    static native void inputOffset(long handle,long offset);
    static native java.nio.ByteBuffer pixels(long handle);
    static native String error();
    static native String capture(long handle,String path);
    static native void destroy(long handle);
}
