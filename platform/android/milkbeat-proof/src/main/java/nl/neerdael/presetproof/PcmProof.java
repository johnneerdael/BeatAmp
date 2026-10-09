package nl.neerdael.presetproof;

import android.app.Instrumentation;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

/** Isolated controlled-source instrumentation. Calls existing public Milkbeat APIs;
 * does not patch its player, capture policy, audio processor or production sources. */
public final class PcmProof extends Instrumentation {
    private Object manager,player;private Class<?> managerClass;
    @Override public void onCreate(Bundle arguments){super.onCreate(arguments);start();}
    @Override public void onStart(){
        Bundle result=new Bundle();File fixture=null;
        try {
            Context context=getTargetContext();ClassLoader loader=context.getClassLoader();
            fixture=new File(context.getCacheDir(),"preset-poc-stereo.wav");writeFixture(fixture,180);
            Uri uri=Uri.fromFile(fixture);
            Class<?> ids=loader.loadClass("io.github.aedev.flow.data.localmedia.LocalMediaIds");
            String id=(String)ids.getMethod("of",Uri.class).invoke(ids.getField("INSTANCE").get(null),uri);
            Class<?> trackClass=loader.loadClass("io.github.aedev.flow.data.music.model.MusicTrack");
            Constructor<?> constructor=Arrays.stream(trackClass.getConstructors()).filter(c->{Class<?>[]p=c.getParameterTypes();return p.length>5&&p[p.length-1].getName().equals("kotlin.jvm.internal.DefaultConstructorMarker");}).findFirst().orElseThrow();
            Class<?>[] types=constructor.getParameterTypes();Object[] args=new Object[types.length];
            for(int i=0;i<types.length;i++)args[i]=types[i]==int.class?0:types[i]==long.class?0L:types[i]==boolean.class?false:null;
            args[0]=id;args[1]="Controlled stereo bass and melody fixture";args[2]="Preset PoC";args[3]="";args[4]=180;args[types.length-2]=-32;
            Object track=constructor.newInstance(args);
            managerClass=loader.loadClass("io.github.aedev.flow.player.EnhancedMusicPlayerManager");manager=managerClass.getField("INSTANCE").get(null);
            runOnMainSync(()->invoke(manager,"initialize",new Class<?>[]{Context.class},context));
            long deadline=System.currentTimeMillis()+20000;
            while(player==null&&System.currentTimeMillis()<deadline){runOnMainSync(()->player=invoke(manager,"getPlayer",new Class<?>[]{}));Thread.sleep(100);}
            if(player==null)throw new IllegalStateException("Milkbeat player did not connect");
            Method play=Arrays.stream(managerClass.getMethods()).filter(m->m.getName().equals("playTrack")&&m.getParameterCount()==7&&m.getParameterTypes()[1]==String.class).findFirst().orElseThrow();
            runOnMainSync(()->{try{play.invoke(manager,track,uri.toString(),List.of(track),0,0L,"Preset PoC controlled local source",Map.of(id,uri));}catch(Exception e){throw new RuntimeException(e);}});
            boolean[] playing={false};deadline=System.currentTimeMillis()+20000;
            while(!playing[0]&&System.currentTimeMillis()<deadline){runOnMainSync(()->playing[0]=(Boolean)invoke(player,"isPlaying",new Class<?>[]{}));Thread.sleep(100);}
            if(!playing[0])throw new IllegalStateException("Milkbeat controlled source did not play");
            Log.i("MilkbeatPcmProof","PLAYING stereo 48000 Hz i16 local fixture through existing Milkbeat player; uri="+uri);
            result.putString("source","Milkbeat public local-file player path");result.putString("format","48k stereo i16 WAV");sendStatus(1,result);
            Thread.sleep(120000);
            result.putString("status","controlled Milkbeat playback completed");finish(0,result);
        }catch(Throwable e){Log.e("MilkbeatPcmProof","Proof failed",e);result.putString("error",e.toString());finish(1,result);}
    }
    private static Object invoke(Object object,String name,Class<?>[]types,Object...args){try{return object.getClass().getMethod(name,types).invoke(object,args);}catch(Exception e){throw new RuntimeException(e);}}
    private static void writeFixture(File file,int seconds)throws IOException {
        try(OutputStream out=new BufferedOutputStream(new FileOutputStream(file))){int rate=48000;int data=rate*seconds*4;out.write("RIFF".getBytes());le(out,data+36,4);out.write("WAVEfmt ".getBytes());le(out,16,4);le(out,1,2);le(out,2,2);le(out,rate,4);le(out,rate*4,4);le(out,4,2);le(out,16,2);out.write("data".getBytes());le(out,data,4);
            for(int frame=0;frame<rate*seconds;frame++){double t=frame/(double)rate;double kick=.4*Math.exp(-(t%.5)*45)*Math.sin(2*Math.PI*65*t);double bass=.18*Math.sin(2*Math.PI*(110+(int)(t/4)%4*13.75)*t);double note=new double[]{440,523.251,659.255,587.33}[(int)(t/4)%4];double lead=.25*Math.sin(2*Math.PI*note*t);le(out,(short)((kick+bass)*32767),2);le(out,(short)(lead*32767),2);}
        }
    }
    private static void le(OutputStream out,int value,int bytes)throws IOException{for(int i=0;i<bytes;i++)out.write(value>>>(i*8));}
}
