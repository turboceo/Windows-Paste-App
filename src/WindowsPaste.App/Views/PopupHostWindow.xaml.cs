using System.Text.Json;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using Microsoft.Web.WebView2.Core;
using WindowsPaste.Models;
using WindowsPaste.Services;
using WindowsPaste.ViewModels;
using MessageBox = System.Windows.MessageBox;

namespace WindowsPaste.Views;

/// <summary>
/// Native shell for the web-based popup. Keeps the parts that must stay native
/// (window positioning, acrylic backdrop, animation, keyboard handling) and
/// delegates the visible UI to a Vue page served from the bundled dist folder.
/// </summary>
public partial class PopupHostWindow : Window
{
    /// <summary>Panel occupies these fractions of the monitor's working area.</summary>
    private const double HeightFraction = 0.6;

    /// <summary>Virtual host that the bundled web assets are served from.</summary>
    private const string AppHostName = "app.local";

    private readonly MainViewModel _viewModel;
    private readonly string _webRoot;
    private readonly string _blobRoot;

    private bool _allowDeactivateClose = true;
    private bool _webReady;
    private bool _pendingShow;

    public PopupHostWindow(MainViewModel viewModel, string webRoot, string blobRoot)
    {
        InitializeComponent();

        _viewModel = viewModel;
        _webRoot = webRoot;
        _blobRoot = blobRoot;

        _viewModel.PropertyChanged += OnViewModelPropertyChanged;
        _viewModel.SearchChanged += OnSearchChanged;

        StateChanged += (_, _) => PushWindowState();

        Web.CoreWebView2InitializationCompleted += OnWebInitialized;
        Loaded += OnLoaded;
    }

    public event EventHandler<ClipboardItem>? ItemActivated;
    public event EventHandler<ClipboardItem>? ItemDeleted;
    public event EventHandler<ClipboardItem>? ItemPinToggled;

    /// <summary>Opens the popup on the monitor that owns the cursor.</summary>
    public void ShowForCurrentScreen()
    {
        if (WindowState == WindowState.Minimized || WindowState == WindowState.Maximized)
        {
            WindowState = WindowState.Normal;
        }

        _viewModel.SearchText = string.Empty;
        _viewModel.Reload(preserveSelection: false);

        PositionOnActiveScreen();
        Opacity = 1;

        Show();
        ApplyWindowFrame();
        Activate();

        if (_webReady)
        {
            FocusWebSearch();
        }
        else
        {
            _pendingShow = true;
        }
    }

