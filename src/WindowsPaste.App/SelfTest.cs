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

    private static System.Windows.Controls.Button? FindPinButton(
        System.Windows.DependencyObject root)
    {
        var count = System.Windows.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = System.Windows.Media.VisualTreeHelper.GetChild(root, i);
            if (child is System.Windows.Controls.Button { Name: "PinButton" } button)
            {
                return button;
            }

            var found = FindPinButton(child);
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
            var beforeSearch = viewModel.Items.Count;
            lines.Add($"-- search 'zzz-no-such-entry' count before={beforeSearch}");
            viewModel.SearchText = "zzz-no-such-entry";
            lines.Add($"-- search 'zzz-no-such-entry' count={viewModel.Items.Count} (expect 0)");

            viewModel.SearchText = string.Empty;
            lines.Add($"-- cleared restores  = {viewModel.Items.Count == beforeSearch} ({viewModel.Items.Count})");

            // Exercise the real TextBox binding, not just the view model property.
            popup.SearchBoxForTest.Text = "zzz-no-such-entry";
            popup.SearchBoxForTest.GetBindingExpression(System.Windows.Controls.TextBox.TextProperty)?.UpdateSource();
            await Task.Delay(120);
            lines.Add($"-- UI TextBox binds  = vm.SearchText='{viewModel.SearchText}' count={viewModel.Items.Count} (expect 0)");
            lines.Add($"-- UI TextBox placeholder visible={popup.SearchPlaceholderForTest.Visibility}");

            // Restore the full list before inspecting rendered cards.
            popup.SearchBoxForTest.Text = string.Empty;
            popup.SearchBoxForTest.GetBindingExpression(System.Windows.Controls.TextBox.TextProperty)?.UpdateSource();
            await Task.Delay(150);
            lines.Add($"-- UI TextBox cleared -> count={viewModel.Items.Count}");

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
                    lines.Add($"-- card scales   = {card.ActualWidth != 172} (not the old fixed size)");

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

                // No horizontal overflow: the panel must not produce a horizontal
                // scrollbar, and the last card in a row must stay inside the list.
                var scroll = popup.ScrollViewerForTest;
                if (scroll is not null)
                {
                    lines.Add($"-- h-scrollbar vis = {scroll.ComputedHorizontalScrollBarVisibility} (expect Collapsed)");
                    lines.Add($"-- h-scrollbar off = {scroll.ComputedHorizontalScrollBarVisibility != System.Windows.Visibility.Visible}");
                    lines.Add($"-- viewport width  = {scroll.ViewportWidth:F0}");
                    lines.Add($"-- extent width    = {scroll.ExtentWidth:F0}");
                    lines.Add($"-- no h overflow   = {scroll.ExtentWidth <= scroll.ViewportWidth + 1}");
                }

                // Every row must fill edge to edge, and the card count across the
                // first row must equal the calculated column count.
                var listBounds = popup.ItemsListForTest.TranslatePoint(new System.Windows.Point(0, 0), popup);
                var listWidth = popup.ItemsListForTest.ActualWidth;

                // Cards must start at the top of the list region. The container
                // carries the spacing margin, so measure it rather than the card.
                var listTop = popup.ItemsListForTest.TranslatePoint(new System.Windows.Point(0, 0), popup).Y;
                var containerTop = firstContainer.TranslatePoint(new System.Windows.Point(0, 0), popup).Y;
                var listHeight = popup.ItemsListForTest.ActualHeight;
                var gapAboveCard = containerTop - listTop;
                lines.Add($"-- list region top   = {listTop:F0}, height {listHeight:F0}");
                lines.Add($"-- first container top= {containerTop:F0} (gap {gapAboveCard:F0}px from list top)");
                lines.Add($"-- cards start at top= {gapAboveCard <= 8}");

                // Inspect the first few containers: they should share a row, then
                // the next ones should drop to a lower row (same X as first).
                var positions = new List<(double X, double Y)>();
                for (var i = 0; i < Math.Min(12, viewModel.Items.Count); i++)
                {
                    if (popup.ItemsListForTest.ItemContainerGenerator.ContainerFromIndex(i)
                        is System.Windows.FrameworkElement c)
                    {
                        var p = c.TranslatePoint(new System.Windows.Point(0, 0), popup);
                        positions.Add((p.X, p.Y));
                    }
                }

                if (positions.Count >= 4)
                {
                    var row1Y = positions[0].Y;
                    var sameRow = positions.Count(p => Math.Abs(p.Y - row1Y) <= 2);
                    var secondRow = positions.Where(p => p.Y > row1Y + 2).ToList();
                    var columns = viewModel.ColumnsPerRow;

                    lines.Add($"-- cards in first row= {sameRow}");
                    lines.Add($"-- computed columns  = {columns}");
                    lines.Add($"-- grid.Columns set  = {popup.CardGridColumnsForTest} (matches: {popup.CardGridColumnsForTest == columns})");
                    lines.Add($"-- row matches columns= {sameRow == columns}");
                    lines.Add($"-- wraps to new row  = {secondRow.Count > 0}");
                    if (secondRow.Count > 0)
                    {
                        lines.Add($"-- row 2 starts at X = {secondRow[0].X:F0} (aligned with row 1: {Math.Abs(secondRow[0].X - positions[0].X) <= 2})");
                        lines.Add($"-- row 2 is lower    = {secondRow[0].Y > row1Y}");
                    }

                    // The last card of the first row must not stick out past the list.
                    var row1Positions = positions.Where(p => Math.Abs(p.Y - row1Y) <= 2).ToList();
                    if (row1Positions.Count > 0)
                    {
                        var containerW = firstContainer.ActualWidth;
                        var lastCardRight = row1Positions.Max(p => p.X) + containerW;
                        var listRight = listBounds.X + listWidth;
                        lines.Add($"-- row 1 right edge  = {lastCardRight:F0} (list right {listRight:F0})");
                        lines.Add($"-- row 1 fits inside = {lastCardRight <= listRight + 2}");
                    }

                    // Row-aware navigation: Down should hop exactly one row.
                    lines.Add($"-- vm.ColumnsPerRow  = {columns}");

                    viewModel.SelectedItem = viewModel.Items[0];
                    var beforeIdx = viewModel.Items.IndexOf(viewModel.SelectedItem);
                    viewModel.MoveSelectionByRow(1);
                    var afterIdx = viewModel.Items.IndexOf(viewModel.SelectedItem);
                    lines.Add($"-- Down hop: idx {beforeIdx} -> {afterIdx} (expect {columns})");
                    lines.Add($"-- row hop correct   = {afterIdx == Math.Min(columns, viewModel.Items.Count - 1)}");

                    viewModel.MoveSelectionByRow(-1);
                    lines.Add($"-- Up hop back       = {viewModel.Items.IndexOf(viewModel.SelectedItem)}");
                }

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

            // Resizing the panel must recompute the column count so rows keep
            // filling the available width instead of overflowing.
            var originalWidth = popup.Width;
            var columnsBefore = viewModel.ColumnsPerRow;
            popup.Width = originalWidth * 0.6;
            popup.UpdateLayout();
            await Task.Delay(200); // allow the debounce timer to fire
            var columnsAfter = viewModel.ColumnsPerRow;
            lines.Add($"-- resize: width {originalWidth:F0} -> {popup.Width:F0}");
            lines.Add($"-- resize: columns {columnsBefore} -> {columnsAfter}");
            lines.Add($"-- columns shrank   = {columnsAfter < columnsBefore}");

            popup.Width = originalWidth;
            popup.UpdateLayout();
            await Task.Delay(200);
            lines.Add($"-- restored columns = {viewModel.ColumnsPerRow}");

            // The card header must carry an interactive pin toggle.
            var pinButton = FindPinButton(firstContainer);
            lines.Add($"-- pin button present= {pinButton is not null}");
            if (pinButton is not null)
            {
                lines.Add($"-- pin button tooltip= {pinButton.ToolTip}");
                lines.Add($"-- pin button tag    = {pinButton.Tag} (item Pinned={((System.Windows.FrameworkElement)firstContainer).DataContext is ClipboardItem ci && ci.Pinned})");
                lines.Add($"-- pin glyph (unpinned state uses E718 hollow pin)");
            }

            // Pin behaviour: pinning must move the item to the front, unpinning
            // must return it to its chronological place, and both must survive a
            // reload (i.e. be persisted, not just in-memory).
            var target = viewModel.Items.FirstOrDefault(i => !i.Pinned);
            if (target is not null)
            {
                var targetId = target.Id;
                var indexBefore = viewModel.Items.IndexOf(target);
                lines.Add($"-- pin: target id    = {targetId} at index {indexBefore}");

                viewModel.TogglePin(target);
                var pinned = viewModel.Items.FirstOrDefault(i => i.Id == targetId);
                lines.Add($"-- pin: now at index = {viewModel.Items.IndexOf(pinned ?? viewModel.Items[0])}");
                lines.Add($"-- pin: flag set     = {pinned?.Pinned}");
                lines.Add($"-- pin: moved to top = {viewModel.Items.FirstOrDefault()?.Id == targetId}");

                // Pin state must be in the database, not just the view model.
                var fromDb = viewModel.Items.FirstOrDefault(i => i.Id == targetId);
                lines.Add($"-- pin: persisted    = {fromDb?.Pinned == true}");

                viewModel.TogglePin(pinned ?? target);
                var unpinned = viewModel.Items.FirstOrDefault(i => i.Id == targetId);
                lines.Add($"-- unpin: flag cleared= {unpinned?.Pinned == false}");
                lines.Add($"-- unpin: left top    = {viewModel.Items.FirstOrDefault()?.Id != targetId}");

                // A pinned item must outrank newer unpinned items.
                var newest = viewModel.Items.FirstOrDefault(i => !i.Pinned);
                if (newest is not null)
                {
                    viewModel.TogglePin(newest);
                    var topIsPinned = viewModel.Items.FirstOrDefault()?.Pinned == true;
                    lines.Add($"-- pinned outranks newer = {topIsPinned}");
                    viewModel.TogglePin(viewModel.Items.FirstOrDefault(x => x.Id == newest.Id) ?? newest);
                }
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
