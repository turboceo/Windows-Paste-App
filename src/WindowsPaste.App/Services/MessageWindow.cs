using System.Windows.Interop;

namespace WindowsPaste.Services;

/// <summary>Message payload surfaced by <see cref="MessageWindow"/>.</summary>
public sealed class WindowMessageEventArgs : EventArgs
{
    public WindowMessageEventArgs(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam)
    {
        Hwnd = hwnd;
        Msg = msg;
        WParam = wParam;
        LParam = lParam;
    }

    public IntPtr Hwnd { get; }
    public int Msg { get; }
    public IntPtr WParam { get; }
    public IntPtr LParam { get; }
    public bool Handled { get; set; }
}

/// <summary>
/// Owns a hidden message-only window and exposes its HWND for WM-based
/// features (clipboard listener, global hotkeys). One instance per application.
/// </summary>
public sealed class MessageWindow : IDisposable
{
    private const int HwndMessage = -3;
    private HwndSource? _source;
    private bool _disposed;

    public IntPtr Handle { get; private set; }

    public event EventHandler<WindowMessageEventArgs>? MessageReceived;

    public void EnsureCreated()
    {
        if (_source is not null)
        {
            return;
        }

        var parameters = new HwndSourceParameters("WindowsPaste.MessageWindow")
        {
            Width = 0,
            Height = 0,
            PositionX = 0,
            PositionY = 0,
            WindowStyle = 0,
            ParentWindow = new IntPtr(HwndMessage),
        };

        _source = new HwndSource(parameters);
        _source.AddHook(WndProc);
        Handle = _source.Handle;
    }

    private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        var args = new WindowMessageEventArgs(hwnd, msg, wParam, lParam);
        MessageReceived?.Invoke(this, args);
        handled = args.Handled;
        return IntPtr.Zero;
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        if (_source is not null)
        {
            _source.RemoveHook(WndProc);
            _source.Dispose();
            _source = null;
        }
    }
}
