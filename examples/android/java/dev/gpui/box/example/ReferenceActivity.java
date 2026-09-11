package dev.gpui.box.example;

import android.content.SharedPreferences;

/** Reference-app checkpoint policy belongs to this example host, not GPUI.
 * Runs in a separate process so the protocol fixture keeps its own GPUI host. */
public final class ReferenceActivity extends dev.gpui.box.GpuiActivity {
    static { System.loadLibrary("gpui_android_demo"); }
    private SharedPreferences checkpoints;
    @Override protected void createNative() {
        checkpoints = getSharedPreferences("reference-checkpoint", MODE_PRIVATE);
        if (getIntent().getBooleanExtra("reset_checkpoint", false)
                && !checkpoints.edit().remove("checkpoint").commit()) {
            throw new IllegalStateException("Reference checkpoint reset failed");
        }
        nativeCreate(checkpoints.getString("checkpoint", null));
    }
    @Override protected void onStop() {
        // Snapshot on the UI thread before Activity suspension. A host that
        // stores product data must choose its own durability/security policy.
        String checkpoint = nativeCheckpoint();
        if (checkpoint != null && !checkpoints.edit().putString("checkpoint", checkpoint).commit()) {
            throw new IllegalStateException("Reference checkpoint save failed");
        }
        super.onStop();
    }
    private native void nativeCreate(String checkpoint);
    private native String nativeCheckpoint();
}
