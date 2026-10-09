/* UI composition/navigation adapted from johnneerdael/ProjectM-TV, LGPL-2.1.
 * Changed for Beatamp on 2026-10-09; see LICENSES/ProjectM-TV-UI-NOTICE.txt. */
package nl.neerdael.presetpoc;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.media.projection.MediaProjectionManager;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.KeyEvent;
import android.view.View;
import android.view.WindowManager;
import android.widget.FrameLayout;
import android.widget.TextView;
import org.json.JSONObject;
import java.util.Locale;
import java.util.concurrent.ThreadLocalRandom;

/** Android owns surfaces, navigation and consent; Rust owns rendering and analysis. */
public final class MainActivity extends Activity {
    private static final int PERMISSION = 1, PROJECTION = 2;
    private final Handler ui = new Handler(Looper.getMainLooper());
    private final CaptureRequest captureRequest = new CaptureRequest();
    private TextView state, audioFormat, audioDetail, renderDetail, sceneName, sceneMeta, overlayName;
    private View controls, overlay, diagnostics;
    private OptionRow captureRow, detailsRow;
    private boolean evidence, hideOverlays, panelOpen;
    private final TvNavigation navigation = new TvNavigation();
    private PcmTestPlayer player;
    private RenderView renderView;
    private final Runnable hideOverlay = () -> overlay.setVisibility(View.GONE);
    private final Runnable refresh = new Runnable() {
        @Override public void run() { showCapture(); ui.postDelayed(this, 500); }
    };

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        setContentView(R.layout.activity_main);
        state = findViewById(R.id.audio_state);
        audioFormat = findViewById(R.id.audio_format);
        audioDetail = findViewById(R.id.audio_detail);
        renderDetail = findViewById(R.id.render_detail);
        sceneName = findViewById(R.id.scene_name);
        sceneMeta = findViewById(R.id.scene_meta);
        overlayName = findViewById(R.id.scene_overlay_name);
        controls = findViewById(R.id.control_panel);
        overlay = findViewById(R.id.scene_overlay);
        diagnostics = findViewById(R.id.diagnostics);
        hideOverlays = getIntent().getBooleanExtra("hide_controls", false);
        evidence = getIntent().getBooleanExtra("evidence", false);
        renderView = new RenderView(this, this::showRenderer, getIntent().getIntExtra("rank", 1),
                getIntent().getBooleanExtra("render_fixture", false));
        FrameLayout root = findViewById(R.id.visualizer_root);
        root.addView(renderView, 0, new FrameLayout.LayoutParams(-1, -1));
        findViewById(R.id.touch_surface).setOnClickListener(v -> showPanel(!panelOpen));
        findViewById(R.id.previous_visual).setOnClickListener(v -> step(-1));
        findViewById(R.id.next_visual).setOnClickListener(v -> step(1));
        findViewById(R.id.shuffle_visual).setOnClickListener(v -> step(ThreadLocalRandom.current().nextInt(1, 100)));
        OptionRow mode = findViewById(R.id.row_mode);
        if (getIntent().getBooleanExtra("native_beat", false)) mode.setVisibility(View.GONE);
        mode.setup("Visual mode", new String[]{"Source", "Adapted"}, renderView.isAdapted() ? 1 : 0,
                false, index -> { if ((index == 1) != renderView.isAdapted()) renderView.toggleAdaptation(); });
        captureRow = findViewById(R.id.row_capture);
        captureRow.setupAction("Capture music", "Start", () -> requestCapture(false));
        ((OptionRow)findViewById(R.id.row_stop)).setupAction("Stop capture", "Stop", this::stopCapture);
        ((OptionRow)findViewById(R.id.row_fixture)).setupAction("Test playback PCM", "Start", () -> requestCapture(true));
        ((OptionRow)findViewById(R.id.row_reference)).setupAction("ProjectM reference", "Open", this::openReference);
        ((OptionRow)findViewById(R.id.row_licenses)).setupAction("Open-source UI", "License", this::showLicense);
        detailsRow = findViewById(R.id.row_details);
        detailsRow.setupAction("Diagnostics", "Show", () -> {
            boolean visible = diagnostics.getVisibility() != View.VISIBLE;
            diagnostics.setVisibility(visible ? View.VISIBLE : View.GONE);
            detailsRow.setActionValue(visible ? "Hide" : "Show");
        });
        if (!hideOverlays) showSceneOverlay();
        if (getIntent().getBooleanExtra("auto_start", false))
            ui.post(() -> requestCapture(getIntent().getBooleanExtra("fixture", false)));
    }

    private void showPanel(boolean open) {
        panelOpen = open;
        controls.animate().cancel();
        ui.removeCallbacks(hideOverlay);
        overlay.setVisibility(View.GONE);
        if (open) {
            controls.setVisibility(View.VISIBLE);
            controls.setAlpha(0f);
            controls.setTranslationX(24f * getResources().getDisplayMetrics().density);
            controls.animate().alpha(1f).translationX(0f).setDuration(160).start();
            findViewById(R.id.previous_visual).requestFocus();
        } else {
            controls.setVisibility(View.GONE);
            controls.clearFocus();
        }
    }

    private void showSceneOverlay() {
        if (hideOverlays || panelOpen) return;
        overlay.setVisibility(View.VISIBLE);
        ui.removeCallbacks(hideOverlay);
        ui.postDelayed(hideOverlay, 3500);
    }

    private void step(int delta) {
        renderView.step(delta);
        sceneName.setText("Loading visual " + renderView.currentRank() + "…");
        overlayName.setText(sceneName.getText());
        showSceneOverlay();
    }

    private void showRenderer(String raw) {
        try {
            JSONObject report = new JSONObject(raw);
            if (report.optString("state").equals("failed")) {
                sceneMeta.setText("Visual unavailable");
                renderDetail.setText(report.optString("error", "Renderer failed"));
                overlayName.setText("Visual unavailable");
                showSceneOverlay();
                return;
            }
            String name = report.optString("preset", report.optString("name", "Visual " + renderView.currentRank()));
            sceneName.setText(name);
            overlayName.setText(name);
            int width = report.optInt("width"), height = report.optInt("height");
            String size = width > 0 && height > 0 ? width + "×" + height : "Dimensions unreported";
            String engine = report.optString("engine").equals("beat") ? ".beat" : "Source diagnostic";
            sceneMeta.setText(String.format(Locale.US, "%03d / 100 · %s · %s", renderView.currentRank(), engine, size));
            renderDetail.setText(String.format(Locale.US,
                    "%s\n%s · %s\nFrame %,d · CPU %s ms\nDeadline misses %s\nInput: %s\nFeedback: %s",
                    engine, size, report.optString("mode", "unreported"), report.optLong("frame"),
                    CaptureText.optionalNumber(report.optDouble("cpu_frame_ms"), 1),
                    report.has("deadline_misses") && !report.isNull("deadline_misses")
                            ? String.format(Locale.US, "%,d", report.getLong("deadline_misses")) : "unmeasured",
                    report.optString("input", "unreported"), report.has("native_feedback") ?
                    report.optBoolean("native_feedback") ? "native" : "non-native" : "unreported"));
        } catch (Exception e) { renderDetail.setText("Renderer status unavailable: " + e); }
    }

    private void showLicense() {
        try (java.io.InputStream notice = getResources().openRawResource(R.raw.ui_notice);
             java.io.InputStream license = getResources().openRawResource(R.raw.ui_license)) {
            TextView text = new TextView(this);
            text.setText(readText(notice)
                    + "\n\n" + readText(license));
            text.setTextSize(13);
            int padding = Math.round(20 * getResources().getDisplayMetrics().density);
            text.setPadding(padding, padding, padding, padding);
            android.widget.ScrollView scroll = new android.widget.ScrollView(this);
            scroll.addView(text);
            new android.app.AlertDialog.Builder(this).setTitle("ProjectM-TV UI · LGPL 2.1")
                    .setView(scroll).setPositiveButton("Close", (dialog, which) -> {}).show();
        } catch (java.io.IOException e) { state.setText("License text unavailable: " + e); }
    }

    private static String readText(java.io.InputStream input) throws java.io.IOException {
        java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
        byte[] block = new byte[4096];
        int count;
        while ((count = input.read(block)) != -1) bytes.write(block, 0, count);
        return bytes.toString(java.nio.charset.StandardCharsets.UTF_8.name());
    }

    private void openReference() {
        try {
            startActivity(new Intent().setClassName("nl.neerdael.presetreference", "nl.neerdael.presetreference.ReferenceActivity")
                    .putExtra("rank", renderView.currentRank()));
        } catch (android.content.ActivityNotFoundException e) {
            state.setText("Install the ProjectM reference adapter to compare.");
        }
    }

    private void requestCapture(boolean fixture) {
        if (!captureRequest.begin(fixture)) return;
        renderView.setFixture(false);
        stopCaptureResources();
        PlaybackCaptureService.latest = "{\"state\":\"requesting-consent\"}";
        captureRow.setActionValue("Waiting");
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, PERMISSION);
            return;
        }
        requestProjection();
    }

    private void requestProjection() {
        try {
            startActivityForResult(getSystemService(MediaProjectionManager.class).createScreenCaptureIntent(), PROJECTION);
        } catch (RuntimeException e) { failCapture(e); }
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] grants) {
        super.onRequestPermissionsResult(request, permissions, grants);
        if (request != PERMISSION || !captureRequest.isPending()) return;
        if (grants.length == 1 && grants[0] == PackageManager.PERMISSION_GRANTED) requestProjection();
        else {
            captureRequest.finish();
            PlaybackCaptureService.latest = "{\"state\":\"permission-denied\"}";
            showCapture();
        }
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != PROJECTION || !captureRequest.isPending()) return;
        boolean fixture = captureRequest.fixture();
        captureRequest.finish();
        if (result != RESULT_OK || data == null) {
            PlaybackCaptureService.latest = "{\"state\":\"consent-denied\"}";
            showCapture();
            return;
        }
        try {
            if (fixture) player = new PcmTestPlayer();
            PlaybackCaptureService.latest = "{\"state\":\"waiting-for-pcm\"}";
            startForegroundService(new Intent(this, PlaybackCaptureService.class)
                    .putExtra("result_code", result).putExtra("result_data", data)
                    .putExtra("fixture", fixture).putExtra("evidence", evidence));
        } catch (Exception e) { failCapture(e); }
    }

    private void failCapture(Exception error) {
        captureRequest.finish();
        stopCaptureResources();
        PlaybackCaptureService.latest = "{\"state\":\"failed\",\"error\":" + JSONObject.quote(error.toString()) + "}";
        showCapture();
    }

    private void stopCaptureResources() {
        stopService(new Intent(this, PlaybackCaptureService.class));
        if (player != null) { player.close(); player = null; }
    }

    private void stopCapture() {
        captureRequest.finish();
        stopCaptureResources();
        PlaybackCaptureService.latest = "{\"state\":\"stopped\"}";
        showCapture();
    }

    private void showCapture() {
        try {
            JSONObject report = new JSONObject(PlaybackCaptureService.latest);
            String status = report.optString("state", "stopped");
            state.setText(CaptureText.status(status, report.optBoolean("fixture")));
            captureRow.setActionValue(captureRequest.isPending() ? "Waiting" : "Start");
            JSONObject worker = report.optJSONObject("worker");
            JSONObject snapshot = worker == null ? null : worker.optJSONObject("snapshot");
            if (snapshot == null) {
                audioFormat.setText(report.isNull("error") ? "Start capture while music plays in a compatible app." : report.optString("error"));
                audioDetail.setText("No continuous PCM analysis available.");
                return;
            }
            double age = (report.optLong("receipt_ns") - snapshot.optLong("capture_end_ns")) / 1_000_000.0;
            double cpu = worker.optLong("worker_cpu_ns") * 100.0 / Math.max(1, worker.optLong("worker_wall_ns"));
            audioFormat.setText(String.format(Locale.US, "%d Hz · %d channels · signed 16-bit PCM\n%s",
                    report.optInt("sample_rate"), report.optInt("channels"), report.optBoolean("fixture") ?
                    "Controlled test source" : "Source player capture policy applies"));
            audioDetail.setText(String.format(Locale.US,
                    "RMS %.4f · bass %.5f\nBass transient %.5f\nLeft %.4f · right %.4f\nBalance %s\nFeature age %.1f ms\nTimestamp: %s\nProcessed %,d · dropped %,d\nAnalysis mean %.1f µs · CPU %s\nGeneration %d · frames %,d\n%s",
                    snapshot.optDouble("rms"), snapshot.optDouble("bass_energy"), snapshot.optDouble("bass_flux"),
                    snapshot.optDouble("left_rms"), snapshot.optDouble("right_rms"), CaptureText.optionalNumber(snapshot.optDouble("balance"), 3), age,
                    snapshot.optBoolean("timestamp_estimated") ? "receipt estimate" : "AudioRecord sample clock", worker.optLong("processed_blocks"),
                    worker.optLong("dropped_blocks") + worker.optLong("queue_stale_blocks"), worker.optDouble("analysis_us_mean"),
                    worker.isNull("worker_cpu_ns") ? "unavailable" : CaptureText.optionalNumber(cpu, 2) + "%",
                    report.optLong("generation"), report.optLong("frames_read"), report.optBoolean("fixture") ?
                    "Test tones verify this capture route; they do not qualify music-player capture." : "Continuous playback capture"));
        } catch (Exception e) {
            state.setText("Capture status unavailable");
            audioDetail.setText(e.toString());
        }
    }

    @Override public boolean dispatchKeyEvent(KeyEvent event) {
        // Consume the matching release too: opening the panel must not click its new focus.
        if (event.getAction() == KeyEvent.ACTION_UP && navigation.keyUp(event.getKeyCode())) return true;
        if (event.getAction() == KeyEvent.ACTION_DOWN) {
            TvNavigation.Action action = navigation.keyDown(event.getKeyCode(), panelOpen, event.getRepeatCount() > 0);
            switch (action) {
                case CONSUME -> { return true; }
                case OPEN_PANEL -> { showPanel(true); return true; }
                case CLOSE_PANEL -> { showPanel(false); return true; }
                case PREVIOUS -> { step(-1); return true; }
                case NEXT -> { step(1); return true; }
                case SHOW_INFO -> { showSceneOverlay(); return true; }
                default -> { }
            }
        }
        return super.dispatchKeyEvent(event);
    }

    @Override public void onBackPressed() {
        if (panelOpen) showPanel(false);
        else super.onBackPressed();
    }

    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
    }

    @Override protected void onResume() {
        super.onResume(); ui.removeCallbacks(refresh); ui.post(refresh);
        if (renderView != null) renderView.start();
    }
    @Override protected void onPause() { ui.removeCallbacks(refresh); super.onPause(); }
    @Override protected void onStop() {
        if (captureRequest.stopOnBackground()) stopCaptureResources();
        if (renderView != null) renderView.stop();
        super.onStop();
    }
    @Override protected void onDestroy() {
        ui.removeCallbacksAndMessages(null);
        stopCaptureResources();
        super.onDestroy();
    }
}
