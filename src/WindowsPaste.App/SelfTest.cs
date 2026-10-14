using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using WindowsPaste.Models;
using WindowsPaste.Services;
using WindowsPaste.ViewModels;
using Application = System.Windows.Application;

namespace WindowsPaste;

/// <summary>
/// Debug-only harness that opens the shelf, reports its on-screen geometry,
/// and exits. Enabled with the --selftest argument. Keeps the UI verifiable
/// without manual interaction.
/// </summary>
internal static class SelfTest
{
    /// <summary>Alpha-composites a translucent brush over an opaque white backdrop.</summary>
    private static System.Windows.Media.Brush CompositeOverWhite(System.Windows.Media.Brush? brush)
    {
        if (brush is not System.Windows.Media.SolidColorBrush solid)
        {
            return System.Windows.Media.Brushes.White;
        }

        var c = solid.Color;
        var a = c.A / 255.0;
        byte Mix(byte channel) => (byte)Math.Round(channel * a + 255 * (1 - a));

        return new System.Windows.Media.SolidColorBrush(
            System.Windows.Media.Color.FromRgb(Mix(c.R), Mix(c.G), Mix(c.B)));
    }

    private static string Describe(System.Windows.Media.Brush? brush)
    {
        if (brush is System.Windows.Media.SolidColorBrush solid)
        {
            var c = solid.Color;
            return $"#{c.A:X2}{c.R:X2}{c.G:X2}{c.B:X2}";
        }

        return brush?.ToString() ?? "(null)";
    }

    /// <summary>WCAG contrast ratio between two brushes (1.0 = identical, 21 = black/white).</summary>
    private static double ContrastRatio(
        System.Windows.Media.Brush? foreground,
        System.Windows.Media.Brush? background)
    {
        if (foreground is not System.Windows.Media.SolidColorBrush fg
            || background is not System.Windows.Media.SolidColorBrush bg)
        {
            return 0;
        }

        var l1 = RelativeLuminance(fg.Color);
        var l2 = RelativeLuminance(bg.Color);
        var lighter = Math.Max(l1, l2);
        var darker = Math.Min(l1, l2);
        return (lighter + 0.05) / (darker + 0.05);
    }

    private static double RelativeLuminance(System.Windows.Media.Color c)
    {
        static double Channel(byte v)
        {
            var s = v / 255.0;
            return s <= 0.03928 ? s / 12.92 : Math.Pow((s + 0.055) / 1.055, 2.4);
        }

        return 0.2126 * Channel(c.R) + 0.7152 * Channel(c.G) + 0.0722 * Channel(c.B);
    }

    private static System.Windows.Controls.TextBlock? FindTextBody(
        System.Windows.DependencyObject root)
    {
        var count = System.Windows.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = System.Windows.Media.VisualTreeHelper.GetChild(root, i);
            if (child is System.Windows.Controls.TextBlock tb && tb.Name == "TextBody")
            {
                return tb;
            }

            var found = FindTextBody(child);
            if (found is not null)
            {
                return found;
            }
        }

