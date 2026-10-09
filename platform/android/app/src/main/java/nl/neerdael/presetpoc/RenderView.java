package nl.neerdael.presetpoc;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.util.Log;
import java.io.*;
import java.util.concurrent.atomic.AtomicInteger;
import org.json.JSONObject;

final class RenderView extends SurfaceView implements SurfaceHolder.Callback {
    private volatile boolean running;
    private Thread thread;
    private final AtomicInteger rank=new AtomicInteger(1);
    private volatile boolean fixture;
    private volatile boolean adapted;
    private volatile boolean comparison;
    private final boolean playlist;
    private final boolean nativeBeat;
    private final int recordFrames,recordPort,inputOffset;
    private final int sweepFrames;
    private final boolean captureSweep;
    private final int sweepCount;
    private final java.util.function.Consumer<String> status;
    private final Handler ui=new Handler(Looper.getMainLooper());
    private volatile String latest="{\"state\":\"loading\"}";
    RenderView(Context context,java.util.function.Consumer<String> status,int initialRank,boolean fixture) {
        super(context);android.content.Intent intent=((android.app.Activity)context).getIntent();nativeBeat=intent.getBooleanExtra("native_beat",false);playlist=intent.getBooleanExtra("render_playlist",false);recordFrames=intent.getIntExtra("record_frames",0);recordPort=intent.getIntExtra("record_port",26101);inputOffset=intent.getIntExtra("input_start_frame",0);sweepCount=Math.max(1,Math.min(100,((android.app.Activity)context).getIntent().getIntExtra("sweep_count",100)));comparison=((android.app.Activity)context).getIntent().getBooleanExtra("render_comparison",false);adapted=((android.app.Activity)context).getIntent().getBooleanExtra("adapted",true);this.status=status;this.fixture=fixture;rank.set(initialRank);sweepFrames=((android.app.Activity)context).getIntent().getIntExtra("sweep_frames",0);captureSweep=((android.app.Activity)context).getIntent().getBooleanExtra("capture_sweep",false);
        getHolder().setFixedSize(3840,2160);getHolder().addCallback(this);
    }
    void step(int delta){rank.updateAndGet(r->Math.floorMod(r-1+delta,100)+1);}
    int currentRank(){return rank.get();}
    void toggleAdaptation(){adapted=!adapted;}
    boolean isAdapted(){return adapted;}
    void setFixture(boolean fixture){comparison=false;this.fixture=fixture;}
    void start(){if(getHolder().getSurface().isValid())surfaceChanged(getHolder(),0,3840,2160);}
    @Override public void surfaceCreated(SurfaceHolder holder){ }
    @Override public void surfaceChanged(SurfaceHolder holder,int format,int width,int height){
        if(thread!=null&&thread.isAlive())return;
        running=true;thread=new Thread(()->render(holder),"native-preset-render");thread.start();
    }
    @Override public void surfaceDestroyed(SurfaceHolder holder){stop();}
    void stop(){running=false;if(thread!=null){boolean interrupted=false;while(thread.isAlive()){try{thread.join(100);}catch(InterruptedException e){interrupted=true;}}thread=null;if(interrupted)Thread.currentThread().interrupt();}}
    private void render(SurfaceHolder holder){
        long handle=0;nl.neerdael.presetrecording.FrameRecorder recorder=null;
        java.util.ArrayList<String> timing=new java.util.ArrayList<>();
        try {
            String assetDirectory=nativeBeat?"beat":"poc";
            File destination=new File(getContext().getFilesDir(),assetDirectory);copyAssets(assetDirectory,destination);
            handle=nativeCreate(holder.getSurface(),destination.getAbsolutePath(),rank.get(),((android.app.Activity)getContext()).getIntent().getBooleanExtra("record_gl",false)?1:0);
            if(handle==0)throw new IllegalStateException(nativeError());
            if(playlist){nativeInputOffset(handle,inputOffset);recorder=new nl.neerdael.presetrecording.FrameRecorder(recordPort,3840,2160);}
            long next=System.nanoTime(),lastReport=0;int completed=0;
            while(running){
                long wait=next-System.nanoTime();if(wait>0){Thread.sleep(wait/1_000_000L,(int)(wait%1_000_000L));}
                latest=nativeFrame(handle,rank.get(),playlist?3:comparison?2:fixture?1:0,adapted?1:0,System.nanoTime());
                JSONObject parsed=new JSONObject(latest);
                if(recorder!=null){if(parsed.optString("state").equals("failed"))throw new IllegalStateException(latest);long captureStart=System.nanoTime();java.nio.ByteBuffer nativePixels=nativePixels(handle);long captureEnd=System.nanoTime();recorder.frame(nativePixels);long queued=System.nanoTime();if(parsed.optLong("frame")%30==0)Log.i("PresetCaptureStage","readback_ms="+(captureEnd-captureStart)/1e6+" bitmap_queue_ms="+(queued-captureEnd)/1e6);if(parsed.optLong("frame")>=recordFrames){Log.i("PresetMovie",recorder.finish(recordFrames));running=false;break;}}

                if(sweepFrames>0&&timing.size()<100_000)timing.add(latest);
                if(sweepFrames>0&&(parsed.optLong("frame")>=sweepFrames||parsed.optString("state").equals("failed"))){Log.i("PresetSweep",latest);if(captureSweep&&!parsed.optString("state").equals("failed")){File captures=new File(getContext().getFilesDir(),nativeBeat?"captures-beat":"captures");captures.mkdirs();Log.i("PresetCapture",nativeCapture(handle,new File(captures,String.format(java.util.Locale.US,"%03d.jpg",rank.get())).getAbsolutePath()));}completed++;if(completed==sweepCount){running=false;break;}step(1);}
                long now=System.nanoTime();next=Math.max(next+((comparison||playlist||nativeBeat)?33_333_333L:16_666_667L),now);
                if(now-lastReport>1_000_000_000L){String report=latest;Log.i("PresetRender",report);ui.post(()->status.accept(report));lastReport=now;}
                if(sweepFrames==0&&new JSONObject(latest).optString("state").equals("failed"))throw new IllegalStateException(latest);
            }
        }catch(Exception | LinkageError e){latest="{\"state\":\"failed\",\"error\":"+JSONObject.quote(e.toString())+"}";String report=latest;Log.e("PresetRender",report,e);ui.post(()->status.accept(report));}
        finally{if(recorder!=null)recorder.close();if(handle!=0)nativeDestroy(handle);
            if(sweepFrames>0){try(java.io.BufferedWriter writer=new java.io.BufferedWriter(new java.io.FileWriter(new File(getContext().getFilesDir(),nativeBeat?"beat-sweep-timing.jsonl":"sweep-timing.jsonl")))){for(String row:timing){writer.write(row);writer.newLine();}}catch(IOException e){Log.e("PresetSweep","Timing export failed",e);}}
        }
    }
    private long nativeCreate(android.view.Surface surface,String directory,int rank,int backend) {
        return nativeBeat?NativeBeat.create(surface,directory,rank,backend):NativeRender.create(surface,directory,rank,backend);
    }
    private String nativeFrame(long handle,int rank,int fixture,int adapted,long nowNanos) {
        return nativeBeat?NativeBeat.frame(handle,rank,fixture,adapted,nowNanos):NativeRender.frame(handle,rank,fixture,adapted,nowNanos);
    }
    private void nativeInputOffset(long handle,long offset) {
        if(nativeBeat)NativeBeat.inputOffset(handle,offset);else NativeRender.inputOffset(handle,offset);
    }
    private java.nio.ByteBuffer nativePixels(long handle) {
        return nativeBeat?NativeBeat.pixels(handle):NativeRender.pixels(handle);
    }
    private String nativeError() {
        return nativeBeat?NativeBeat.error():NativeRender.error();
    }
    private String nativeCapture(long handle,String path) {
        return nativeBeat?NativeBeat.capture(handle,path):NativeRender.capture(handle,path);
    }
    private void nativeDestroy(long handle) {
        if(nativeBeat)NativeBeat.destroy(handle);else NativeRender.destroy(handle);
    }
    private void copyAssets(String source,File destination)throws IOException{
        String[] children=getContext().getAssets().list(source);
        if(children!=null&&children.length>0){if(!destination.exists()&&!destination.mkdirs())throw new IOException("Asset directory creation failed");for(String child:children)copyAssets(source+"/"+child,new File(destination,child));}
        else{try(InputStream input=getContext().getAssets().open(source);OutputStream output=new FileOutputStream(destination)){input.transferTo(output);}}
    }
}
