using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Windows.Media.Imaging;
using WindowsPaste.Interop;
using WindowsPaste.Models;

namespace WindowsPaste.Services;

/// <summary>
/// Reads the current clipboard content in a priority order and produces a
/// normalized, hashable snapshot. All Win32 access is funneled through here.
/// </summary>
public sealed class ClipboardSniffer
{
    private readonly string _blobsDirectory;

    public ClipboardSniffer(string blobsDirectory)
    {
        _blobsDirectory = blobsDirectory;
        Directory.CreateDirectory(_blobsDirectory);
    }

    /// <summary>
    /// Attempts to capture the clipboard. Returns null when nothing supported
    /// is available or the clipboard is locked by another process.
    /// </summary>
    public ClipboardItem? Capture()
    {
        // Order matters: files > image > text, matching user intent.
        var files = TryCaptureFiles();
        if (files is not null)
        {
            return files;
        }

        var image = TryCaptureImage();
        if (image is not null)
        {
            return image;
        }

        return TryCaptureText();
    }

    private static T? WithClipboard<T>(Func<T?> action) where T : class
    {
        // Retry a few times: another process may briefly hold the clipboard open.
        for (var attempt = 0; attempt < 8; attempt++)
        {
            if (NativeMethods.OpenClipboard(IntPtr.Zero) == IntPtr.Zero)
            {
                Thread.Sleep(25);
                continue;
            }

            try
            {
                return action();
            }
            finally
            {
                NativeMethods.CloseClipboard();
            }
        }

        return null;
    }

    private ClipboardItem? TryCaptureFiles()
    {
        if (!NativeMethods.IsClipboardFormatAvailable(NativeMethods.CF_HDROP))
        {
            return null;
        }

        var paths = WithClipboard(() =>
        {
            var handle = NativeMethods.GetClipboardData(NativeMethods.CF_HDROP);
            if (handle == IntPtr.Zero)
            {
                return null;
            }

            var count = NativeMethods.DragQueryFile(handle, 0xFFFFFFFF, null, 0);
            if (count == 0)
            {
                return null;
            }

            var list = new List<string>((int)count);
            for (uint i = 0; i < count; i++)
            {
                var length = NativeMethods.DragQueryFile(handle, i, null, 0);
                var buffer = new StringBuilder((int)length + 1);
                NativeMethods.DragQueryFile(handle, i, buffer, (uint)buffer.Capacity);
                list.Add(buffer.ToString());
            }

            return list;
        });

        if (paths is null || paths.Count == 0)
        {
            return null;
        }

        var content = string.Join(Environment.NewLine, paths);
        return new ClipboardItem
        {
            Type = ClipboardItemType.Files,
            Content = content,
            Hash = HashText("files:" + content),
            CreatedAt = DateTime.Now,
        };
    }

    private ClipboardItem? TryCaptureImage()
    {
        if (!NativeMethods.IsClipboardFormatAvailable(NativeMethods.CF_DIB) &&
            !NativeMethods.IsClipboardFormatAvailable(NativeMethods.CF_BITMAP))
        {
            return null;
        }

        // Read through WPF/System.Windows.Clipboard which handles DIB decode for us.
        // Wrapped defensively because another app may hold the clipboard.
        BitmapSource? source = null;
        try
        {
            if (System.Windows.Clipboard.ContainsImage())
            {
                source = System.Windows.Clipboard.GetImage();
            }
        }
        catch (System.Runtime.InteropServices.COMException)
        {
            return null;
        }

        if (source is null)
        {
            return null;
        }

        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(source));

        using var memory = new MemoryStream();
        encoder.Save(memory);
        var bytes = memory.ToArray();

        var hash = HashBytes(bytes);
        var fileName = hash + ".png";
        var fullPath = Path.Combine(_blobsDirectory, fileName);

        try
        {
            if (!File.Exists(fullPath))
            {
                File.WriteAllBytes(fullPath, bytes);
            }
        }
        catch (IOException)
        {
            return null;
        }

        var label = $"{source.PixelWidth} x {source.PixelHeight} image";
        return new ClipboardItem
        {
            Type = ClipboardItemType.Image,
            Content = label,
            BlobPath = fileName,
            Hash = hash,
            CreatedAt = DateTime.Now,
        };
    }

    private ClipboardItem? TryCaptureText()
    {
        if (!NativeMethods.IsClipboardFormatAvailable(NativeMethods.CF_UNICODETEXT))
        {
            return null;
        }

        var text = WithClipboard(() =>
        {
            var handle = NativeMethods.GetClipboardData(NativeMethods.CF_UNICODETEXT);
            if (handle == IntPtr.Zero)
            {
                return null;
            }

            var pointer = NativeMethods.GlobalLock(handle);
            if (pointer == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                return System.Runtime.InteropServices.Marshal.PtrToStringUni(pointer);
            }
            finally
            {
                NativeMethods.GlobalUnlock(handle);
            }
        });

        if (string.IsNullOrWhiteSpace(text))
        {
            return null;
        }

        return new ClipboardItem
        {
            Type = ClipboardItemType.Text,
            Content = text,
            Hash = HashText("text:" + text),
            CreatedAt = DateTime.Now,
        };
    }

    public string ResolveBlobPath(ClipboardItem item)
    {
        return item.BlobPath is null ? string.Empty : Path.Combine(_blobsDirectory, item.BlobPath);
    }

    private static string HashText(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        return HashBytes(bytes);
    }

    private static string HashBytes(byte[] bytes)
    {
        using var sha = SHA256.Create();
        var hash = sha.ComputeHash(bytes);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }
}
