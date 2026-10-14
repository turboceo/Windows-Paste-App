using System.Windows;
using System.Windows.Input;
using WindowsPaste.Interop;
using WindowsPaste.Services;

namespace WindowsPaste.Views;

public partial class SettingsWindow : Window
{
    private readonly AppSettings _settings;
    private HotkeyDefinition _hotkey = HotkeyDefinition.Default;
    private bool _capturingHotkey;

    public SettingsWindow(AppSettings settings)
    {
        InitializeComponent();
        _settings = settings;

        HotkeyBox.Text = settings.HotkeyDisplay;
        CapacitySlider.Value = settings.HistoryCapacity;
        StartupCheck.IsChecked = settings.StartWithWindows;
    }

    /// <summary>Set by the caller after Save so the app can re-register the hotkey.</summary>
    public HotkeyDefinition? ResultHotkey { get; private set; }

    private void OnHotkeyBoxFocus(object sender, KeyboardFocusChangedEventArgs e)
    {
        _capturingHotkey = true;
        HotkeyBox.Text = "Press a key combination...";
    }

    private void OnHotkeyBoxBlur(object sender, KeyboardFocusChangedEventArgs e)
    {
        _capturingHotkey = false;
        HotkeyBox.Text = _hotkey.Display;
    }

    private void OnHotkeyBoxKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (!_capturingHotkey)
        {
            return;
        }

        e.Handled = true;

        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        if (key is Key.LeftCtrl or Key.RightCtrl or Key.LeftShift or Key.RightShift
            or Key.LeftAlt or Key.RightAlt or Key.LWin or Key.RWin or Key.None)
        {
            return;
        }

        var modifiers = Keyboard.Modifiers;
        if (modifiers == ModifierKeys.None)
        {
            HotkeyBox.Text = "Include Ctrl, Alt, Shift, or Win";
            return;
        }

        var virtualKey = (uint)KeyInterop.VirtualKeyFromKey(key);
        var win32Modifiers = ToWin32Modifiers(modifiers) | NativeMethods.MOD_NOREPEAT;

        _hotkey = new HotkeyDefinition(win32Modifiers, virtualKey, FormatDisplay(modifiers, key));
        HotkeyBox.Text = _hotkey.Display;
        _capturingHotkey = false;
        Keyboard.ClearFocus();
    }

    private static uint ToWin32Modifiers(ModifierKeys modifiers)
    {
        uint result = 0;
        if (modifiers.HasFlag(ModifierKeys.Control)) result |= NativeMethods.MOD_CONTROL;
        if (modifiers.HasFlag(ModifierKeys.Alt)) result |= NativeMethods.MOD_ALT;
        if (modifiers.HasFlag(ModifierKeys.Shift)) result |= NativeMethods.MOD_SHIFT;
        if (modifiers.HasFlag(ModifierKeys.Windows)) result |= NativeMethods.MOD_WIN;
        return result;
    }

    private static string FormatDisplay(ModifierKeys modifiers, Key key)
    {
        var parts = new List<string>();
        if (modifiers.HasFlag(ModifierKeys.Control)) parts.Add("Ctrl");
        if (modifiers.HasFlag(ModifierKeys.Shift)) parts.Add("Shift");
        if (modifiers.HasFlag(ModifierKeys.Alt)) parts.Add("Alt");
        if (modifiers.HasFlag(ModifierKeys.Windows)) parts.Add("Win");
        parts.Add(key.ToString());
        return string.Join(" + ", parts);
    }

    private void OnSave(object sender, RoutedEventArgs e)
    {
        _settings.StartWithWindows = StartupCheck.IsChecked == true;
        _settings.HistoryCapacity = (int)CapacitySlider.Value;
        _settings.HotkeyDisplay = _hotkey.Display;
        _settings.HotkeyModifiers = _hotkey.Modifiers & ~NativeMethods.MOD_NOREPEAT;
        _settings.HotkeyVirtualKey = _hotkey.VirtualKey;
        _settings.Save();
        _settings.ApplyRunAtLogin(AppSettings.ExecutablePath());

        ResultHotkey = _hotkey;
        DialogResult = true;
    }

    private void OnCancel(object sender, RoutedEventArgs e) => DialogResult = false;
}
