package nl.neerdael.presetrecording;
import android.media.MediaCodecInfo;
import android.media.MediaCodecList;
import android.graphics.Bitmap;
import java.nio.ByteBuffer;
import java.net.Socket;
import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.util.concurrent.*;
import org.json.JSONObject;

/** Bounded ordered native4K transport. Capture work is not a playback benchmark. */
public final class FrameRecorder implements AutoCloseable {
    private final Socket socket;private final BufferedOutputStream output;
    private final Bitmap[] bitmaps=new Bitmap[3];
    private final ArrayBlockingQueue<Bitmap> pool=new ArrayBlockingQueue<>(3);
    private final ArrayBlockingQueue<Pending> pending=new ArrayBlockingQueue<>(3);
    private final ExecutorService workers=Executors.newFixedThreadPool(3);
    private final CountDownLatch done=new CountDownLatch(1);
    private final Object copyLock=new Object();private volatile boolean closed;
    private volatile Throwable failure;private volatile int written;private int frames;
    public final int width,height;
    private static final class Pending {final Future<byte[]> bytes;Pending(Future<byte[]> bytes){this.bytes=bytes;}}
    public FrameRecorder(int port,int width,int height)throws Exception {
        this.width=width;this.height=height;socket=new Socket("10.0.2.2",port);socket.setTcpNoDelay(true);
        output=new BufferedOutputStream(socket.getOutputStream(),1024*1024);
        for(int i=0;i<3;i++){bitmaps[i]=Bitmap.createBitmap(width,height,Bitmap.Config.ARGB_8888);pool.add(bitmaps[i]);}
        new Thread(()->{try{while(true){Pending item=pending.take();if(item.bytes==null)break;output.write(item.bytes.get());output.flush();written++;}}
            catch(Throwable e){failure=e;}finally{try{output.close();socket.close();}catch(Exception ignored){}done.countDown();}},"comparison-frame-writer").start();
    }
    public void frame(ByteBuffer pixels)throws Exception {
        if(closed)throw new IllegalStateException("Frame recorder closed");
        if(failure!=null)throw new IllegalStateException("Frame stream failed",failure);
        if(pixels==null||pixels.capacity()!=width*height*4)throw new IllegalStateException("Native frame dimensions mismatch");
        Bitmap bitmap=null;long deadline=System.nanoTime()+30_000_000_000L;while(bitmap==null&&!closed&&System.nanoTime()<deadline)bitmap=pool.poll(100,TimeUnit.MILLISECONDS);if(bitmap==null||closed)throw new IllegalStateException("JPEG queue closed or stalled");
        final Bitmap selected=bitmap;
        synchronized(copyLock){if(closed)throw new IllegalStateException("Recorder closed during copy");pixels.rewind();selected.copyPixelsFromBuffer(pixels);}
        Future<byte[]> future=workers.submit(()->{try{ByteArrayOutputStream bytes=new ByteArrayOutputStream();if(!selected.compress(Bitmap.CompressFormat.JPEG,92,bytes))throw new IllegalStateException("JPEG stream failed");return bytes.toByteArray();}finally{pool.offer(selected);}});
        if(!pending.offer(new Pending(future),30,TimeUnit.SECONDS))throw new IllegalStateException("Frame writer stalled");frames++;
    }
    public String finish(int expected)throws Exception {
        if(frames!=expected)throw new IllegalStateException("Input frame count mismatch");
        if(!pending.offer(new Pending(null),30,TimeUnit.SECONDS)||!done.await(60,TimeUnit.SECONDS))throw new IllegalStateException("Frame export completion timeout");
        if(failure!=null||written!=expected)throw new IllegalStateException("Frame export failed: "+written+" / "+expected,failure);
        return new JSONObject(java.util.Map.of("frames",written,"width",width,"height",height,"input_transport","nativeRGBA8 -> orderedJPEG92 -> hostVideoToolbox","performance_sample",false)).toString();
    }
    @Override public synchronized void close(){if(closed)return;closed=true;try{socket.close();}catch(Exception ignored){}pending.clear();pending.offer(new Pending(null));workers.shutdown();try{if(workers.awaitTermination(30,TimeUnit.SECONDS)){synchronized(copyLock){for(Bitmap bitmap:bitmaps)bitmap.recycle();}}}catch(InterruptedException e){Thread.currentThread().interrupt();}}
    public static String capabilities(){org.json.JSONArray rows=new org.json.JSONArray();
        for(MediaCodecInfo info:new MediaCodecList(MediaCodecList.REGULAR_CODECS).getCodecInfos()){
            if(!info.isEncoder())continue;for(String type:info.getSupportedTypes())if(type.equals("video/avc")||type.equals("video/hevc")){
                try{MediaCodecInfo.VideoCapabilities v=info.getCapabilitiesForType(type).getVideoCapabilities();rows.put(new JSONObject(java.util.Map.of("name",info.getName(),"mime",type,"4k30",v.areSizeAndRateSupported(3840,2160,30),"4k_size",v.isSizeSupported(3840,2160),"1080p30",v.areSizeAndRateSupported(1920,1080,30))));}catch(Exception ignored){}
            }
        }return rows.toString();
    }
}