    private async void OnLoaded(object sender, RoutedEventArgs e)
    {
        try
        {
            var userDataFolder = System.IO.Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "WindowsPaste", "WebView2");
            System.IO.Directory.CreateDirectory(userDataFolder);

            var environment = await CoreWebView2Environment.CreateAsync(
                browserExecutableFolder: null, userDataFolder: userDataFolder);
            await Web.EnsureCoreWebView2Async(environment);

            var core = Web.CoreWebView2;

            // Chrome-less experience: no default menu, no browser accelerators,
            // no status bar. Keyboard handling lives in WPF (see PreviewKeyDown).
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.Settings.AreBrowserAcceleratorKeysEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            core.Settings.IsZoomControlEnabled = false;
            core.Settings.AreDevToolsEnabled = System.Diagnostics.Debugger.IsAttached;

            // Serve the bundled SPA from a virtual HTTPS origin, which avoids the
            // module/CORS restrictions of file:// URLs. Blobs get their own host
            // so <img> tags can point straight at the image files.
            core.SetVirtualHostNameToFolderMapping(
                AppHostName, _webRoot, CoreWebView2HostResourceAccessKind.DenyCors);
            core.SetVirtualHostNameToFolderMapping(
                "blobs.local", _blobRoot, CoreWebView2HostResourceAccessKind.DenyCors);

            core.WebMessageReceived += OnWebMessageReceived;
            core.NavigationCompleted += OnNavigationCompleted;
            core.ProcessFailed += OnProcessFailed;

            core.Navigate($"https://{AppHostName}/index.html");
        }
        catch (Exception ex)
        {
            MessageBox.Show(
                $"Failed to start the popup UI:\n{ex.Message}\n\nWeb root: {_webRoot}",
                "Windows Paste", MessageBoxButton.OK, MessageBoxImage.Error);
            Hide();
        }
    }

    private void OnWebInitialized(object? sender, CoreWebView2InitializationCompletedEventArgs e)
    {
        if (!e.IsSuccess)
        {
            MessageBox.Show(
                $"WebView2 initialization failed:\n{e.InitializationException?.Message}",
                "Windows Paste", MessageBoxButton.OK, MessageBoxImage.Error);
        }
    }

    private void OnProcessFailed(object? sender, CoreWebView2ProcessFailedEventArgs e)
    {
        WriteDiagnostic($"process failed: {e.ProcessFailedKind}, reason: {e.Reason}");
        if (e.ProcessFailedKind == CoreWebView2ProcessFailedKind.GpuProcessExited)
        {
            return;
        }

        try { Web.Reload(); } catch { }
    }

    private void OnNavigationCompleted(object? sender, CoreWebView2NavigationCompletedEventArgs e)
    {
        if (!e.IsSuccess)
        {
            WriteDiagnostic($"navigation failed: {e.WebErrorStatus}");
            return;
        }

        _webReady = true;

        if (_pendingShow)
        {
            _pendingShow = false;
            FocusWebSearch();
        }

        _ = WriteRenderDiagnosticAsync();
    }

    /// <summary>
    /// Dumps what the page actually rendered, so the host can be verified
    /// without a human looking at the screen.
    /// </summary>
    private async Task WriteRenderDiagnosticAsync()
    {
        try
        {
            // Give Vue a moment to mount and the host to push the first payload.
            await Task.Delay(500);

            const string script = """
                (() => {
                  const cards = document.querySelectorAll('.card');
                  const panel = document.querySelector('.panel');
                  const grid = document.querySelector('.visible-grid') || document.querySelector('.grid');
                  const spacer = document.querySelector('.virtual-spacer');
                  const container = document.querySelector('.viewport-container');
                  const selected = document.querySelector('.card.selected');
                  const search = document.querySelector('.search-input');
                  const titlebar = document.querySelector('.titlebar');
                  const controlBtns = document.querySelectorAll('.window-control-btn');
                  const bar = grid ? grid.getBoundingClientRect() : null;
                  const style = panel ? getComputedStyle(panel) : null;
                  return JSON.stringify({
                    cardCount: cards.length,
                    hasTitleBar: !!titlebar,
                    controlButtonCount: controlBtns.length,
                    virtualSpacerHeight: spacer ? Math.round(spacer.offsetHeight) : 0,
                    containerHeight: container ? Math.round(container.clientHeight) : 0,
                    gridColumns: grid ? getComputedStyle(grid).gridTemplateColumns.split(' ').length : 0,
                    selectedId: selected ? (selected.textContent || '').slice(0, 24) : null,
                    hasSearch: !!search,
                    hasToolbar: !!(document.querySelector('.toolbar')),
                    panelBg: style ? style.backgroundColor : null,
                    panelRadius: style ? style.borderRadius : null,
                    gridWidth: bar ? Math.round(bar.width) : 0,
                    bodyText: (document.body.innerText || '').slice(0, 120)
                  });
                })()
                """;

            var result = await Web.CoreWebView2.ExecuteScriptAsync(script);
            WriteDiagnostic($"render: {result}");
            WriteDiagnostic($"geometry: window={Width:F0}x{Height:F0} at {Left:F0},{Top:F0} opacity={Opacity:F2}");

            await VerifyInteractionsAsync();
        }
        catch (Exception ex)
        {
            WriteDiagnostic($"diagnostic failed: {ex.Message}");
        }
    }

    /// <summary>
    /// Drives the page's own controls and checks the host state changes, so the
    /// full round trip is verified rather than just the initial render.
    /// </summary>
    private async Task VerifyInteractionsAsync()
    {
        const string searchScript = """
            (() => {
              const input = document.querySelector('.search-input');
              input.value = 'TODO';
              input.dispatchEvent(new Event('input', { bubbles: true }));
              return 'typed';
            })()
            """;

        await Web.CoreWebView2.ExecuteScriptAsync(searchScript);
        await Task.Delay(350);
        WriteDiagnostic($"after search 'TODO' -> vm.Items={_viewModel.Items.Count}, vm.SearchText='{_viewModel.SearchText}'");

        const string clearScript = """
            (() => {
              const input = document.querySelector('.search-input');
              input.value = '';
              input.dispatchEvent(new Event('input', { bubbles: true }));
              return 'cleared';
            })()
            """;

        await Web.CoreWebView2.ExecuteScriptAsync(clearScript);
        await Task.Delay(350);
        WriteDiagnostic($"after clear -> vm.Items={_viewModel.Items.Count}");

        // Next button must advance the host selection by one.
        var before = _viewModel.SelectedItem?.Id;
        await Web.CoreWebView2.ExecuteScriptAsync(
            "document.querySelectorAll('.nav-btn')[1].click();");
        await Task.Delay(300);
        WriteDiagnostic($"after next -> selection {before} -> {_viewModel.SelectedItem?.Id}");

        // Pin button on the first card must flip the pinned flag.
        var target = _viewModel.Items.FirstOrDefault();
        if (target is not null)
        {
            var targetId = target.Id;
            var wasPinned = target.Pinned;
            await Web.CoreWebView2.ExecuteScriptAsync(
                "document.querySelector('.card .pin-btn').click();");
            await Task.Delay(350);
            var nowPinned = _viewModel.Items.FirstOrDefault(i => i.Id == targetId)?.Pinned;
            WriteDiagnostic($"after pin click -> id {targetId}: {wasPinned} -> {nowPinned}");
        }

        // Clicking a card must select it without pasting.
        await Web.CoreWebView2.ExecuteScriptAsync(
            "document.querySelectorAll('.card')[2].dispatchEvent(new MouseEvent('click', { bubbles: true }));");
        await Task.Delay(300);
        WriteDiagnostic($"after card click -> selection={_viewModel.SelectedItem?.Id}");

        // Clicking the maximize/restore button in the titlebar must toggle WindowState.
        var stateBefore = WindowState;
        await Web.CoreWebView2.ExecuteScriptAsync(
            "document.querySelectorAll('.window-control-btn')[1].click();");
        await Task.Delay(350);
        WriteDiagnostic($"after maximize click -> WindowState: {stateBefore} -> {WindowState}");

        // Restore back to normal
        await Web.CoreWebView2.ExecuteScriptAsync(
            "document.querySelectorAll('.window-control-btn')[1].click();");
        await Task.Delay(350);
        WriteDiagnostic($"after restore click -> WindowState: {WindowState}");
    }

    private static void WriteDiagnostic(string line)
    {
        try
        {
            var path = System.IO.Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "WindowsPaste", "webhost.txt");
            System.IO.File.AppendAllText(path, line + Environment.NewLine);
        }
        catch (Exception)
        {
            // Diagnostics must never break the app.
        }
    }

    /// <summary>Focuses the search box inside the web page.</summary>
    private void FocusWebSearch()
    {
        Web.CoreWebView2?.ExecuteScriptAsync(
            "document.querySelector('.search-input')?.focus();");
    }

    // --- Host -> web ---------------------------------------------------------

    private void OnViewModelPropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(MainViewModel.Items)
            or nameof(MainViewModel.IsEmpty)
            or nameof(MainViewModel.TotalCount)
            or nameof(MainViewModel.SelectedItem)
            or nameof(MainViewModel.CanSelectPrevious)
            or nameof(MainViewModel.CanSelectNext))
        {
            PushItems();
            PushSelection();
        }
    }

    private void OnSearchChanged(object? sender, EventArgs e)
    {
        PushItems();
        PushSelection();
    }

    private void PushItems()
    {
        var payload = new
        {
            type = "items",
            items = _viewModel.Items.Select(ToDto).ToArray(),
            isEmpty = _viewModel.IsEmpty,
            totalCount = _viewModel.TotalCount,
        };

        WriteDiagnostic($"-> items count={_viewModel.Items.Count}");
        Push(payload);
    }

    private void PushSelection()
    {
        var payload = new
        {
            type = "selection",
            itemId = _viewModel.SelectedItem?.Id,
            canPrev = _viewModel.CanSelectPrevious,
            canNext = _viewModel.CanSelectNext,
        };

        Push(payload);
    }

    private void PushWindowState()
    {
        Push(new
        {
            type = "windowState",
            isMaximized = WindowState == WindowState.Maximized,
        });
    }

    private object ToDto(ClipboardItem item)
    {
        return new
        {
            id = item.Id,
            type = item.Type.ToString(),
            content = item.Content,
            preview = item.Preview,
            imageUrl = item.Type == ClipboardItemType.Image && !string.IsNullOrEmpty(item.BlobPath)
                ? $"https://blobs.local/{item.BlobPath}"
                : null,
            createdAtLabel = item.CreatedAtLabel,
            pinned = item.Pinned,
        };
    }

    private void Push(object payload)
    {
        if (!_webReady && !_pendingShow)
        {
            // Nothing is listening yet; the page asks for state on 'ready'.
        }

        Web.CoreWebView2?.PostWebMessageAsJson(JsonSerializer.Serialize(payload));
    }

    // --- Web -> host ---------------------------------------------------------

    private void OnWebMessageReceived(object? sender, CoreWebView2WebMessageReceivedEventArgs e)
    {
        using var document = JsonDocument.Parse(e.WebMessageAsJson);
        var root = document.RootElement;

        if (!root.TryGetProperty("type", out var typeProperty))
        {
            return;
        }

        var messageType = typeProperty.GetString();
        WriteDiagnostic($"<- {messageType}");

        switch (messageType)
        {
            case "ready":
                PushItems();
                PushSelection();
                PushWindowState();
                Push(new { type = "hotkey", label = _viewModel.HotkeyLabel });
                FocusWebSearch();
                break;

            case "search":
                _viewModel.SearchText = root.TryGetProperty("text", out var text)
                    ? text.GetString() ?? string.Empty
                    : string.Empty;
                break;

            case "select":
                if (TryGetItem(root, out var toSelect))
                {
                    _viewModel.SelectedItem = toSelect;
                }
                break;

            case "move":
                if (root.TryGetProperty("delta", out var delta))
                {
                    _viewModel.MoveSelection(delta.GetInt32());
                }
                break;

            case "paste":
                if (TryGetItem(root, out var toPaste))
                {
                    _viewModel.SelectedItem = toPaste;
                    ActivateSelected();
                }
                break;

            case "delete":
                if (TryGetItem(root, out var toDelete))
                {
                    ItemDeleted?.Invoke(this, toDelete);
                }
                break;

            case "togglePin":
                if (TryGetItem(root, out var toPin))
                {
                    ItemPinToggled?.Invoke(this, toPin);
                }
                break;

            case "columns":
                if (root.TryGetProperty("columns", out var columns))
                {
                    _viewModel.ColumnsPerRow = columns.GetInt32();
                }
                break;

            case "dragMove":
                if (WindowState == WindowState.Normal)
                {
                    try { DragMove(); } catch { }
                }
                break;

            case "minimize":
                WindowState = WindowState.Minimized;
                break;

            case "maximize":
            case "toggleMaximize":
                ToggleMaximize();
                break;

            case "close":
                SlideOutAndHide();
                break;
        }
    }

    private void ToggleMaximize()
    {
        WindowState = WindowState == WindowState.Maximized
            ? WindowState.Normal
            : WindowState.Maximized;
        PushWindowState();
    }

    private bool TryGetItem(JsonElement root, out ClipboardItem item)
    {
        item = null!;

        if (!root.TryGetProperty("itemId", out var idProperty))
        {
            return false;
        }

        var id = idProperty.GetInt64();
        var match = _viewModel.Items.FirstOrDefault(candidate => candidate.Id == id);
        if (match is null)
        {
            return false;
        }

        item = match;
        return true;
    }

    private void ActivateSelected()
    {
        var item = _viewModel.SelectedItem;
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

    // --- Keyboard ------------------------------------------------------------

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
                _viewModel.MoveSelection(1);
                break;

            case Key.Left:
                e.Handled = true;
                _viewModel.MoveSelection(-1);
                break;

            case Key.Down:
                e.Handled = true;
                _viewModel.MoveSelectionByRow(1);
                break;

            case Key.Up:
                e.Handled = true;
                _viewModel.MoveSelectionByRow(-1);
                break;

            case Key.Enter:
                e.Handled = true;
                ActivateSelected();
                break;

            case Key.Delete:
                e.Handled = true;
                if (_viewModel.SelectedItem is { } toDelete)
                {
                    ItemDeleted?.Invoke(this, toDelete);
                }

                break;

            case Key.P when Keyboard.Modifiers == ModifierKeys.Control:
                e.Handled = true;
                if (_viewModel.SelectedItem is { } toPin)
                {
                    ItemPinToggled?.Invoke(this, toPin);
                }

                break;
        }
    }

    // --- Window behaviour ----------------------------------------------------

    private void OnDeactivated(object? sender, EventArgs e)
    {
        if (_allowDeactivateClose)
        {
            SlideOutAndHide();
        }
    }

    private void PositionOnActiveScreen()
    {
        var cursor = System.Windows.Forms.Cursor.Position;
        var screen = System.Windows.Forms.Screen.FromPoint(cursor);
        var area = screen.WorkingArea;

        var dpi = VisualTreeHelper.GetDpi(this);

        // Comfortable centered width: 78% of working area, bounded nicely
        var targetWidthPx = Math.Max(1000, Math.Min(1520, area.Width * 0.78));
        var widthPx = Math.Min(targetWidthPx, area.Width * 0.95);
        var heightPx = area.Height * HeightFraction;

        Width = widthPx / dpi.DpiScaleX;
        Height = heightPx / dpi.DpiScaleY;

        Left = (area.Left + (area.Width - widthPx) / 2.0) / dpi.DpiScaleX;
        Top = (area.Top + (area.Height - heightPx) / 2.0) / dpi.DpiScaleY;
    }

    private void ApplyWindowFrame()
    {
        var handle = new System.Windows.Interop.WindowInteropHelper(this).Handle;
        WindowBlur.ApplySquareCorners(handle);
    }

    private void SlideOutAndHide()
    {
        Hide();
    }
}