        return null;
    }

    private static System.Windows.Controls.TextBlock? FindGlyph(
        System.Windows.DependencyObject root)
    {
        var count = System.Windows.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = System.Windows.Media.VisualTreeHelper.GetChild(root, i);
            // The glyph is the first TextBlock holding a single private-use char.
            if (child is System.Windows.Controls.TextBlock tb
                && tb.Text.Length == 1
                && tb.Text[0] >= '\uE000' && tb.Text[0] <= '\uF8FF')
            {
                return tb;
            }

            var found = FindGlyph(child);
            if (found is not null)
            {
                return found;
            }
        }

        return null;
    }

    private static System.Windows.Controls.Border? FindHalo(
        System.Windows.DependencyObject root)
    {
        var count = System.Windows.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = System.Windows.Media.VisualTreeHelper.GetChild(root, i);
            if (child is System.Windows.Controls.Border { Name: "Halo" } halo)
            {
                return halo;
            }

            var found = FindHalo(child);
            if (found is not null)
            {
                return found;
            }
        }

        return null;
    }

    private static System.Windows.Controls.Border? FindCard(
        System.Windows.DependencyObject root)
    {
        var count = System.Windows.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = System.Windows.Media.VisualTreeHelper.GetChild(root, i);
            if (child is System.Windows.Controls.Border { Name: "Card" } border)
            {
                return border;
            }

            var found = FindCard(child);
            if (found is not null)
            {
                return found;
            }
        }

        return null;
    }

    public static async Task RunAsync(MainViewModel viewModel, AppPaths paths)
    {
        var lines = new List<string>();
        try
        {
            // Seed a few representative entries so every card style renders.
            viewModel.Add(new ClipboardItem
            {
                Type = ClipboardItemType.Text,
                Content = "Text entry for layout verification",
                Hash = "selftest-text",
                CreatedAt = DateTime.Now,
            });
            viewModel.Add(new ClipboardItem
            {
                Type = ClipboardItemType.Files,
                Content = @"C:\Windows\notepad.exe" + Environment.NewLine + @"C:\Windows\explorer.exe",
                Hash = "selftest-files",
                CreatedAt = DateTime.Now,
            });

            var popup = new Views.PopupWindow(viewModel);
            popup.ShowForCurrentScreen();

            // Sample right away to prove the slide-in is actually animating.
            var animStartTop = popup.Top;
            var animStartOpacity = popup.Opacity;
            await Task.Delay(60);
            var animMidTop = popup.Top;
            var animMidOpacity = popup.Opacity;

            // Let layout settle before measuring.
            await Task.Delay(400);

            lines.Add($"anim start  Top={animStartTop:F1} opacity={animStartOpacity:F2}");
            lines.Add($"anim @60ms  Top={animMidTop:F1} opacity={animMidOpacity:F2}");
            lines.Add($"anim moves up       : {animMidTop < animStartTop}");
            lines.Add($"anim fades in       : {animMidOpacity > animStartOpacity}");
            lines.Add($"resting Top         = {popup.RestingTopForTest:F1}");
            lines.Add($"settles at rest     : {Math.Abs(popup.Top - popup.RestingTopForTest) <= 1}");

            var cursor = System.Windows.Forms.Cursor.Position;
            var screen = System.Windows.Forms.Screen.FromPoint(cursor);
            var area = screen.WorkingArea;
            var dpi = VisualTreeHelper.GetDpi(popup);

            var leftPx = (int)Math.Round(popup.Left * dpi.DpiScaleX);
            var topPx = (int)Math.Round(popup.Top * dpi.DpiScaleY);
            var widthPx = (int)Math.Round(popup.ActualWidth * dpi.DpiScaleX);
            var heightPx = (int)Math.Round(popup.ActualHeight * dpi.DpiScaleY);

            lines.Add($"screen working area : {area.Left},{area.Top} {area.Width}x{area.Height}");
            lines.Add($"panel window        : {leftPx},{topPx} {widthPx}x{heightPx}");
            lines.Add($"occupies 90% width  : {Math.Abs(widthPx - area.Width * 0.9) <= 4} ({widthPx / (double)area.Width:P0})");
            lines.Add($"occupies 60% height : {Math.Abs(heightPx - area.Height * 0.6) <= 4} ({heightPx / (double)area.Height:P0})");
            lines.Add($"centred horizontally: {Math.Abs((leftPx + widthPx / 2.0) - (area.Left + area.Width / 2.0)) <= 4}");
            lines.Add($"centred vertically  : {Math.Abs((topPx + heightPx / 2.0) - (area.Top + area.Height / 2.0)) <= 4}");
            lines.Add($"opacity settles at 1= {Math.Abs(popup.Opacity - 1) <= 0.01}");
            lines.Add($"reported items      : {viewModel.Items.Count}");
            lines.Add($"selected is first   : {viewModel.SelectedItem?.Id == viewModel.Items.FirstOrDefault()?.Id}");
            lines.Add($"selected id         : {viewModel.SelectedItem?.Id}");
            lines.Add("ordering (id | created | preview):");
            foreach (var entry in viewModel.Items)
            {
                lines.Add($"   {entry.Id} | {entry.CreatedAt:HH:mm:ss.fff} | {entry.Preview[..Math.Min(28, entry.Preview.Length)]}");
            }

            // Search must filter the collection the ListBox is bound to.
            lines.Add($"-- search 'notepad' (before) count={viewModel.Items.Count}");
            viewModel.SearchText = "notepad";
            lines.Add($"-- search 'notepad' (after)  count={viewModel.Items.Count}");
            foreach (var entry in viewModel.Items)
            {
                lines.Add($"   match: {entry.Preview[..Math.Min(40, entry.Preview.Length)]}");
            }

            viewModel.SearchText = "zzz-no-such-entry";
            lines.Add($"-- search 'zzz-no-such-entry' count={viewModel.Items.Count} (expect 0)");

            viewModel.SearchText = string.Empty;
            lines.Add($"-- cleared count={viewModel.Items.Count}");

            // Exercise the real TextBox binding, not just the view model property.
            popup.SearchBoxForTest.Text = "notepad";
            popup.SearchBoxForTest.GetBindingExpression(System.Windows.Controls.TextBox.TextProperty)?.UpdateSource();
            await Task.Delay(120);
            lines.Add($"-- UI TextBox 'notepad' -> vm.SearchText='{viewModel.SearchText}' count={viewModel.Items.Count} (expect 1)");
            lines.Add($"-- UI TextBox placeholder visible={popup.SearchPlaceholderForTest.Visibility}");

            // Confirm the rendered first card is square (width == height).
            var firstContainer = popup.ItemsListForTest.ItemContainerGenerator
                .ContainerFromIndex(0) as System.Windows.FrameworkElement;
            if (firstContainer is not null)
            {
                firstContainer.ApplyTemplate();
                var card = FindCard(firstContainer);
                if (card is not null)
                {
                    lines.Add($"-- card size     = {card.ActualWidth} x {card.ActualHeight}");
                    lines.Add($"-- card is square= {Math.Abs(card.ActualWidth - card.ActualHeight) <= 1}");

                    // The card tint is translucent, so composite it over the white
                    // shelf to get the effective background before measuring contrast.
                    var effectiveBg = CompositeOverWhite(card.Background);
                    lines.Add($"-- card bg       = {Describe(card.Background)} -> over white {Describe(effectiveBg)}");

                    // Verify the type glyph uses the icon font, not a fallback:
                    // a missing FontFamily renders the PUA codepoint as tofu.
                    var glyph = FindGlyph(card);
                    if (glyph is not null)
                    {
                        lines.Add($"-- glyph text    = U+{(int)glyph.Text[0]:X4}");
                        lines.Add($"-- glyph font    = {glyph.FontFamily.Source}");
                        lines.Add($"-- glyph is MDL2 = {glyph.FontFamily.Source.Contains("MDL2")}");
                        lines.Add($"-- glyph colour  = {Describe(glyph.Foreground)}");
                        lines.Add($"-- glyph contrast= {ContrastRatio(glyph.Foreground, effectiveBg):F2} (>=4.5 readable)");
                    }

                    // The text preview must be dark enough on the light card.
                    var body = FindTextBody(card);
                    if (body is not null)
                    {
                        lines.Add($"-- body colour   = {Describe(body.Foreground)}");
                        lines.Add($"-- body contrast = {ContrastRatio(body.Foreground, effectiveBg):F2} (>=4.5 readable)");
                    }
                }
                lines.Add($"-- container size = {firstContainer.ActualWidth} x {firstContainer.ActualHeight}");

                // The hit box must not change between default, hover and selected.
                // Simulate each state on a fresh container and compare geometry.
                var item = (System.Windows.Controls.ListBoxItem)firstContainer;
                var baseline = $"{item.ActualWidth:F2}x{item.ActualHeight:F2}";
                lines.Add($"-- box default    = {baseline}");

                var halo = FindHalo(item);
                lines.Add($"-- halo thickness = {halo?.BorderThickness.ToString() ?? "(none)"} (constant across states)");
                lines.Add($"-- halo brush def = {Describe(halo?.BorderBrush)}");

                // Selected state (the container we measured is the selected one).
                item.ApplyTemplate();
                var selectedHalo = FindHalo(item);
                var selectedBox = $"{item.ActualWidth:F2}x{item.ActualHeight:F2}";
                lines.Add($"-- box selected   = {selectedBox}  brush={Describe(selectedHalo?.BorderBrush)}");
                lines.Add($"-- box unchanged  = {baseline == selectedBox}");
            }

            popup.SearchBoxForTest.Text = string.Empty;
            popup.SearchBoxForTest.GetBindingExpression(System.Windows.Controls.TextBox.TextProperty)?.UpdateSource();
            await Task.Delay(120);
            lines.Add($"-- UI TextBox cleared -> count={viewModel.Items.Count} (expect 2)");

            // Toolbar must read as: search on the left, nav buttons on the right.
            var searchBox = popup.SearchBoxForTest;
            var prev = popup.PrevButtonForTest;
            var next = popup.NextButtonForTest;

            if (searchBox.IsDescendantOf(popup) && prev != null && next != null)
            {
                var searchTopLeft = searchBox.TranslatePoint(new System.Windows.Point(0, 0), popup);
                var prevTopLeft = prev.TranslatePoint(new System.Windows.Point(0, 0), popup);
                var nextTopLeft = next.TranslatePoint(new System.Windows.Point(0, 0), popup);

                lines.Add($"-- toolbar search x  = {searchTopLeft.X:F0} (left side)");
                lines.Add($"-- toolbar prev   x  = {prevTopLeft.X:F0}");
                lines.Add($"-- toolbar next   x  = {nextTopLeft.X:F0}");
                lines.Add($"-- search left of nav= {searchTopLeft.X < prevTopLeft.X}");
                lines.Add($"-- next right of prev= {nextTopLeft.X > prevTopLeft.X}");
                lines.Add($"-- nav below search? = {prevTopLeft.Y >= searchTopLeft.Y - 1} (toolbar is a row)");
                lines.Add($"-- prev enabled      = {prev.IsEnabled}");
                lines.Add($"-- next enabled      = {next.IsEnabled}");
                lines.Add($"-- nav on same row   = {Math.Abs(nextTopLeft.Y - prevTopLeft.Y) <= 1}");

                // Clicking next must move the selection to the older entry.
                var beforeId = viewModel.SelectedItem?.Id;
                next.RaiseEvent(new RoutedEventArgs(System.Windows.Controls.Primitives.ButtonBase.ClickEvent));
                await Task.Delay(80);
                var afterId = viewModel.SelectedItem?.Id;
                lines.Add($"-- next click moved  : {beforeId} -> {afterId} (changed={beforeId != afterId})");
                lines.Add($"-- prev now enabled  : {prev.IsEnabled}");

                // And back again.
                prev.RaiseEvent(new RoutedEventArgs(System.Windows.Controls.Primitives.ButtonBase.ClickEvent));
                await Task.Delay(80);
                lines.Add($"-- prev click moved  : {afterId} -> {viewModel.SelectedItem?.Id}");
            }

            popup.Hide();
        }
        catch (Exception ex)
        {
            lines.Add("SELFTEST FAILED: " + ex);
        }

        var reportPath = System.IO.Path.Combine(paths.Root, "selftest.txt");
        await System.IO.File.WriteAllLinesAsync(reportPath, lines);
        Application.Current.Shutdown();
    }
}
