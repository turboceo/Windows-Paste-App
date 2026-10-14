using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using WindowsPaste.Models;
using WindowsPaste.ViewModels;

namespace WindowsPaste.Views;

public partial class PopupWindow : Window
{
    private const double BottomOffset = 80;
    private const double SlideDistance = 100;

    /// <summary>Panel occupies this fraction of the monitor's working width.</summary>
    private const double WidthFraction = 0.9;

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

        // After filtering, rewind the strip so matches are visible from the left.
        viewModel.SearchChanged += (_, _) =>
        {
            ItemsList.UpdateLayout();
            var scrollViewer = FindScrollViewer(ItemsList);
            if (scrollViewer is not null)
            {
                scrollViewer.ScrollToLeftEnd();
            }
        };
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

    public event EventHandler<ClipboardItem>? ItemActivated;
    public event EventHandler<ClipboardItem>? ItemDeleted;
    public event EventHandler<ClipboardItem>? ItemPinToggled;

    /// <summary>Shows the shelf anchored above the bottom of the monitor under the cursor.</summary>
    public void ShowForCurrentScreen()
    {
        ViewModel.SearchText = string.Empty;
        ViewModel.Reload(preserveSelection: false);

        PositionOnActiveScreen();

        // Start off-screen below the resting position, then animate up.
        _restingTop = Top;
        Opacity = 0;
        Top = _restingTop + SlideDistance;

        Show();
        ApplyFrostedBackdrop();
        Activate();
        SearchBox.Focus();
        Keyboard.Focus(SearchBox);

        BeginSlideIn();
    }

    /// <summary>
    /// Slides the panel up into place while fading in. Uses a decisive
    /// ease-out so the motion feels light rather than mechanical.
    /// </summary>
    private void BeginSlideIn()
    {
        var duration = TimeSpan.FromMilliseconds(260);

        var slide = new System.Windows.Media.Animation.DoubleAnimation
        {
            From = Top,
            To = _restingTop,
            Duration = duration,
            EasingFunction = new System.Windows.Media.Animation.CubicEase
            {
                EasingMode = System.Windows.Media.Animation.EasingMode.EaseOut,
            },
            FillBehavior = System.Windows.Media.Animation.FillBehavior.Stop,
        };

        slide.Completed += (_, _) =>
        {
            // Stop() reverts to the base value, so pin the resting position.
            Top = _restingTop;
        };

        BeginAnimation(TopProperty, slide);

        var fade = new System.Windows.Media.Animation.DoubleAnimation
        {
            From = 0,
            To = 1,
            Duration = TimeSpan.FromMilliseconds(160),
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
    /// Slides the panel back down and hides it. Used when dismissing without a
    /// paste so the exit matches the entry motion.
    /// </summary>
    private void SlideOutAndHide()
    {
        var slide = new System.Windows.Media.Animation.DoubleAnimation
        {
            From = Top,
            To = _restingTop + SlideDistance,
            Duration = TimeSpan.FromMilliseconds(170),
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
        var height = Height * dpi.DpiScaleY;

        // Occupy 90% of the monitor's working width, centred on that monitor.
        var widthPx = area.Width * WidthFraction;
        Left = (area.Left + (area.Width - widthPx) / 2.0) / dpi.DpiScaleX;
        Width = widthPx / dpi.DpiScaleX;

        // Sit BottomOffset above the working-area bottom edge.
        var bottomOffsetPx = BottomOffset * dpi.DpiScaleY;
        Top = (area.Bottom - height - bottomOffsetPx) / dpi.DpiScaleY;
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

            case Key.Down:
            case Key.Right:
                e.Handled = true;
                ViewModel.MoveSelection(1);
                ScrollSelectionIntoView();
                break;

            case Key.Up:
            case Key.Left:
                e.Handled = true;
                ViewModel.MoveSelection(-1);
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
        if (ViewModel.SelectedItem is not null)
        {
            ItemsList.ScrollIntoView(ViewModel.SelectedItem);
        }
    }

    private void OnItemActivated(object sender, MouseButtonEventArgs e)
    {
        ActivateSelected();
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
