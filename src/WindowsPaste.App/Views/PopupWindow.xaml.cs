using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using WindowsPaste.Models;
using WindowsPaste.ViewModels;

namespace WindowsPaste.Views;

public partial class PopupWindow : Window
{
    private const double SlideDistance = 100;

    /// <summary>Panel occupies these fractions of the monitor's working area.</summary>
    private const double WidthFraction = 0.9;
    private const double HeightFraction = 0.6;

    private bool _allowDeactivateClose = true;
    private double _restingTop;

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

        // The card width is fixed, so remeasure the column count on resize.
        SizeChanged += (_, _) => UpdateColumnsPerRow();
    }

    /// <summary>
    /// Counts how many cards fit across the grid so Up/Down can move by whole
    /// rows. Measured from the rendered containers rather than assumed.
    /// </summary>
    private void UpdateColumnsPerRow()
    {
        if (ViewModel is null || ItemsList.ActualWidth <= 0)
        {
            return;
        }

        var count = Math.Min(ViewModel.Items.Count, 64);
        if (count == 0)
        {
            ViewModel.ColumnsPerRow = 1;
            return;
        }

        ItemsList.UpdateLayout();

        var first = ItemsList.ItemContainerGenerator.ContainerFromIndex(0) as System.Windows.FrameworkElement;
        if (first is null || first.ActualWidth <= 0)
        {
            return;
        }

        // Count containers sharing the first card's row.
        var rowTop = first.TranslatePoint(new System.Windows.Point(0, 0), ItemsList).Y;
        var columns = 0;
        for (var i = 0; i < count; i++)
        {
            if (ItemsList.ItemContainerGenerator.ContainerFromIndex(i)
                is System.Windows.FrameworkElement container)
            {
                var top = container.TranslatePoint(new System.Windows.Point(0, 0), ItemsList).Y;
                if (Math.Abs(top - rowTop) > 2)
                {
                    break;
                }

                columns++;
            }
        }

        ViewModel.ColumnsPerRow = Math.Max(1, columns);
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
