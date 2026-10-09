package nl.neerdael.presetreference;

import android.app.Activity;
import android.os.Bundle;
import android.opengl.GLSurfaceView;
import android.opengl.GLES20;
import android.opengl.GLES30;
import android.graphics.Bitmap;
import android.graphics.Matrix;
import android.util.Log;
import android.view.WindowManager;
import nl.neerdael.projectm.core.ProjectMJNI;
import java.io.*;
import java.nio.ByteBuffer;
import java.util.LinkedHashMap;
import java.util.Locale;
import org.json.JSONObject;
import javax.microedition.khronos.opengles.GL10;
import javax.microedition.khronos.egl.EGLConfig;

/** Independent adapter; the identified shipping engine binary is unchanged. */
public final class ReferenceActivity extends Activity {
    private GLSurfaceView view;
    private volatile nl.neerdael.presetrecording.FrameRecorder activeRecorder;
    private volatile java.io.RandomAccessFile activePlaylist;
    private volatile boolean recordingCancelled;
    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        if(getIntent().getBooleanExtra("probe_codec",false)){Log.i("PresetCodec",nl.neerdael.presetrecording.FrameRecorder.capabilities());finish();return;}
        int rank=getIntent().getIntExtra("rank",1);
        boolean common=getIntent().getBooleanExtra("common_audio",false);
        int captureFrame=getIntent().getIntExtra("capture_frame",0);
        int recordFrames=getIntent().getIntExtra("record_frames",0),recordPort=getIntent().getIntExtra("record_port",26101),inputOffset=getIntent().getIntExtra("input_start_frame",0);
        try {
            File pack=new File(getFilesDir(),"reference-"+rank);pack.mkdirs();
            String name=new JSONObject(new String(getAssets().open("catalog.json").readAllBytes(),java.nio.charset.StandardCharsets.UTF_8)).getString(Integer.toString(rank));
            File storage=new File(pack,"0");storage.mkdirs();
            try(InputStream in=getAssets().open(String.format(Locale.US,"presets/%03d.milk",rank));OutputStream out=new FileOutputStream(new File(storage,"0.milk"))){in.transferTo(out);}
            try(FileWriter index=new FileWriter(new File(pack,"presets.idx"))){index.write("0/0.milk\t"+name+"\n");}
            java.io.RandomAccessFile playlist=recordFrames>0?new java.io.RandomAccessFile(new File(getFilesDir(),"playlist/reference.bin"),"r"):null;
            activePlaylist=playlist;
            byte[] commonPcm=common?getAssets().open("audio/reference-input.bin").readAllBytes():null;
            if(common&&commonPcm.length!=480*576)throw new IOException("Common PCM profile length mismatch");
            ProjectMJNI.init(getAssets(),new File(getFilesDir(),"reference-skip.json").getAbsolutePath(),new File(getFilesDir(),"reference-textures").getAbsolutePath());
            ProjectMJNI.setCustomPresetPack(pack.getAbsolutePath());ProjectMJNI.setMusicCategory("custom");
            ProjectMJNI.setAutoChange(false);ProjectMJNI.setBeatCuts(false);ProjectMJNI.setBlankDetection(false);
            ProjectMJNI.setSoftCutDuration(0);ProjectMJNI.setNativeTrails(-1);ProjectMJNI.setMeshSize(48,32);
            view=new GLSurfaceView(this);view.setEGLContextClientVersion(3);view.getHolder().setFixedSize(3840,2160);
            view.setRenderer(new GLSurfaceView.Renderer(){
                long started,last,next,reported,frame;int width,height;boolean captured;byte[]wave=new byte[576];nl.neerdael.presetrecording.FrameRecorder recorder;ByteBuffer pixels;
                @Override public void onSurfaceCreated(GL10 gl,EGLConfig config){ProjectMJNI.onSurfaceCreated();started=System.nanoTime();next=started+33333333;
                    if(recordFrames>0)try{recorder=createRecorder(recordPort);pixels=ByteBuffer.allocateDirect(3840*2160*4);}catch(Exception e){Log.e("PresetMovie","Recorder failed",e);captured=true;}
                }
                @Override public void onSurfaceChanged(GL10 gl,int w,int h){width=w;height=h;ProjectMJNI.onSurfaceChanged(w,h);ProjectMJNI.configureRenderBudget(3840,2160,-1,0,1);}
                @Override public void onDrawFrame(GL10 gl){
                    if(captured||(recordFrames>0&&recordingCancelled))return;
                    long wait=next-System.nanoTime();if(wait>0)try{Thread.sleep(wait/1000000,(int)(wait%1000000));}catch(InterruptedException ignored){}
                    long now=System.nanoTime();
                    if(!ProjectMJNI.isCustomPresetPackPending()&&ProjectMJNI.getCategoryPresetCount("custom")==1&&!ProjectMJNI.getMusicCategory().equals("custom"))ProjectMJNI.setMusicCategory("custom");
                    double time=(now-started)/1e9;
                    if(playlist!=null){try{playlist.seek((inputOffset+frame)*576L);playlist.readFully(wave);}catch(Exception e){Log.e("PresetMovie","Playlist read failed",e);captured=true;if(recorder!=null)recorder.close();return;}}
                    else if(common){System.arraycopy(commonPcm,(int)(frame%480)*576,wave,0,576);}
                    else{for(int i=0;i<wave.length;i++){double t=time+i/44100.0;double v=.175*Math.sin(2*Math.PI*110*t)+.125*Math.sin(2*Math.PI*440*t);wave[i]=(byte)Math.max(0,Math.min(255,128+(int)(v*127)));}}
                    ProjectMJNI.addWaveform(wave,wave.length);ProjectMJNI.onDrawFrame();
                    if(recordFrames>0&&!ProjectMJNI.getCurrentPresetName().endsWith(name))return;
                    long end=System.nanoTime();next=Math.max(next+33333333,end);frame++;
                    LinkedHashMap<String,Object> fields=new LinkedHashMap<>();fields.put("rank",rank);fields.put("source",name);fields.put("current",ProjectMJNI.getCurrentPresetName());fields.put("surface",width+"x"+height);fields.put("frame",frame);fields.put("cpu_frame_ms",(end-now)/1e6);fields.put("interval_ms",last==0?0:(now-last)/1e6);fields.put("native_trails",ProjectMJNI.getNativeTrailsStatus());fields.put("input",playlist!=null?"Beatport same44100 stereo source derivedu8 mono576 snapshots":common?"common48k stereo PCM derived u8 mono44100; end-aligned576 snapshots":"derived u8 mono44100 sine snapshot");fields.put("clock_rng","uncontrolled shipping core");fields.put("version",ProjectMJNI.getVersion());
                    String report=new JSONObject(fields).toString();
                    if(end-reported>1000000000L){Log.i("PresetReference",report);reported=end;}last=now;
                    if(recorder!=null){try{
                        if(width!=3840||height!=2160)throw new IOException("Recording lost native4K dimensions");
                        readNative(width,height,pixels);
                        recorder.frame(pixels);
                        if(frame>=recordFrames){Log.i("PresetMovie",recorder.finish(recordFrames));recorder.close();playlist.close();captured=true;view.setRenderMode(GLSurfaceView.RENDERMODE_WHEN_DIRTY);}
                    }catch(Exception e){Log.e("PresetMovie","Recording failed",e);recorder.close();captured=true;view.setRenderMode(GLSurfaceView.RENDERMODE_WHEN_DIRTY);} }
                    if(captureFrame>0&&frame>=captureFrame){
                        captured=true;view.setRenderMode(GLSurfaceView.RENDERMODE_WHEN_DIRTY);
                        try {
                            if(width!=3840||height!=2160||!ProjectMJNI.getCurrentPresetName().endsWith(name))throw new IOException("Reference dimensions/source identity mismatch");
                            ByteBuffer pixels=ByteBuffer.allocateDirect(width*height*4);
                            readNative(width,height,pixels);
                            pixels.rewind();Bitmap bitmap=Bitmap.createBitmap(width,height,Bitmap.Config.ARGB_8888);bitmap.copyPixelsFromBuffer(pixels);
                            Matrix flip=new Matrix();flip.setScale(1,-1);Bitmap oriented=Bitmap.createBitmap(bitmap,0,0,width,height,flip,false);
                            File folder=new File(getFilesDir(),"captures");folder.mkdirs();
                            try(OutputStream output=new FileOutputStream(new File(folder,String.format(Locale.US,"%03d.jpg",rank)))){oriented.compress(Bitmap.CompressFormat.JPEG,92,output);}
                            oriented.recycle();bitmap.recycle();
                            fields.put("capture_backend","native GLES RGBA8 readback JPEG92; row reversal");fields.put("performance_sample",false);
                            Log.i("PresetReferenceCapture",new JSONObject(fields).toString());
                        }catch(Exception e){Log.e("PresetReferenceCapture","failed: "+e);}
                    }
                }
            });setContentView(view);
        }catch(Exception e){Log.e("PresetReference","Reference adapter failed",e);finish();}
    }
    private static void readNative(int width,int height,ByteBuffer pixels)throws IOException {
        int[] framebuffer=new int[1],packBuffer=new int[1],alignment=new int[1],samples=new int[1];
        // The core can leave a read FBO/PBO or earlier GL error pending.
        // Read the actual displayed default framebuffer, preserving its state.
        GLES30.glGetIntegerv(GLES30.GL_SAMPLES,samples,0);
        GLES30.glGetIntegerv(GLES30.GL_READ_FRAMEBUFFER_BINDING,framebuffer,0);
        GLES30.glGetIntegerv(GLES30.GL_PIXEL_PACK_BUFFER_BINDING,packBuffer,0);
        GLES30.glGetIntegerv(GLES20.GL_PACK_ALIGNMENT,alignment,0);
        GLES30.glBindFramebuffer(GLES30.GL_READ_FRAMEBUFFER,0);GLES30.glBindBuffer(GLES30.GL_PIXEL_PACK_BUFFER,0);GLES20.glPixelStorei(GLES20.GL_PACK_ALIGNMENT,1);
        int previous=0;for(int i=0;i<32;i++){int error=GLES20.glGetError();if(error==0)break;previous=error;}
        if(previous!=0)Log.i("PresetReferenceState","pre-capture GL error "+previous+"; readFBO="+framebuffer[0]+" packPBO="+packBuffer[0]);
        pixels.rewind();GLES20.glReadPixels(0,0,width,height,GLES20.GL_RGBA,GLES20.GL_UNSIGNED_BYTE,pixels);int error=GLES20.glGetError();
        GLES30.glBindFramebuffer(GLES30.GL_READ_FRAMEBUFFER,framebuffer[0]);GLES30.glBindBuffer(GLES30.GL_PIXEL_PACK_BUFFER,packBuffer[0]);GLES20.glPixelStorei(GLES20.GL_PACK_ALIGNMENT,alignment[0]);
        if(error!=0)throw new IOException("Native GL readback error "+error+" samples="+samples[0]+" readFBO="+framebuffer[0]+" PBO="+packBuffer[0]);
    }
    private nl.neerdael.presetrecording.FrameRecorder createRecorder(int port)throws Exception {
        if(recordingCancelled)throw new IOException("Recording cancelled");
        closeRecorder();
        // Socket connection/bitmap allocation must not hold the UI lifecycle lock.
        nl.neerdael.presetrecording.FrameRecorder fresh=new nl.neerdael.presetrecording.FrameRecorder(port,3840,2160);
        synchronized(this){
            if(!recordingCancelled){activeRecorder=fresh;return fresh;}
        }
        // Cancellation can finish while construction is still in progress.
        fresh.close();throw new IOException("Recording cancelled during recorder construction");
    }
    private synchronized void closeRecorder(){nl.neerdael.presetrecording.FrameRecorder old=activeRecorder;activeRecorder=null;if(old!=null)old.close();}
    private void cancelRecording(){recordingCancelled=true;closeRecorder();java.io.RandomAccessFile old=activePlaylist;activePlaylist=null;if(old!=null)try{old.close();}catch(IOException ignored){}}
    @Override protected void onPause(){cancelRecording();if(view!=null)view.onPause();super.onPause();}
    @Override protected void onResume(){super.onResume();if(view!=null)view.onResume();}
    @Override protected void onDestroy(){cancelRecording();if(view!=null)view.queueEvent(ProjectMJNI::release);super.onDestroy();}
}
