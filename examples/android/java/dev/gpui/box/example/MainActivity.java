package dev.gpui.box.example;

public final class MainActivity extends dev.gpui.box.GpuiActivity {
    static { System.loadLibrary("gpui_android_demo"); }
    @Override protected void createNative() { nativeCreate(); }
    private native void nativeCreate();
}
