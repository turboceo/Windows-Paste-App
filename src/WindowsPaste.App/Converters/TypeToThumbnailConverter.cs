using System.Globalization;
using System.Windows;
using System.Windows.Data;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using WindowsPaste.Models;

namespace WindowsPaste.Converters;

/// <summary>
/// Resolves a preview thumbnail for image entries. The application supplies the
/// blob directory through <see cref="BlobDirectory"/> so the converter can build
/// an absolute path from the file name stored in the database.
/// </summary>
public sealed class TypeToThumbnailConverter : IValueConverter
{
    /// <summary>Absolute path of the blobs folder; set once at startup.</summary>
    public static string? BlobDirectory { get; set; }

    public object? Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is not ClipboardItem { Type: ClipboardItemType.Image } item
            || string.IsNullOrEmpty(item.BlobPath)
            || string.IsNullOrEmpty(BlobDirectory))
        {
            return null;
        }

        var fullPath = System.IO.Path.Combine(BlobDirectory, item.BlobPath);
        if (!System.IO.File.Exists(fullPath))
        {
            return null;
        }

        try
        {
            var bitmap = new BitmapImage();
            bitmap.BeginInit();
            bitmap.CacheOption = BitmapCacheOption.OnLoad;
            bitmap.DecodePixelWidth = 320;
            bitmap.UriSource = new Uri(fullPath);
            bitmap.EndInit();
            bitmap.Freeze();
            return bitmap;
        }
        catch (Exception)
        {
            return null;
        }
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}

/// <summary>True when the bound item is an image, used to swap body content.</summary>
public sealed class TypeIsImageConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        var isImage = value is ClipboardItemType.Image;
        return isImage ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
