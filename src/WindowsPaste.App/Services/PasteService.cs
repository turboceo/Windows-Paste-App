using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Media.Imaging;
using WindowsPaste.Interop;
using WindowsPaste.Models;

namespace WindowsPaste.Services;

/// <summary>
/// Writes a history item back to the clipboard and simulates Ctrl+V into the
/// previously focused window.
/// </summary>
public sealed class PasteService
{
    private readonly ClipboardSniffer _sniffer;
    private readonly ClipboardMonitor _monitor;

    public PasteService(ClipboardSniffer sniffer, ClipboardMonitor monitor)
    {
        _sniffer = sniffer;
        _monitor = monitor;
    }

    /// <summary>
    /// Places the item on the clipboard and pastes it. When
    /// <paramref name="autoPaste"/> is false only the clipboard is updated.
    /// </summary>
    public async Task PasteAsync(ClipboardItem item, IntPtr targetWindow, bool autoPaste = true)
    {
        _monitor.Paused = true;
        try
        {
            WriteToClipboard(item);
        }
        finally
        {
            // Give the OS a moment to settle, then re-enable listening.
            await Task.Delay(150).ConfigureAwait(true);
            _monitor.Paused = false;
        }

        if (!autoPaste)
        {
            return;
        }

        if (targetWindow != IntPtr.Zero)
        {
            NativeMethods.SetForegroundWindow(targetWindow);
        }

        // The popup must be gone before the keystroke lands on the target app.
        await Task.Delay(120).ConfigureAwait(true);
        SendCtrlV();
    }

    private void WriteToClipboard(ClipboardItem item)
    {
        switch (item.Type)
        {
            case ClipboardItemType.Image:
                WriteImage(item);
                break;
            case ClipboardItemType.Files:
                WriteText(string.Join(Environment.NewLine, ParsePaths(item.Content)));
                break;
            default:
                WriteText(item.Content);
                break;
        }
    }

    private static void WriteText(string text)
    {
        for (var attempt = 0; attempt < 8; attempt++)
        {
            try
            {
                System.Windows.Clipboard.SetText(text);
                return;
            }
            catch (COMException)
            {
                Thread.Sleep(25);
            }
        }
    }

    private void WriteImage(ClipboardItem item)
    {
        var path = _sniffer.ResolveBlobPath(item);
        if (string.IsNullOrEmpty(path) || !File.Exists(path))
        {
            return;
        }

        var image = new BitmapImage();
        image.BeginInit();
        image.CacheOption = BitmapCacheOption.OnLoad;
        image.UriSource = new Uri(path);
        image.EndInit();
        image.Freeze();

        for (var attempt = 0; attempt < 8; attempt++)
        {
            try
            {
                // Also expose the file drop list so apps that prefer paths can use it.
                var data = new System.Windows.DataObject();
                data.SetImage(image);
                data.SetData(System.Windows.DataFormats.FileDrop, new[] { path });
                System.Windows.Clipboard.SetDataObject(data, true);
                return;
            }
            catch (COMException)
            {
                Thread.Sleep(25);
            }
        }
    }

    private static IEnumerable<string> ParsePaths(string content)
    {
        return content
            .Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)
            .Select(line => line.Trim())
            .Where(line => line.Length > 0);
    }

    private static void SendCtrlV()
    {
        var inputs = new[]
        {
            KeyDown(NativeMethods.VK_CONTROL),
            KeyDown(NativeMethods.VK_V),
            KeyUp(NativeMethods.VK_V),
            KeyUp(NativeMethods.VK_CONTROL),
        };

        NativeMethods.SendInput((uint)inputs.Length, inputs, Marshal.SizeOf<NativeMethods.INPUT>());
    }

    private static NativeMethods.INPUT KeyDown(ushort vk) => new()
    {
        type = NativeMethods.INPUT_KEYBOARD,
        u = new NativeMethods.InputUnion
        {
            ki = new NativeMethods.KEYBDINPUT { wVk = vk, dwFlags = 0 },
        },
    };

    private static NativeMethods.INPUT KeyUp(ushort vk) => new()
    {
        type = NativeMethods.INPUT_KEYBOARD,
        u = new NativeMethods.InputUnion
        {
            ki = new NativeMethods.KEYBDINPUT { wVk = vk, dwFlags = NativeMethods.KEYEVENTF_KEYUP },
        },
    };
}
