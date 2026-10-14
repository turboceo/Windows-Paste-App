using WindowsPaste.Interop;

namespace WindowsPaste.Services;

/// <summary>
/// Listens for WM_CLIPBOARDUPDATE on the shared message window and raises
/// <see cref="ClipboardChanged"/> when the OS clipboard content changes.
/// </summary>
public sealed class ClipboardMonitor : IDisposable
{
    private readonly MessageWindow _messageWindow;
    private bool _registered;
    private bool _disposed;
    private bool _paused;

    public ClipboardMonitor(MessageWindow messageWindow)
    {
        _messageWindow = messageWindow;
        _messageWindow.MessageReceived += OnMessage;
    }

    public event EventHandler? ClipboardChanged;

    /// <summary>When true, updates are ignored (used while we write to the clipboard ourselves).</summary>
    public bool Paused
    {
        get => _paused;
        set => _paused = value;
    }

    public void Start()
    {
        if (_registered || _disposed)
        {
            return;
        }

        _messageWindow.EnsureCreated();
        _registered = NativeMethods.AddClipboardFormatListener(_messageWindow.Handle);
        if (!_registered)
        {
            throw new InvalidOperationException(
                $"AddClipboardFormatListener failed (error {System.Runtime.InteropServices.Marshal.GetLastWin32Error()}).");
        }
    }

    private void OnMessage(object? sender, WindowMessageEventArgs e)
    {
        if (e.Msg != NativeMethods.WM_CLIPBOARDUPDATE || _paused)
        {
            return;
        }

        e.Handled = true;
        ClipboardChanged?.Invoke(this, EventArgs.Empty);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _messageWindow.MessageReceived -= OnMessage;

        if (_registered)
        {
            NativeMethods.RemoveClipboardFormatListener(_messageWindow.Handle);
            _registered = false;
        }
    }
}
