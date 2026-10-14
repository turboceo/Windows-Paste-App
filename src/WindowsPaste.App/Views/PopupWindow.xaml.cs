using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using WindowsPaste.Models;
using WindowsPaste.ViewModels;

namespace WindowsPaste.Views;

public partial class PopupWindow : Window
{
    private const double SlideDistance = 100;

    /// <summary>Target width of one card cell, including its margins.</summary>
    private const double CardCellWidth = 184;

    /// <summary>Panel occupies these fractions of the monitor's working area.</summary>
    private const double WidthFraction = 0.9;
    private const double HeightFraction = 0.6;

    private bool _allowDeactivateClose = true;
    private double _restingTop;
    private System.Windows.Threading.DispatcherTimer? _resizeDebounce;

    public PopupWindow(MainViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;

        SearchBox.TextChanged += (_, _) =>
        {
            SearchPlaceholder.Visibility = string.IsNullOrEmpty(SearchBox.Text)
                ? Visibility.Visible
                : Visibility.Collapsed;
        };

        // After filtering, rewind the grid so the first matches are on screen.
        viewModel.SearchChanged += (_, _) =>
        {
            ItemsList.UpdateLayout();
            var scrollViewer = FindScrollViewer(ItemsList);
            scrollViewer?.ScrollToHome();
            UpdateColumnsPerRow();
        };

        // The card width is fixed, so remeasure the column count on resize. A
        // short debounce avoids re-laying out on every pixel of a window drag.
        _resizeDebounce = new System.Windows.Threading.DispatcherTimer
        {
            Interval = TimeSpan.FromMilliseconds(60),
        };
        _resizeDebounce.Tick += (_, _) =>
        {
            _resizeDebounce.Stop();
            UpdateColumnsPerRow();
        };

        SizeChanged += (_, _) =>
        {
            if (_resizeDebounce is null)
            {
                return;
            }

            _resizeDebounce.Stop();
            _resizeDebounce.Start();
        };
    }

    /// <summary>
    /// Chooses how many cards fit across the grid so every row fills the panel
    /// edge to edge. The count drives the UniformGrid, rather than being measured
    /// from it, which is what keeps the layout from overflowing horizontally.
    /// </summary>
    private void UpdateColumnsPerRow()
    {
        var grid = FindUniformGrid(ItemsList);
        var available = ItemsList.ActualWidth;
        if (grid is null || available <= 0)
        {
            return;
        }

        // Subtract the padding the ListBox adds around the panel.
        var usable = available - ItemsList.Padding.Left - ItemsList.Padding.Right;
        if (usable <= 0)
        {
            return;
        }

        // Each cell is CardCellWidth wide including its margins.
        var columns = Math.Max(1, (int)Math.Floor(usable / CardCellWidth));

        // Avoid re-laying out when nothing actually changed.
        if (grid.Columns == columns && ViewModel.ColumnsPerRow == columns)
        {
            return;
        }

        grid.Columns = columns;
        ViewModel.ColumnsPerRow = columns;
    }

    /// <summary>Finds the UniformGrid inside the ListBox's items presenter.</summary>
    private static System.Windows.Controls.Primitives.UniformGrid? FindUniformGrid(
        System.Windows.DependencyObject root)
    {
        if (root is System.Windows.Controls.Primitives.UniformGrid grid)
        {
            return grid;
        }

        var count = System.Windows.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = System.Windows.Media.VisualTreeHelper.GetChild(root, i);
            var found = FindUniformGrid(child);
            if (found is not null)
            {
                return found;
            }
        }

