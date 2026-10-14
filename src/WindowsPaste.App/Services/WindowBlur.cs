using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using WindowsPaste.Interop;

namespace WindowsPaste.Services;

/// <summary>
/// Applies a frosted (acrylic) backdrop to a window. WPF's AllowsTransparency
/// disables the compositor's own blur, so the accent policy is set directly on
/// the HWND after the window is shown.
/// </summary>
public static class WindowBlur
{
    /// <summary>
    /// Enables an acrylic blur behind the window. <paramref name="tintColor"/> is
    /// ARGB packed as 0xAABBGGRR, matching the Win32 convention.
    /// </summary>
    public static void ApplyAcrylic(Window window, int tintColor)
    {
        var handle = new WindowInteropHelper(window).Handle;
        if (handle == IntPtr.Zero)
        {
            return;
        }

        var accent = new NativeMethods.AccentPolicy
        {
            AccentState = NativeMethods.AccentState.AcrylicBlurBehind,
            AccentFlags = 2,
            GradientColor = tintColor,
        };

        Apply(handle, accent);

        // Rounded corners, matching the shelf's top-corner radius.
        var preference = NativeMethods.DWMWCP_ROUND;
        NativeMethods.DwmSetWindowAttribute(
            handle, NativeMethods.DWMWA_WINDOW_CORNER_PREFERENCE, ref preference, sizeof(int));
    }

    private static void Apply(IntPtr handle, NativeMethods.AccentPolicy accent)
    {
        var size = Marshal.SizeOf(accent);
        var pointer = Marshal.AllocHGlobal(size);

        try
        {
            Marshal.StructureToPtr(accent, pointer, false);

            var data = new NativeMethods.WindowCompositionAttributeData
            {
                Attribute = NativeMethods.WCA_ACCENT_POLICY,
                Data = pointer,
                SizeOfData = size,
            };

            NativeMethods.SetWindowCompositionAttribute(handle, ref data);
        }
        finally
        {
            Marshal.FreeHGlobal(pointer);
        }
    }
}
