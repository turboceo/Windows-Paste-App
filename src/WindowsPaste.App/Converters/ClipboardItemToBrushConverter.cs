using System.Globalization;
using System.Windows.Data;
using System.Windows.Media;
using WindowsPaste.Models;
using Brush = System.Windows.Media.Brush;
using Color = System.Windows.Media.Color;

namespace WindowsPaste.Converters;

/// <summary>
/// Supplies the card background tint per clipboard type. Tints are light and
/// saturated enough to stay legible against the white frosted shelf.
/// </summary>
public sealed class ClipboardItemToBrushConverter : IValueConverter
{
    private static readonly Brush TextBrush = FrozenColor(0x1F, 0x3A, 0x78, 0xFF);
    private static readonly Brush ImageBrush = FrozenColor(0x1F, 0xE8, 0x7A, 0x1E);
    private static readonly Brush FilesBrush = FrozenColor(0x1F, 0x1E, 0xA0, 0x6A);
    private static readonly Brush DefaultBrush = FrozenColor(0x14, 0x00, 0x00, 0x00);

    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        return value switch
        {
            ClipboardItem { Type: ClipboardItemType.Text } => TextBrush,
            ClipboardItem { Type: ClipboardItemType.Image } => ImageBrush,
            ClipboardItem { Type: ClipboardItemType.Files } => FilesBrush,
            _ => DefaultBrush,
        };
    }

    private static Brush FrozenColor(byte a, byte r, byte g, byte b)
    {
        var brush = new SolidColorBrush(Color.FromArgb(a, r, g, b));
        brush.Freeze();
        return brush;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}

/// <summary>
/// Supplies a dark, saturated icon/label colour per type so glyphs read clearly
/// on the light frosted background instead of fading into it. Colours are darker
/// than the card tints so the glyphs clear WCAG AA contrast with room to spare.
/// </summary>
public sealed class ClipboardItemToGlyphBrushConverter : IValueConverter
{
    private static readonly Brush TextBrush = FrozenColor(0x1E, 0x51, 0xC8);
    private static readonly Brush ImageBrush = FrozenColor(0x9A, 0x4A, 0x00);
    private static readonly Brush FilesBrush = FrozenColor(0x0A, 0x5A, 0x39);
    private static readonly Brush DefaultBrush = FrozenColor(0x33, 0x33, 0x3A);

    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        return value switch
        {
            ClipboardItem { Type: ClipboardItemType.Text } => TextBrush,
            ClipboardItem { Type: ClipboardItemType.Image } => ImageBrush,
            ClipboardItem { Type: ClipboardItemType.Files } => FilesBrush,
            _ => DefaultBrush,
        };
    }

    private static Brush FrozenColor(byte r, byte g, byte b)
    {
        var brush = new SolidColorBrush(Color.FromRgb(r, g, b));
        brush.Freeze();
        return brush;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
