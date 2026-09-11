package dev.gpui.box;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.graphics.Insets;
import android.graphics.Matrix;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.Choreographer;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.WindowManager;
import android.view.accessibility.AccessibilityManager;
import android.view.inputmethod.CursorAnchorInfo;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.window.OnBackInvokedCallback;
import android.window.OnBackInvokedDispatcher;
import java.util.List;

/** Android UI-thread host. Requires API 33+, one Activity and one GPUI window.
 * Subclasses load their cdylib and implement createNative to call initialize.
 * The manifest must preserve the Activity across the documented configuration
 * changes. Process death creates a fresh caller-owned application model. */
public abstract class GpuiActivity extends Activity implements SurfaceHolder.Callback, Choreographer.FrameCallback {
    private final Handler ui = new Handler(Looper.getMainLooper());
    private Content content;
    private boolean alive, resumed, surfaceReady, backEnabled, backRegistered;
    int surfaceDetachCount;
    boolean isNativeSurfaceReady() { return surfaceReady; }
    private AccessibilityManager accessibility;
    private final android.os.PowerManager.OnThermalStatusChangedListener thermalListener = status -> { if (alive) nativeSettingsChanged(true); };
    private final AccessibilityManager.AccessibilityStateChangeListener accessibilityListener = enabled -> {
        if (alive && surfaceReady) nativeAccessibility(enabled);
    };
    private final OnBackInvokedCallback backCallback = () -> { if (!nativeBack()) finish(); };

    protected abstract void createNative();