        return null;
    }

    private static System.Windows.Controls.ScrollViewer? FindScrollViewer(
        System.Windows.DependencyObject root)
    {
        if (root is System.Windows.Controls.ScrollViewer viewer)
        {
            return viewer;
        }

        var count = System.Windows.Media.VisualTreeHelper.GetChildrenCount(root);
        for (var i = 0; i < count; i++)
        {
            var child = System.Windows.Media.VisualTreeHelper.GetChild(root, i);
            var found = FindScrollViewer(child);
            if (found is not null)
            {
                return found;
            }
        }

        return null;
    }

    public MainViewModel ViewModel => (MainViewModel)DataContext;

    // Test-only accessors so the layout harness can drive the real controls.
    internal System.Windows.Controls.TextBox SearchBoxForTest => SearchBox;
    internal System.Windows.Controls.TextBlock SearchPlaceholderForTest => SearchPlaceholder;
    internal System.Windows.Controls.ListBox ItemsListForTest => ItemsList;
    internal double RestingTopForTest => _restingTop;
    internal System.Windows.Controls.Button PrevButtonForTest => PrevButton;
    internal System.Windows.Controls.Button NextButtonForTest => NextButton;
    internal int CardGridColumnsForTest => FindUniformGrid(ItemsList)?.Columns ?? -1;
    internal System.Windows.Controls.ScrollViewer? ScrollViewerForTest => FindScrollViewer(ItemsList);

    public event EventHandler<ClipboardItem>? ItemActivated;
    public event EventHandler<ClipboardItem>? ItemDeleted;
    public event EventHandler<ClipboardItem>? ItemPinToggled;

    /// <summary>Shows the shelf anchored above the bottom of the monitor under the cursor.</summary>
    public void ShowForCurrentScreen()
    {
        ViewModel.SearchText = string.Empty;
        ViewModel.Reload(preserveSelection: false);

        PositionOnActiveScreen();

        // Start slightly below the resting position, then drift up into place.
        _restingTop = Top;
        Opacity = 0;
        Top = _restingTop + SlideDistance;

        Show();
        ApplyFrostedBackdrop();
        Activate();
        SearchBox.Focus();
        Keyboard.Focus(SearchBox);

        // Grid is laid out now, so the row size is measurable for arrow keys.
        UpdateColumnsPerRow();

        BeginSlideIn();
    }

    /// <summary>
    /// Eases the panel into place with a short upward drift and a fade, which
    /// suits a centred dialog better than the old off-screen slide.
    /// </summary>
    private void BeginSlideIn()
    {
        var slide = new System.Windows.Media.Animation.DoubleAnimation
        {
            From = _restingTop + SlideDistance,
            To = _restingTop,
            Duration = TimeSpan.FromMilliseconds(240),
            EasingFunction = new System.Windows.Media.Animation.CubicEase
            {
                EasingMode = System.Windows.Media.Animation.EasingMode.EaseOut,
            },
            FillBehavior = System.Windows.Media.Animation.FillBehavior.Stop,
        };

        slide.Completed += (_, _) => Top = _restingTop;

        BeginAnimation(TopProperty, slide);

        var fade = new System.Windows.Media.Animation.DoubleAnimation
        {
            From = 0,
            To = 1,
            Duration = TimeSpan.FromMilliseconds(170),
            EasingFunction = new System.Windows.Media.Animation.CubicEase
            {
                EasingMode = System.Windows.Media.Animation.EasingMode.EaseOut,
            },
            FillBehavior = System.Windows.Media.Animation.FillBehavior.Stop,
        };

        // Stop() reverts opacity to its base value, so make the base value 1
        // afterwards; otherwise the panel animates in then disappears.
        fade.Completed += (_, _) => Opacity = 1;

        BeginAnimation(OpacityProperty, fade);
    }

    /// <summary>
    /// Drifts the panel down and fades it out. Used when dismissing without a
    /// paste so the exit matches the entry motion.
    /// </summary>
    private void SlideOutAndHide()
    {
        var slide = new System.Windows.Media.Animation.DoubleAnimation
        {
            From = Top,
            To = _restingTop + SlideDistance,
            Duration = TimeSpan.FromMilliseconds(160),
            EasingFunction = new System.Windows.Media.Animation.CubicEase
            {
                EasingMode = System.Windows.Media.Animation.EasingMode.EaseIn,
            },
            FillBehavior = System.Windows.Media.Animation.FillBehavior.Stop,
        };

        slide.Completed += (_, _) =>
        {
            Top = _restingTop;
            Opacity = 1;
            Hide();
        };

        BeginAnimation(TopProperty, slide);

        BeginAnimation(OpacityProperty, new System.Windows.Media.Animation.DoubleAnimation
        {
            From = 1,
            To = 0,
            Duration = TimeSpan.FromMilliseconds(130),
            FillBehavior = System.Windows.Media.Animation.FillBehavior.Stop,
        });
    }

    /// <summary>
    /// Frosts the shelf. The window must be shown first so its HWND exists, and
    /// the acrylic tint is a translucent white so the desktop shows through.
    /// </summary>
    private void ApplyFrostedBackdrop()
    {
        // 0xAABBGGRR: alpha 0xB4, white tint.
        const int frostedWhite = unchecked((int)0xB4FFFFFF);
        Services.WindowBlur.ApplyAcrylic(this, frostedWhite);
    }

    private void PositionOnActiveScreen()
    {
        var cursor = System.Windows.Forms.Cursor.Position;
        var screen = System.Windows.Forms.Screen.FromPoint(cursor);
        var area = screen.WorkingArea;

        var dpi = VisualTreeHelper.GetDpi(this);

        // 90% wide and 60% tall, centred on the monitor under the cursor.
        var widthPx = area.Width * WidthFraction;
        var heightPx = area.Height * HeightFraction;

        Width = widthPx / dpi.DpiScaleX;
        Height = heightPx / dpi.DpiScaleY;

        Left = (area.Left + (area.Width - widthPx) / 2.0) / dpi.DpiScaleX;
        Top = (area.Top + (area.Height - heightPx) / 2.0) / dpi.DpiScaleY;
    }

    private void OnDeactivated(object? sender, EventArgs e)
    {
        if (_allowDeactivateClose)
        {
            SlideOutAndHide();
        }
    }

    private void OnPreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        switch (e.Key)
        {
            case Key.Escape:
                e.Handled = true;
                SlideOutAndHide();
                break;

            case Key.Right:
                e.Handled = true;
                ViewModel.MoveSelection(1);
                ScrollSelectionIntoView();
                break;

            case Key.Left:
                e.Handled = true;
                ViewModel.MoveSelection(-1);
                ScrollSelectionIntoView();
                break;

            // Up/Down hop whole rows, matching the wrapping grid.
            case Key.Down:
                e.Handled = true;
                ViewModel.MoveSelectionByRow(1);
                ScrollSelectionIntoView();
                break;

            case Key.Up:
                e.Handled = true;
                ViewModel.MoveSelectionByRow(-1);
                ScrollSelectionIntoView();
                break;

            case Key.Enter:
                e.Handled = true;
                ActivateSelected();
                break;

            case Key.Delete:
                e.Handled = true;
                DeleteSelected();
                break;

            case Key.P when Keyboard.Modifiers == ModifierKeys.Control:
                e.Handled = true;
                TogglePinSelected();
                break;
        }
    }

    private void ScrollSelectionIntoView()
    {
        var item = ViewModel.SelectedItem;
        if (item is null)
        {
            return;
        }

        ItemsList.ScrollIntoView(item);

        // Nudge the container fully into view; WrapPanel rows can leave the
        // selected card half-clipped at the region edge.
        ItemsList.UpdateLayout();
        if (ItemsList.ItemContainerGenerator.ContainerFromItem(item)
            is System.Windows.FrameworkElement container)
        {
            container.BringIntoView();
        }
    }

    private void OnItemActivated(object sender, MouseButtonEventArgs e)
    {
        ActivateSelected();
    }

    private void OnPreviousClick(object sender, RoutedEventArgs e)
    {
        ViewModel.MoveSelection(-1);
        ScrollSelectionIntoView();
    }

    private void OnNextClick(object sender, RoutedEventArgs e)
    {
        ViewModel.MoveSelection(1);
        ScrollSelectionIntoView();
    }

    /// <summary>Pin toggle on a card header. The sender's DataContext is the item.</summary>
    private void OnPinButtonClick(object sender, RoutedEventArgs e)
    {
        if (sender is System.Windows.FrameworkElement { DataContext: ClipboardItem item })
        {
            ItemPinToggled?.Invoke(this, item);
        }
    }

    private void OnContextPaste(object sender, RoutedEventArgs e)
    {
        if (ResolveContextItem(sender) is { } item)
        {
            ViewModel.SelectedItem = item;
            ActivateSelected();
        }
    }

    private void OnContextPin(object sender, RoutedEventArgs e)
    {
        if (ResolveContextItem(sender) is { } item)
        {
            ItemPinToggled?.Invoke(this, item);
        }
    }

    private void OnContextRemove(object sender, RoutedEventArgs e)
    {
        if (ResolveContextItem(sender) is { } item)
        {
            ItemDeleted?.Invoke(this, item);
        }
    }

    /// <summary>
    /// Walks up from a context menu item to the card it belongs to. Menu items
    /// live in a separate visual tree, so the item is read from the placement
    /// target that the menu was opened against.
    /// </summary>
    private ClipboardItem? ResolveContextItem(object sender)
    {
        if (sender is System.Windows.Controls.MenuItem menuItem
            && menuItem.Parent is System.Windows.Controls.ContextMenu { PlacementTarget: var target }
            && target is System.Windows.FrameworkElement { DataContext: ClipboardItem item })
        {
            return item;
        }

        return ViewModel.SelectedItem;
    }

    private void ActivateSelected()
    {
        var item = ViewModel.SelectedItem;
        if (item is null)
        {
            return;
        }

        // Hide first so the simulated Ctrl+V reaches the previous foreground window.
        _allowDeactivateClose = false;
        Hide();
        _allowDeactivateClose = true;

        ItemActivated?.Invoke(this, item);
    }

    private void DeleteSelected()
    {
        var item = ViewModel.SelectedItem;
        if (item is null)
        {
            return;
        }

        ItemDeleted?.Invoke(this, item);
    }

    private void TogglePinSelected()
    {
        var item = ViewModel.SelectedItem;
        if (item is null)
        {
            return;
        }

        ItemPinToggled?.Invoke(this, item);
    }
}
