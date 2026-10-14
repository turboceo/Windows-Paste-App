using System.Windows.Forms;
using WindowsPaste.Interop;

namespace WindowsPaste.Services;

public sealed class HotkeyDefinition
{
    public HotkeyDefinition(uint modifiers, uint virtualKey, string display)
    {
        Modifiers = modifiers;
        VirtualKey = virtualKey;
        Display = display;
    }

    public uint Modifiers { get; }
    public uint VirtualKey { get; }
    public string Display { get; }

    public static HotkeyDefinition Default => new(
        NativeMethods.MOD_CONTROL | NativeMethods.MOD_SHIFT | NativeMethods.MOD_NOREPEAT,
        NativeMethods.VK_V,
        "Ctrl + Shift + V");
}

/// <summary>
/// Registers a single system-wide hotkey on the shared message window and raises
/// <see cref="Pressed"/> on WM_HOTKEY.
/// </summary>
public sealed class HotkeyManager : IDisposable
{
    private const int HotkeyId = 0x5041; // "PA"

    private readonly MessageWindow _messageWindow;
    private HotkeyDefinition? _current;
    private bool _disposed;

    public HotkeyManager(MessageWindow messageWindow)
    {
        _messageWindow = messageWindow;
        _messageWindow.MessageReceived += OnMessage;
    }

    public event EventHandler? Pressed;

    public HotkeyDefinition? Current => _current;

    /// <summary>Registers or replaces the hotkey. Returns false when the OS rejects it (already taken).</summary>
    public bool Register(HotkeyDefinition definition)
    {
        if (_disposed)
        {
            return false;
        }

        _messageWindow.EnsureCreated();
        UnregisterInternal();

        if (!NativeMethods.RegisterHotKey(_messageWindow.Handle, HotkeyId, definition.Modifiers, definition.VirtualKey))
        {
            return false;
        }

        _current = definition;
        return true;
    }

    private void UnregisterInternal()
    {
        if (_current is null)
        {
            return;
        }

        NativeMethods.UnregisterHotKey(_messageWindow.Handle, HotkeyId);
        _current = null;
    }

    private void OnMessage(object? sender, WindowMessageEventArgs e)
    {
        if (e.Msg != NativeMethods.WM_HOTKEY || e.WParam.ToInt32() != HotkeyId)
        {
            return;
        }

        e.Handled = true;
        Pressed?.Invoke(this, EventArgs.Empty);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _messageWindow.MessageReceived -= OnMessage;
        UnregisterInternal();
    }
}
