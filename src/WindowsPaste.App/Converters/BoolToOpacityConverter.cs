using System.Globalization;
using System.Windows.Data;

namespace WindowsPaste.Converters;

/// <summary>
/// Maps a boolean to an opacity, so a control can fade in on hover without a
/// XAML trigger reaching across template boundaries.
/// </summary>
public sealed class BoolToOpacityConverter : IValueConverter
{
    /// <summary>Opacity applied when the bound value is true.</summary>
    public double TrueOpacity { get; set; } = 1.0;

    /// <summary>Opacity applied when the bound value is false.</summary>
    public double FalseOpacity { get; set; }

    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        => value is bool flag && flag ? TrueOpacity : FalseOpacity;

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
