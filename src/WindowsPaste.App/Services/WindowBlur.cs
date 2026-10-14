using System.Windows;
using System.Windows.Interop;
using WindowsPaste.Interop;

namespace WindowsPaste.Services;

/// <summary>
/// Configures window frame attributes using standard Windows DWM APIs.
/// Unofficial composition APIs (such as SetWindowCompositionAttribute) have been
/// removed because they conflict with DirectX swap chains in WebView2 and trigger
/// GPU driver resets that affect other apps.
/// </summary>
public static class WindowBlur
{
    public static void ApplySquareCorners(Window window)
    {
        ApplySquareCorners(new WindowInteropHelper(window).Handle);
    }

    public static void ApplySquareCorners(IntPtr handle)
    {
        if (handle == IntPtr.Zero)
        {
            return;
        }

        // Standard Windows 11 DWM API to enforce sharp rectangular corners
        var preference = NativeMethods.DWMWCP_DONOTROUND;
        NativeMethods.DwmSetWindowAttribute(
            handle, NativeMethods.DWMWA_WINDOW_CORNER_PREFERENCE, ref preference, sizeof(int));
    }
}
