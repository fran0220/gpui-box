using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace GpuiBox.Accessibility
{
    // The subset of the Windows SDK UIAutomationClient COM interfaces needed
    // to query a property newer than PowerShell's managed UIAutomationTypes.
    // IUnknown vtable order is significant, including methods not called here.
    [Flags]
    internal enum TreeScope
    {
        None = 0,
        Element = 1,
        Children = 2,
        Descendants = 4,
        Parent = 8,
        Ancestors = 16,
        Subtree = Element | Children | Descendants,
    }

    [ComImport]
    [Guid("FF48DBA4-60EF-4201-AA87-54103EEF594E")]
    [ClassInterface(ClassInterfaceType.None)]
    internal class CUIAutomation
    {
    }

    [ComImport]
    [Guid("352FFBA8-0973-437C-A61F-F64CAFD81DF9")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IUIAutomationCondition
    {
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct Point
    {
        internal int X;
        internal int Y;
    }

    [ComImport]
    [Guid("30CBE57D-D9D0-452A-AB13-7AC5AC4825EE")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IUIAutomation
    {
        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Bool)]
        bool CompareElements(IUIAutomationElement first, IUIAutomationElement second);

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Bool)]
        bool CompareRuntimeIds(
            [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_I4)] int[] first,
            [MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_I4)] int[] second
        );

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement GetRootElement();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement ElementFromHandle(IntPtr handle);

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement ElementFromPoint(Point point);

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement GetFocusedElement();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement GetRootElementBuildCache([MarshalAs(UnmanagedType.Interface)] object request);

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement ElementFromHandleBuildCache(
            IntPtr handle,
            [MarshalAs(UnmanagedType.Interface)] object request
        );

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement ElementFromPointBuildCache(
            Point point,
            [MarshalAs(UnmanagedType.Interface)] object request
        );

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement GetFocusedElementBuildCache([MarshalAs(UnmanagedType.Interface)] object request);

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Interface)]
        object CreateTreeWalker(IUIAutomationCondition condition);

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Interface)]
        object GetControlViewWalker();

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Interface)]
        object GetContentViewWalker();

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Interface)]
        object GetRawViewWalker();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition GetRawViewCondition();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition GetControlViewCondition();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition GetContentViewCondition();

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Interface)]
        object CreateCacheRequest();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition CreateTrueCondition();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition CreateFalseCondition();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition CreatePropertyCondition(
            int propertyId,
            [MarshalAs(UnmanagedType.Struct)] object value
        );

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition CreatePropertyConditionEx(
            int propertyId,
            [MarshalAs(UnmanagedType.Struct)] object value,
            int flags
        );

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationCondition CreateAndCondition(
            IUIAutomationCondition first,
            IUIAutomationCondition second
        );
    }

    [ComImport]
    [Guid("D22108AA-8AC5-49A5-837B-37BBB3D7591E")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IUIAutomationElement
    {
        [MethodImpl(MethodImplOptions.InternalCall)]
        void SetFocus();

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.SafeArray, SafeArraySubType = VarEnum.VT_I4)]
        int[] GetRuntimeId();

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement FindFirst(TreeScope scope, IUIAutomationCondition condition);

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Interface)]
        object FindAll(TreeScope scope, IUIAutomationCondition condition);

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement FindFirstBuildCache(
            TreeScope scope,
            IUIAutomationCondition condition,
            [MarshalAs(UnmanagedType.Interface)] object request
        );

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Interface)]
        object FindAllBuildCache(
            TreeScope scope,
            IUIAutomationCondition condition,
            [MarshalAs(UnmanagedType.Interface)] object request
        );

        [MethodImpl(MethodImplOptions.InternalCall)]
        IUIAutomationElement BuildUpdatedCache([MarshalAs(UnmanagedType.Interface)] object request);

        [MethodImpl(MethodImplOptions.InternalCall)]
        [return: MarshalAs(UnmanagedType.Struct)]
        object GetCurrentPropertyValue(int propertyId);
    }

    // Host keyboard focus for the target process. AccessKit reports
    // HasKeyboardFocus and answers GetFocusedElement only while the window
    // that owns the tree holds Win32 focus (WM_SETFOCUS). UIA's SetFocus
    // establishes that for the editable mode as a side effect, and the macOS
    // script sets `frontmost`; a check that only reads focus must establish
    // it explicitly. A process that was not started from the foreground may
    // not call SetForegroundWindow directly, so the caller attaches its input
    // queue to the current foreground thread and the target thread first.
    public static class NativeWindow
    {
        private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lparam);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lparam);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern IntPtr GetWindow(IntPtr hwnd, uint command);

        [DllImport("user32.dll")]
        private static extern IntPtr GetForegroundWindow();

        [DllImport("user32.dll")]
        private static extern bool SetForegroundWindow(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern bool BringWindowToTop(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern IntPtr SetFocus(IntPtr hwnd);

        [DllImport("user32.dll")]
        private static extern bool AttachThreadInput(uint attach, uint attachTo, bool attaching);

        [DllImport("kernel32.dll")]
        private static extern uint GetCurrentThreadId();

        private const uint GW_OWNER = 4;

        public static IntPtr TopLevelWindow(int processId)
        {
            IntPtr found = IntPtr.Zero;
            EnumWindows(
                delegate (IntPtr hwnd, IntPtr lparam)
                {
                    uint owner;
                    GetWindowThreadProcessId(hwnd, out owner);
                    if (owner != (uint)processId ||
                        !IsWindowVisible(hwnd) ||
                        GetWindow(hwnd, GW_OWNER) != IntPtr.Zero)
                    {
                        return true;
                    }
                    found = hwnd;
                    return false;
                },
                IntPtr.Zero
            );
            return found;
        }

        // Returns whether the target's top-level window holds the foreground
        // afterwards. Idempotent; safe to retry.
        public static bool Activate(int processId)
        {
            IntPtr hwnd = TopLevelWindow(processId);
            if (hwnd == IntPtr.Zero)
            {
                return false;
            }
            if (GetForegroundWindow() == hwnd)
            {
                return true;
            }

            uint current = GetCurrentThreadId();
            uint ignored;
            uint target = GetWindowThreadProcessId(hwnd, out ignored);
            IntPtr foreground = GetForegroundWindow();
            uint foregroundThread = foreground == IntPtr.Zero
                ? 0
                : GetWindowThreadProcessId(foreground, out ignored);

            bool attachedForeground = foregroundThread != 0 &&
                foregroundThread != current &&
                AttachThreadInput(current, foregroundThread, true);
            bool attachedTarget = target != current && AttachThreadInput(current, target, true);
            try
            {
                BringWindowToTop(hwnd);
                SetForegroundWindow(hwnd);
                SetFocus(hwnd);
            }
            finally
            {
                if (attachedTarget)
                {
                    AttachThreadInput(current, target, false);
                }
                if (attachedForeground)
                {
                    AttachThreadInput(current, foregroundThread, false);
                }
            }
            return GetForegroundWindow() == hwnd;
        }
    }

    public static class NativeUia
    {
        private const int ProcessIdProperty = 30002;
        private const int ControlTypeProperty = 30003;
        private const int NameProperty = 30005;
        private const int FullDescriptionProperty = 30159;

        public static string FullDescription(int processId, int controlTypeId, string name)
        {
            IUIAutomation automation = (IUIAutomation)new CUIAutomation();
            IUIAutomationCondition process = automation.CreatePropertyCondition(
                ProcessIdProperty,
                processId
            );
            IUIAutomationCondition controlType = automation.CreatePropertyCondition(
                ControlTypeProperty,
                controlTypeId
            );
            IUIAutomationCondition named = automation.CreatePropertyCondition(NameProperty, name);
            IUIAutomationCondition identity = automation.CreateAndCondition(process, controlType);
            IUIAutomationCondition condition = automation.CreateAndCondition(identity, named);
            IUIAutomationElement element = automation.GetRootElement().FindFirst(
                TreeScope.Descendants,
                condition
            );
            if (element == null)
            {
                throw new InvalidOperationException(
                    "Native UIA could not find control type " + controlTypeId + " named '" + name + "'."
                );
            }
            object value = element.GetCurrentPropertyValue(FullDescriptionProperty);
            return value == null ? String.Empty : Convert.ToString(value);
        }
    }
}
