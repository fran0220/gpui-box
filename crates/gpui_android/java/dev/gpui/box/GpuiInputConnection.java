package dev.gpui.box;

import android.view.View;
import android.view.KeyEvent;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;
import org.json.JSONObject;
import org.json.JSONException;

/** UTF-16 InputConnection backed by the current GPUI InputHandler. No second
 * editable document lives in Java. Unsupported rich-content operations retain
 * BaseInputConnection's false return rather than reporting accepted content. */
final class GpuiInputConnection extends BaseInputConnection {
    private final GpuiActivity activity;
    private final long generation;
    private boolean closed;
    static final class State {
        final String text;
        final String purpose, action;
        final boolean secure, multiline;
        final long generation;
        final int start, end, markedStart, markedEnd;
        State(String json) throws JSONException {
            JSONObject state = new JSONObject(json);
            text = state.getString("text"); start = state.getInt("start"); end = state.getInt("end");
            markedStart = state.getInt("markedStart"); markedEnd = state.getInt("markedEnd");
            generation = state.getLong("generation"); purpose = state.getString("purpose"); action = state.getString("action");
            secure = state.getBoolean("secure"); multiline = state.getBoolean("multiline");
        }
    }
    static State readState(GpuiActivity activity) {
        return readState(activity, 0);
    }
    private static State readState(GpuiActivity activity, long generation) {
        String json = activity.nativeInputSnapshot(generation);
        if (json == null) return null;
        try { return new State(json); } catch (JSONException error) { throw new IllegalStateException("Invalid GPUI input state", error); }
    }
    GpuiInputConnection(View view, GpuiActivity activity, long generation) { super(view, true); this.activity = activity; this.generation = generation; }
    private State state() { return closed ? null : readState(activity, generation); }
    private boolean edit(int command, String text, int start, int end, int cursor) {
        return !closed && activity.nativeEdit(command, text, start, end, cursor, generation);
    }
    @Override public void closeConnection() { finishComposingText(); closed = true; }
    @Override public CharSequence getTextBeforeCursor(int n, int flags) {
        State state = state(); if (state == null) return null;
        int left = Math.min(state.start, state.end);
        return state.text.substring(Math.max(0, left - Math.max(0, n)), left);
    }
    @Override public CharSequence getTextAfterCursor(int n, int flags) {
        State state = state(); if (state == null) return null;
        int right = Math.max(state.start, state.end);
        return state.text.substring(right, (int)Math.min(state.text.length(), (long)right + Math.max(0, n)));
    }
    @Override public CharSequence getSelectedText(int flags) { State state = state(); return state == null ? null : state.text.substring(Math.min(state.start, state.end), Math.max(state.start, state.end)); }
    @Override public int getCursorCapsMode(int reqModes) { State state = state(); return state == null ? 0 : android.text.TextUtils.getCapsMode(state.text, Math.min(state.start, state.end), reqModes); }
    @Override public ExtractedText getExtractedText(ExtractedTextRequest request, int flags) {
        // Monitoring requires unsolicited full-text updates; it is not claimed.
        if ((flags & GET_EXTRACTED_TEXT_MONITOR) != 0) return null;
        State state = state(); if (state == null) return null;
        ExtractedText result = new ExtractedText(); result.text = state.text; result.selectionStart = state.start; result.selectionEnd = state.end;
        result.partialStartOffset = result.partialEndOffset = -1; return result;
    }
    @Override public boolean commitText(CharSequence text, int cursor) { return edit(0, text.toString(), 0, 0, cursor); }
    @Override public boolean setComposingText(CharSequence text, int cursor) { return edit(1, text.toString(), 0, 0, cursor); }
    @Override public boolean setSelection(int start, int end) { return edit(2, null, start, end, 0); }
    @Override public boolean finishComposingText() { return edit(3, null, 0, 0, 0); }
    @Override public boolean setComposingRegion(int start, int end) { return edit(4, null, start, end, 0); }
    @Override public boolean performEditorAction(int action) { return edit(6, null, action, 0, 0); }
    @Override public boolean deleteSurroundingText(int before, int after) { return delete(before, after, false); }
    @Override public boolean deleteSurroundingTextInCodePoints(int before, int after) { return delete(before, after, true); }
    private boolean delete(int before, int after, boolean codePoints) {
        State state = state();
        if (state == null || before < 0 || after < 0) return false;
        int start = Math.min(state.start, state.end), end = Math.max(state.start, state.end);
        if (state.markedStart >= 0) start = Math.min(start, state.markedStart);
        if (state.markedEnd >= 0) end = Math.max(end, state.markedEnd);
        int left, right;
        if (codePoints) {
            left = state.text.offsetByCodePoints(start, -Math.min(before, state.text.codePointCount(0, start)));
            right = state.text.offsetByCodePoints(end, Math.min(after, state.text.codePointCount(end, state.text.length())));
        } else {
            left = Math.max(0, start - before); right = (int)Math.min(state.text.length(), (long)end + after);
            // Never send a split surrogate to Rust's UTF-8 document.
            if (left > 0 && left < state.text.length() && Character.isLowSurrogate(state.text.charAt(left))) left--;
            if (right > 0 && right < state.text.length() && Character.isLowSurrogate(state.text.charAt(right))) right++;
        }
        if (right > end && !edit(5, null, end, right, 0)) return false;
        return left == start || edit(5, null, left, start, 0);
    }
    @Override public boolean sendKeyEvent(KeyEvent event) {
        if (closed) return false;
        if (event.getAction() != KeyEvent.ACTION_DOWN) return event.getAction() == KeyEvent.ACTION_UP;
        if (event.getKeyCode() == KeyEvent.KEYCODE_DEL) {
            State state = state(); if (state == null) return false;
            return state.start != state.end ? edit(5, null, state.start, state.end, 0) : deleteSurroundingTextInCodePoints(1, 0);
        }
        if (event.getKeyCode() == KeyEvent.KEYCODE_ENTER) return commitText("\n", 1);
        int codePoint = event.getUnicodeChar();
        return Character.isValidCodePoint(codePoint) && codePoint != 0 && commitText(new String(Character.toChars(codePoint)), 1);
    }
    @Override public boolean requestCursorUpdates(int mode) { return !closed && (mode == 0 || mode == CURSOR_UPDATE_MONITOR); }
    @Override public boolean beginBatchEdit() { return false; }
    @Override public boolean endBatchEdit() { return false; }
}
