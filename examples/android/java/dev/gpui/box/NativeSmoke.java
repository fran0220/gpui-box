package dev.gpui.box;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import java.io.FileOutputStream;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

/** Real-device instrumentation, not a host simulation. Packaged only in the
 * development fixture APK. Fails rather than treating unavailable IME/a11y as
 * empty data. UI operations run on the Activity's actual Android UI thread. */
public final class NativeSmoke extends Instrumentation {
    private GpuiActivity activity;
    private View content;
    private interface Check { boolean run() throws Exception; }
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }
    private void await(Check check, String failure) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) { waitForIdleSync(); if (check.run()) return; Thread.sleep(30); }
        throw new AssertionError(failure);
    }
    private <T> T onUi(java.util.concurrent.Callable<T> work) {
        AtomicReference<T> result = new AtomicReference<>(); AtomicReference<Throwable> failure = new AtomicReference<>();
        runOnMainSync(() -> { try { result.set(work.call()); } catch (Throwable error) { failure.set(error); } });
        if (failure.get() != null) throw new AssertionError("Android UI operation failed", failure.get());
        return result.get();
    }
    private AccessibilityNodeInfo find(String text) {
        return onUi(() -> {
            List<AccessibilityNodeInfo> nodes = content.getAccessibilityNodeProvider().findAccessibilityNodeInfosByText(text, -1);
            return nodes.isEmpty() ? null : nodes.get(0);
        });
    }
    private void tap(String label) throws Exception {
        await(() -> find(label) != null, "Missing accessibility node: " + label);
        Rect bounds = new Rect(); find(label).getBoundsInScreen(bounds);
        if (bounds.isEmpty()) throw new AssertionError("Empty native hit bounds: " + label);
        long time = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(time, time, MotionEvent.ACTION_DOWN, bounds.exactCenterX(), bounds.exactCenterY(), 0);
        MotionEvent up = MotionEvent.obtain(time, time + 30, MotionEvent.ACTION_UP, bounds.exactCenterX(), bounds.exactCenterY(), 0);
        try { sendPointerSync(down); sendPointerSync(up); } finally { down.recycle(); up.recycle(); }
    }
    private GpuiInputConnection.State state() { return onUi(() -> GpuiInputConnection.readState(activity)); }
    private void textEquals(String expected) throws Exception {
        await(() -> state() != null && expected.equals(state().text), "Authoritative input does not equal fixture expectation");
    }
    private void require(boolean value, String message) { if (!value) throw new AssertionError(message); }
    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            Intent intent = new Intent(getTargetContext(), dev.gpui.box.example.MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            activity = (GpuiActivity)startActivitySync(intent);
            content = onUi(() -> ((ViewGroup)activity.findViewById(android.R.id.content)).getChildAt(0));
            await(() -> onUi(() -> activity.isNativeSurfaceReady() && activity.nativePresentedFrames() > 0), "No native GPU presentation");
            onUi(() -> { activity.nativeAccessibility(true); return null; });
            tap("Increment");
            await(() -> find("Count: 1") != null, "Native touch did not increment exactly once");
            sendKeyDownUpSync(android.view.KeyEvent.KEYCODE_BACK);
            await(() -> find("Count: 0") != null, "System back did not reach the enabled GPUI handler");
            tap("Increment");
            tap("Native editable fixture");
            await(() -> state() != null, "Touch did not focus native editable input");
            InputConnection connection = onUi(() -> content.onCreateInputConnection(new EditorInfo()));
            require(connection != null, "No Android InputConnection");
            require(onUi(() -> connection.setSelection(4, 1)), "Reversed UTF16 selection refused");
            require(state().start == 4 && state().end == 1, "Anchor/head order was lost in native snapshot");
            require("😀中".contentEquals(onUi(() -> connection.getSelectedText(0))), "Reversed selected text was not normalized for slicing");
            require("A".contentEquals(onUi(() -> connection.getTextBeforeCursor(10, 0))), "Text before reversed selection is wrong");
            require("Z".contentEquals(onUi(() -> connection.getTextAfterCursor(10, 0))), "Text after reversed selection is wrong");
            require(onUi(() -> connection.setSelection(1, 3)), "UTF16 emoji selection refused");
            require(onUi(() -> connection.commitText("e\u0301", 1)), "Combining text commit refused");
            textEquals("Ae\u0301中Z");
            require(onUi(() -> connection.setSelection(1, 3)), "Combining selection refused");
            require(onUi(() -> connection.setComposingText("汉", 1)), "Composition refused");
            textEquals("A汉中Z");
            require(state().markedStart == 1 && state().markedEnd == 2, "Composition range not preserved");
            require(onUi(connection::finishComposingText), "Finish composition refused");
            require(state().markedStart == -1, "Composition not cleared");
            require(onUi(() -> connection.deleteSurroundingTextInCodePoints(1, 1)), "Code point deletion refused");
            textEquals("AZ");
            onUi(() -> { connection.closeConnection(); return null; });
            require(!onUi(() -> connection.commitText("stale", 1)), "Closed input connection was accepted");
            textEquals("AZ");

            // Exercise the real SurfaceHolder destroy/create path while keeping
            // the same Activity and GPUI application handle alive.
            int detachCount = onUi(() -> activity.surfaceDetachCount);
            long presented = onUi(activity::nativePresentedFrames);
            onUi(() -> { content.setVisibility(View.GONE); return null; });
            await(() -> onUi(() -> activity.surfaceDetachCount > detachCount && !activity.isNativeSurfaceReady()), "Native Surface was not detached");
            onUi(() -> { content.setVisibility(View.VISIBLE); return null; });
            await(() -> onUi(() -> activity.isNativeSurfaceReady() && activity.nativePresentedFrames() > presented), "No native presentation after Surface recreation");
            await(() -> find("Count: 1") != null, "State lost after Surface recreation");
            onUi(() -> { activity.setRequestedOrientation(ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE); return null; });
            await(() -> onUi(() -> content.getWidth() > content.getHeight()), "Landscape surface did not resize");
            require(find("Count: 1") != null, "Counter lost on orientation change");
            tap("Native editable fixture"); textEquals("AZ");

            Bitmap screenshot = getUiAutomation().takeScreenshot();
            require(screenshot != null, "No native screenshot captured");
            try (FileOutputStream output = getTargetContext().openFileOutput("native-smoke.png", 0)) { require(screenshot.compress(Bitmap.CompressFormat.PNG, 100, output), "Screenshot encode failed"); }
            screenshot.recycle();
            result.putString("stream", "PASS: native touch/back; AccessKit bounds; UTF16 commit/composition/deletion; closed input rejection; surface recreation; rotation. Screenshot: files/native-smoke.png\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            result.putString("stream", "FAIL: " + android.util.Log.getStackTraceString(error));
            finish(Activity.RESULT_CANCELED, result);
        }
    }
}
