using System.Globalization;
using System.Windows.Data;
using WindowsPaste.Models;

namespace WindowsPaste.Converters;

public sealed class TypeToGlyphConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        return value switch
        {
            ClipboardItem item => GlyphFor(item.Type),
            ClipboardItemType type => GlyphFor(type),
            _ => "\uE8A5",
        };
    }

    private static string GlyphFor(ClipboardItemType type) => type switch
    {
        ClipboardItemType.Image => "\uEB9F",
        ClipboardItemType.Files => "\uE8B7",
        _ => "\uE8A5",
    };

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
