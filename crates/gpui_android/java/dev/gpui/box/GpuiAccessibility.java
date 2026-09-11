package dev.gpui.box;

import android.graphics.Rect;
import android.os.Bundle;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Android virtual view tree for GPUI's AccessKit nodes. The Rust side assigns
 * stable ids and removes unreachable nodes before each update. */
final class GpuiAccessibility extends AccessibilityNodeProvider {
    private final View view;
    private final GpuiActivity activity;
    private final LinkedHashMap<Integer, JSONObject> nodes = new LinkedHashMap<>();
    private int root = -1, focused = -1, hovered = -1;
    GpuiAccessibility(View view, GpuiActivity activity) { this.view = view; this.activity = activity; }
    void update(String json) {
        try {
            JSONObject tree = new JSONObject(json);
            nodes.clear(); root = tree.optInt("root", -1);
            JSONArray values = tree.getJSONArray("nodes");
            for (int i = 0; i < values.length(); i++) { JSONObject node = values.getJSONObject(i); nodes.put(node.getInt("id"), node); }
            if (!nodes.containsKey(focused)) focused = -1;
            if (!nodes.containsKey(hovered)) hovered = -1;
            event(HOST_VIEW_ID, AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
        } catch (JSONException error) { throw new IllegalStateException("Invalid GPUI accessibility tree", error); }
    }
    private Rect bounds(JSONObject node) {
        JSONArray values = node.optJSONArray("bounds");
        return new Rect((int)Math.floor(values.optDouble(0)), (int)Math.floor(values.optDouble(1)), (int)Math.ceil(values.optDouble(2)), (int)Math.ceil(values.optDouble(3)));
    }
    private int parentOf(int id) {
        for (JSONObject parent : nodes.values()) {
            JSONArray children = parent.optJSONArray("children");
            for (int i = 0; i < children.length(); i++) if (children.optInt(i) == id) return parent.optInt("id");
        }
        return HOST_VIEW_ID;
    }
    @Override public AccessibilityNodeInfo createAccessibilityNodeInfo(int id) {
        if (id == HOST_VIEW_ID) {
            AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain(view);
            view.onInitializeAccessibilityNodeInfo(info);
            if (nodes.containsKey(root)) info.addChild(view, root);
            return info;
        }
        JSONObject node = nodes.get(id); if (node == null) return null;
        AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain();
        info.setSource(view, id); info.setPackageName(activity.getPackageName());
        int parent = parentOf(id);
        if (parent == HOST_VIEW_ID) info.setParent(view); else info.setParent(view, parent);
        String role = node.optString("role");
        info.setClassName(role.equals("Button") ? "android.widget.Button" : role.equals("TextInput") || role.equals("MultilineTextInput") ? "android.widget.EditText" : "android.view.View");
        info.setContentDescription(node.optString("label"));
        info.setPassword(node.optBoolean("password"));
        if (!node.optString("value").isEmpty()) info.setText(node.optString("value"));
        info.setEnabled(!node.optBoolean("disabled"));
        info.setVisibleToUser(view.isShown() && !node.optBoolean("hidden"));
        info.setFocusable(node.optBoolean("focusable")); info.setFocused(node.optBoolean("focused"));
        info.setAccessibilityFocused(id == focused);
        if (node.optBoolean("click")) { info.setClickable(true); info.addAction(AccessibilityNodeInfo.ACTION_CLICK); }
        if (node.optBoolean("focusable")) info.addAction(AccessibilityNodeInfo.ACTION_FOCUS);
        if (node.optBoolean("setValue")) { info.setEditable(true); info.addAction(AccessibilityNodeInfo.ACTION_SET_TEXT); }
        info.addAction(id == focused ? AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS : AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS);
        Rect rect = bounds(node);
        Rect relative = new Rect(rect);
        if (nodes.containsKey(parent)) { Rect parentBounds = bounds(nodes.get(parent)); relative.offset(-parentBounds.left, -parentBounds.top); }
        info.setBoundsInParent(relative);
        int[] origin = new int[2]; view.getLocationOnScreen(origin); rect.offset(origin[0], origin[1]); info.setBoundsInScreen(rect);
        JSONArray children = node.optJSONArray("children");
        for (int i = 0; i < children.length(); i++) if (nodes.containsKey(children.optInt(i))) info.addChild(view, children.optInt(i));
        return info;
    }
    @Override public boolean performAction(int id, int action, Bundle arguments) {
        if (!nodes.containsKey(id)) return false;
        if (action == AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS) {
            if (focused == id) return false;
            if (focused != -1) event(focused, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED);
            focused = id; event(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED); return true;
        }
        if (action == AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS) {
            if (focused != id) return false;
            focused = -1; event(id, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED); return true;
        }
        CharSequence value = arguments == null ? null : arguments.getCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE);
        return activity.nativeAccessibilityAction(id, action, value == null ? null : value.toString());
    }
    @Override public AccessibilityNodeInfo findFocus(int focus) {
        if (focus == AccessibilityNodeInfo.FOCUS_ACCESSIBILITY) return focused == -1 ? null : createAccessibilityNodeInfo(focused);
        for (JSONObject node : nodes.values()) if (node.optBoolean("focused")) return createAccessibilityNodeInfo(node.optInt("id"));
        return null;
    }
    @Override public List<AccessibilityNodeInfo> findAccessibilityNodeInfosByText(String text, int id) {
        ArrayList<AccessibilityNodeInfo> found = new ArrayList<>();
        search(id == HOST_VIEW_ID ? root : id, text.toLowerCase(Locale.ROOT), found); return found;
    }
    private void search(int id, String text, List<AccessibilityNodeInfo> found) {
        JSONObject node = nodes.get(id); if (node == null) return;
        if ((node.optString("label") + " " + node.optString("value")).toLowerCase(Locale.ROOT).contains(text)) found.add(createAccessibilityNodeInfo(id));
        JSONArray children = node.optJSONArray("children");
        for (int i = 0; i < children.length(); i++) search(children.optInt(i), text, found);
    }
    private void event(int id, int type) {
        if (!activity.getSystemService(AccessibilityManager.class).isEnabled() || view.getParent() == null) return;
        AccessibilityEvent event = AccessibilityEvent.obtain(type);
        event.setPackageName(activity.getPackageName()); event.setClassName("android.view.View"); event.setSource(view, id);
        if (nodes.containsKey(id)) event.setContentDescription(nodes.get(id).optString("label"));
        if (type == AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED) event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE);
        view.getParent().requestSendAccessibilityEvent(view, event);
    }
    boolean hover(MotionEvent event) {
        if (!activity.getSystemService(AccessibilityManager.class).isTouchExplorationEnabled()) return false;
        int id = event.getActionMasked() == MotionEvent.ACTION_HOVER_EXIT ? -1 : hit(root, (int)event.getX(), (int)event.getY());
        if (id != hovered) { if (id != -1) event(id, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER); if (hovered != -1) event(hovered, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT); hovered = id; }
        return id != -1;
    }
    private int hit(int id, int x, int y) {
        JSONObject node = nodes.get(id); if (node == null || node.optBoolean("hidden")) return -1;
        JSONArray children = node.optJSONArray("children");
        for (int i = children.length() - 1; i >= 0; i--) { int child = hit(children.optInt(i), x, y); if (child != -1) return child; }
        return bounds(node).contains(x, y) ? id : -1;
    }
}