    @Override public void onCreate(Bundle saved) {
        super.onCreate(saved);
        getWindow().setDecorFitsSystemWindows(false);
        // The viewport remains full size. Insets report actual occlusion instead
        // of counting an IME both as a resize and as a bottom inset.
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING);
        content = new Content();
        setContentView(content);
        createNative();
        alive = true;
        accessibility = getSystemService(AccessibilityManager.class);
        accessibility.addAccessibilityStateChangeListener(accessibilityListener);
        getSystemService(android.os.PowerManager.class).addThermalStatusListener(getMainExecutor(), thermalListener);
    }
    @Override protected void onStart() { super.onStart(); if (alive) nativeLifecycle(0); }
    @Override protected void onResume() {
        super.onResume(); resumed = true;
        if (alive) nativeLifecycle(1);
        scheduleFrame();
    }
    @Override protected void onPause() {
        resumed = false; Choreographer.getInstance().removeFrameCallback(this);
        if (alive) nativeLifecycle(2);
        super.onPause();
    }
    @Override protected void onStop() { if (alive) nativeLifecycle(3); super.onStop(); }
    @Override protected void onDestroy() {
        Choreographer.getInstance().removeFrameCallback(this);
        if (accessibility != null) accessibility.removeAccessibilityStateChangeListener(accessibilityListener);
        getSystemService(android.os.PowerManager.class).removeThermalStatusListener(thermalListener);
        if (Build.VERSION.SDK_INT >= 33 && backRegistered) getOnBackInvokedDispatcher().unregisterOnBackInvokedCallback(backCallback);
        ui.removeCallbacksAndMessages(null);
        if (alive) { nativeDestroy(); alive = false; }
        super.onDestroy();
    }
    @Override public void onTrimMemory(int level) { super.onTrimMemory(level); if (alive) nativeMemory(); }
    @Override public void onLowMemory() { super.onLowMemory(); if (alive) nativeMemory(); }
    @Override public void onConfigurationChanged(android.content.res.Configuration configuration) {
        super.onConfigurationChanged(configuration);
        if (alive) nativeSettingsChanged(false);
        content.requestApplyInsets();
    }
    public int uiMode() { return getResources().getConfiguration().uiMode; }
    public int thermalStatus() { return getSystemService(android.os.PowerManager.class).getCurrentThermalStatus(); }
    @Override public void onWindowFocusChanged(boolean focus) {
        super.onWindowFocusChanged(focus);
        if (alive && resumed) nativeLifecycle(focus ? 1 : 2);
    }
    @Override public void surfaceCreated(SurfaceHolder holder) { /* surfaceChanged supplies dimensions. */ }
    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        if (!alive || width == 0 || height == 0) return;
        nativeSurface(holder.getSurface(), width, height, getResources().getDisplayMetrics().density);
        surfaceReady = true;
        content.requestApplyInsets();
        nativeAccessibility(accessibility.isEnabled());
        scheduleFrame();
    }
    @Override public void surfaceDestroyed(SurfaceHolder holder) {
        surfaceReady = false;
        surfaceDetachCount++;
        Choreographer.getInstance().removeFrameCallback(this);
        if (alive) nativeDetach(); // Must finish GPU detachment before returning.
    }
    private void scheduleFrame() {
        Choreographer.getInstance().removeFrameCallback(this);
        if (alive && resumed && surfaceReady) Choreographer.getInstance().postFrameCallback(this);
    }
    @Override public void doFrame(long nanos) { if (alive && resumed && surfaceReady) nativeFrame(); scheduleFrame(); }

    public void wake() { ui.post(() -> { if (alive) nativeDrain(); }); }
    public void backgroundTask() { if (!moveTaskToBack(true)) throw new IllegalStateException("Android refused moving the task to background"); }
    public void focusContent() { content.requestFocus(); }
    public void setTaskTitle(String title) { setTitle(title); }
    public void setPointerStyle(String style) {
        int type;
        switch (style) {
            case "IBeam": type = android.view.PointerIcon.TYPE_TEXT; break;
            case "IBeamCursorForVerticalLayout": type = android.view.PointerIcon.TYPE_VERTICAL_TEXT; break;
            case "PointingHand": type = android.view.PointerIcon.TYPE_HAND; break;
            case "OpenHand": type = android.view.PointerIcon.TYPE_GRAB; break;
            case "ClosedHand": type = android.view.PointerIcon.TYPE_GRABBING; break;
            case "Crosshair": type = android.view.PointerIcon.TYPE_CROSSHAIR; break;
            case "OperationNotAllowed": type = android.view.PointerIcon.TYPE_NO_DROP; break;
            case "DragCopy": type = android.view.PointerIcon.TYPE_COPY; break;
            case "DragLink": type = android.view.PointerIcon.TYPE_ALIAS; break;
            case "ContextualMenu": type = android.view.PointerIcon.TYPE_CONTEXT_MENU; break;
            case "ResizeLeft": case "ResizeRight": case "ResizeLeftRight": case "ResizeColumn": type = android.view.PointerIcon.TYPE_HORIZONTAL_DOUBLE_ARROW; break;
            case "ResizeUp": case "ResizeDown": case "ResizeUpDown": case "ResizeRow": type = android.view.PointerIcon.TYPE_VERTICAL_DOUBLE_ARROW; break;
            case "ResizeUpLeftDownRight": type = android.view.PointerIcon.TYPE_TOP_LEFT_DIAGONAL_DOUBLE_ARROW; break;
            case "ResizeUpRightDownLeft": type = android.view.PointerIcon.TYPE_TOP_RIGHT_DIAGONAL_DOUBLE_ARROW; break;
            default: type = android.view.PointerIcon.TYPE_ARROW;
        }
        content.setPointerIcon(android.view.PointerIcon.getSystemIcon(this, type));
    }
    public void openUrl(String url) { startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url))); }
    public String readClipboard() {
        ClipboardManager manager = getSystemService(ClipboardManager.class);
        ClipData data = manager.getPrimaryClip();
        return data == null || data.getItemCount() == 0 ? null : data.getItemAt(0).coerceToText(this).toString();
    }
    public void writeClipboard(String text) { getSystemService(ClipboardManager.class).setPrimaryClip(ClipData.newPlainText("", text)); }
    public void showKeyboard() { ui.post(() -> { if (alive) { content.requestFocus(); getSystemService(InputMethodManager.class).showSoftInput(content, InputMethodManager.SHOW_IMPLICIT); } }); }
    public void hideKeyboard() { getSystemService(InputMethodManager.class).hideSoftInputFromWindow(content.getWindowToken(), 0); }
    public void setBackEnabled(boolean enabled) {
        backEnabled = enabled;
        if (Build.VERSION.SDK_INT >= 33 && enabled != backRegistered) {
            if (enabled) getOnBackInvokedDispatcher().registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT, backCallback);
            else getOnBackInvokedDispatcher().unregisterOnBackInvokedCallback(backCallback);
            backRegistered = enabled;
        }
    }
    @SuppressWarnings("deprecation") @Override public void onBackPressed() {
        if (!backEnabled || !nativeBack()) super.onBackPressed();
    }
    public void inputChanged(int kind) {
        ui.post(() -> {
            if (!alive) return;
            if (kind == 1) { content.editable = false; hideKeyboard(); }
            if (kind == 0) content.editable = true;
            InputMethodManager manager = getSystemService(InputMethodManager.class);
            if (kind == 0 || kind == 1 || kind == 3 || kind == 4) manager.restartInput(content);
            GpuiInputConnection.State state = GpuiInputConnection.readState(this);
            if (state != null) manager.updateSelection(content, state.start, state.end, state.markedStart, state.markedEnd);
        });
    }
    public void updateCaret(float x, float y, float width, float height) {
        // Runs without a callback into GPUI, so safe during GPUI prepaint.
        int[] origin = new int[2]; content.getLocationOnScreen(origin);
        Matrix matrix = new Matrix(); matrix.setTranslate(origin[0], origin[1]);
        CursorAnchorInfo info = new CursorAnchorInfo.Builder().setMatrix(matrix)
            .setInsertionMarkerLocation(x, y, y + height, y + height, CursorAnchorInfo.FLAG_HAS_VISIBLE_REGION).build();
        getSystemService(InputMethodManager.class).updateCursorAnchorInfo(content, info);
    }
    public void updateAccessibility(String json) { content.provider.update(json); }

    private final class Content extends SurfaceView {
        boolean editable;
        final GpuiAccessibility provider = new GpuiAccessibility(this, GpuiActivity.this);
        Content() {
            super(GpuiActivity.this);
            getHolder().addCallback(GpuiActivity.this);
            setFocusable(true); setFocusableInTouchMode(true);
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
            setOnApplyWindowInsetsListener((v, insets) -> { reportInsets(insets); return insets; });
            setWindowInsetsAnimationCallback(new WindowInsetsAnimation.Callback(WindowInsetsAnimation.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE) {
                @Override public WindowInsets onProgress(WindowInsets insets, List<WindowInsetsAnimation> animations) { reportInsets(insets); return insets; }
            });
        }
        private void reportInsets(WindowInsets insets) {
            if (!alive || !surfaceReady) return;
            Insets safe = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            Insets ime = insets.getInsets(WindowInsets.Type.ime());
            nativeInsets(new int[]{safe.left, safe.top, safe.right, safe.bottom, ime.left, ime.top, ime.right, ime.bottom});
        }
        @Override public boolean onTouchEvent(MotionEvent event) {
            if (!alive || !surfaceReady) return false;
            int count = event.getPointerCount();
            int[] ids = new int[count]; float[] xs = new float[count], ys = new float[count], pressures = new float[count];
            for (int i = 0; i < count; i++) { ids[i] = event.getPointerId(i); xs[i] = event.getX(i); ys[i] = event.getY(i); pressures[i] = event.getPressure(i); }
            nativeTouch(event.getAction(), ids, xs, ys, pressures);
            return true;
        }
        @Override public boolean dispatchHoverEvent(MotionEvent event) { return provider.hover(event) || super.dispatchHoverEvent(event); }
        @Override public android.view.accessibility.AccessibilityNodeProvider getAccessibilityNodeProvider() { return provider; }
        @Override public boolean onCheckIsTextEditor() { return editable; }
        @Override public InputConnection onCreateInputConnection(EditorInfo out) {
            if (!editable) return null;
            GpuiInputConnection.State state = GpuiInputConnection.readState(GpuiActivity.this);
            if (state == null) return null;
            out.inputType = android.text.InputType.TYPE_CLASS_TEXT;
            switch (state.purpose) {
                case "Number": out.inputType = android.text.InputType.TYPE_CLASS_NUMBER; break;
                case "Decimal": out.inputType = android.text.InputType.TYPE_CLASS_NUMBER | android.text.InputType.TYPE_NUMBER_FLAG_DECIMAL; break;
                case "Phone": out.inputType = android.text.InputType.TYPE_CLASS_PHONE; break;
                case "Email": out.inputType |= android.text.InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS; break;
                case "Url": out.inputType |= android.text.InputType.TYPE_TEXT_VARIATION_URI; break;
            }
            if (state.multiline && (out.inputType & android.text.InputType.TYPE_MASK_CLASS) == android.text.InputType.TYPE_CLASS_TEXT) out.inputType |= android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE;
            out.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI;
            switch (state.action) {
                case "Go": out.imeOptions |= EditorInfo.IME_ACTION_GO; break;
                case "Search": out.imeOptions |= EditorInfo.IME_ACTION_SEARCH; break;
                case "Send": out.imeOptions |= EditorInfo.IME_ACTION_SEND; break;
                case "Next": out.imeOptions |= EditorInfo.IME_ACTION_NEXT; break;
                case "Previous": out.imeOptions |= EditorInfo.IME_ACTION_PREVIOUS; break;
                case "Done": out.imeOptions |= EditorInfo.IME_ACTION_DONE; break;
                default: out.imeOptions |= EditorInfo.IME_ACTION_NONE;
            }
            if (state.secure) {
                out.inputType = android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD;
                out.imeOptions |= EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
            }
            out.initialSelStart = state.start; out.initialSelEnd = state.end;
            if (!state.secure) out.setInitialSurroundingText(state.text);
            return new GpuiInputConnection(this, GpuiActivity.this, state.generation);
        }
    }

    private native void nativeSurface(Surface surface, int width, int height, float scale);
    private native void nativeDetach();
    private native void nativeFrame();
    private native void nativeDrain();
    private native void nativeLifecycle(int phase);
    private native void nativeMemory();
    private native void nativeDestroy();
    private native void nativeTouch(int action, int[] ids, float[] xs, float[] ys, float[] pressures);
    private native void nativeInsets(int[] values);
    private native boolean nativeBack();
    native String nativeInputSnapshot(long generation);
    native boolean nativeEdit(int command, String text, int start, int end, int cursor, long generation);
    private native void nativeSettingsChanged(boolean thermal);
    native void nativeAccessibility(boolean enabled);
    native long nativePresentedFrames();
    native boolean nativeAccessibilityAction(int id, int action, String value);
}
