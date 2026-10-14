using System.IO;
using Microsoft.Win32;

namespace WindowsPaste.Services;

/// <summary>Persists small user preferences to the registry and applies run-at-login.</summary>
public sealed class AppSettings
{
    private const string RegistryPath = @"Software\WindowsPaste";
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string RunValueName = "WindowsPaste";

    public bool StartWithWindows { get; set; }

    public int HistoryCapacity { get; set; } = 500;

    public string HotkeyDisplay { get; set; } = "Ctrl + Shift + V";

    public uint HotkeyModifiers { get; set; } =
        Interop.NativeMethods.MOD_CONTROL | Interop.NativeMethods.MOD_SHIFT;

    public uint HotkeyVirtualKey { get; set; } = Interop.NativeMethods.VK_V;

    public HotkeyDefinition ToHotkeyDefinition()
    {
        var modifiers = HotkeyModifiers | Interop.NativeMethods.MOD_NOREPEAT;
        return new HotkeyDefinition(modifiers, HotkeyVirtualKey, HotkeyDisplay);
    }

    public static AppSettings Load()
    {
        var settings = new AppSettings();

        using var key = Registry.CurrentUser.OpenSubKey(RegistryPath);
        if (key is not null)
        {
            settings.HistoryCapacity = ReadInt(key, nameof(HistoryCapacity), settings.HistoryCapacity);
            settings.HotkeyDisplay = key.GetValue(nameof(HotkeyDisplay)) as string ?? settings.HotkeyDisplay;
            settings.HotkeyModifiers = ReadUInt(key, nameof(HotkeyModifiers), settings.HotkeyModifiers);
            settings.HotkeyVirtualKey = ReadUInt(key, nameof(HotkeyVirtualKey), settings.HotkeyVirtualKey);
        }

        settings.StartWithWindows = IsRunAtLoginEnabled();
        return settings;
    }

    public void Save()
    {
        using var key = Registry.CurrentUser.CreateSubKey(RegistryPath);
        key.SetValue(nameof(HistoryCapacity), HistoryCapacity, RegistryValueKind.DWord);
        key.SetValue(nameof(HotkeyDisplay), HotkeyDisplay, RegistryValueKind.String);
        key.SetValue(nameof(HotkeyModifiers), (int)HotkeyModifiers, RegistryValueKind.DWord);
        key.SetValue(nameof(HotkeyVirtualKey), (int)HotkeyVirtualKey, RegistryValueKind.DWord);
    }

    public void ApplyRunAtLogin(string executablePath)
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true)
                        ?? Registry.CurrentUser.CreateSubKey(RunKeyPath);

        if (StartWithWindows)
        {
            key.SetValue(RunValueName, $"\"{executablePath}\" --background", RegistryValueKind.String);
        }
        else if (key.GetValue(RunValueName) is not null)
        {
            key.DeleteValue(RunValueName, throwOnMissingValue: false);
        }
    }

    public static bool IsRunAtLoginEnabled()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath);
        return key?.GetValue(RunValueName) is not null;
    }

    public static string ExecutablePath()
    {
        var path = Environment.ProcessPath;
        if (!string.IsNullOrEmpty(path) && File.Exists(path))
        {
            return path;
        }

        return System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName ?? string.Empty;
    }

    private static int ReadInt(RegistryKey key, string name, int fallback)
    {
        return key.GetValue(name) is int value ? value : fallback;
    }

    private static uint ReadUInt(RegistryKey key, string name, uint fallback)
    {
        return key.GetValue(name) is int value ? (uint)value : fallback;
    }}
