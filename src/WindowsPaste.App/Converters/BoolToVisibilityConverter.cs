using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace WindowsPaste.Converters;

public sealed class BoolToVisibilityConverter : IValueConverter
{
    public bool Invert { get; set; }

    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        var flag = value is bool b && b;
        if (Invert)
        {
            flag = !flag;
        }

        return flag ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}

public sealed class InverseBoolToVisibilityConverter : IValueConverter
{
    private readonly BoolToVisibilityConverter _inner = new() { Invert = true };

    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        => _inner.Convert(value, targetType, parameter, culture);

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
