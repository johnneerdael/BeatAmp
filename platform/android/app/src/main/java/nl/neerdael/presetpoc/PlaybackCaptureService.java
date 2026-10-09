package nl.neerdael.presetpoc;

import android.app.*;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.media.projection.*;
import android.os.*;
import android.util.Log;
import org.json.JSONObject;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;

public final class PlaybackCaptureService extends Service {
    static final String STOP = "stop";
    static volatile String latest = "{\"state\":\"stopped\"}";
    private final Handler main = new Handler(Looper.getMainLooper());
    private MediaProjection projection;
    private PcmCapture capture;
    private boolean evidence;
    private int evidenceLines;
    private File evidenceFile;
    private final MediaProjection.Callback revoked = new MediaProjection.Callback() {
        @Override public void onStop() {
            latest = "{\"state\":\"revoked\",\"reason\":\"Playback capture consent ended\"}";
            stopSelf();
        }
    };
    private final Runnable telemetry = new Runnable() {
        @Override public void run() {
            if (capture == null) return;
            latest = capture.report().toString(); Log.i("PresetPcm", latest);
            if (evidence && evidenceLines < 180) {
                try (FileOutputStream output = new FileOutputStream(evidenceFile, true)) {
                    output.write((latest + "\n").getBytes(StandardCharsets.UTF_8)); evidenceLines++;
                } catch (Exception e) { Log.e("PresetPcm", "Evidence write failed", e); evidence = false; }
            }
            if (!capture.isRunning()) { stopSelf(); return; }
            main.postDelayed(this, 1000);
        }
    };

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent == null || STOP.equals(intent.getAction())) { stopSelf(); return START_NOT_STICKY; }
        if (capture != null) return START_NOT_STICKY;
        try {
            NotificationManager notifications = getSystemService(NotificationManager.class);
            notifications.createNotificationChannel(new NotificationChannel("capture", "Playback audio capture", NotificationManager.IMPORTANCE_LOW));
            Intent stop = new Intent(this, PlaybackCaptureService.class).setAction(STOP);
            PendingIntent stopAction = PendingIntent.getService(this, 1, stop, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
            Notification notification = new Notification.Builder(this, "capture")
                    .setSmallIcon(android.R.drawable.ic_media_play).setContentTitle("Beatamp: playback PCM")
                    .setContentText("Continuous stereo audio for visual effects")
                    .addAction(new Notification.Action.Builder(null, "Stop capture", stopAction).build()).build();
            startForeground(1, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION);
            MediaProjectionManager manager = getSystemService(MediaProjectionManager.class);
            projection = manager.getMediaProjection(intent.getIntExtra("result_code", Activity.RESULT_CANCELED), intent.getParcelableExtra("result_data"));
            if (projection == null) throw new IllegalStateException("Playback capture consent unavailable");
            projection.registerCallback(revoked, main);
            capture = new PcmCapture(projection, intent.getBooleanExtra("fixture", false));
            evidence = intent.getBooleanExtra("evidence", false); evidenceLines = 0;
            evidenceFile = new File(getFilesDir(), "pcm-" + capture.generation + "-" + System.currentTimeMillis() + ".jsonl");
            main.post(telemetry);
        } catch (Exception e) {
            try { latest = new JSONObject().put("state", "failed").put("error", e.toString()).toString(); } catch (Exception json) { latest = "{\"state\":\"failed\"}"; }
            Log.e("PresetPcm", "Continuous playback capture failed", e); stopSelf();
        }
        return START_NOT_STICKY;
    }

    @Override public void onDestroy() {
        main.removeCallbacks(telemetry);
        if (capture != null) { capture.close(); capture = null; }
        if (projection != null) { projection.unregisterCallback(revoked); projection.stop(); projection = null; }
        stopForeground(STOP_FOREGROUND_REMOVE);
        if (!latest.contains("failed") && !latest.contains("revoked")) latest = "{\"state\":\"stopped\"}";
        super.onDestroy();
    }
    @Override public IBinder onBind(Intent intent) { return null; }
}
