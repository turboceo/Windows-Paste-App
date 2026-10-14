using System.Threading;
using System.Windows;
using System.Windows.Forms;
using WindowsPaste.Interop;
using WindowsPaste.Models;
using WindowsPaste.Services;
using WindowsPaste.ViewModels;
using WindowsPaste.Views;
using Application = System.Windows.Application;
using MessageBox = System.Windows.MessageBox;

namespace WindowsPaste;

public partial class App : Application
{
    private const string SingleInstanceMutexName = @"Local\WindowsPaste.SingleInstance";

    private Mutex? _mutex;
    private MessageWindow? _messageWindow;
    private ClipboardMonitor? _monitor;
    private HotkeyManager? _hotkeyManager;
    private ClipboardSniffer? _sniffer;
    private ClipboardRepository? _repository;
    private PasteService? _pasteService;
    private MainViewModel? _viewModel;
    private PopupHostWindow? _popup;
    private TrayIcon? _tray;
    private AppSettings _settings = new();
    private string _blobRoot = string.Empty;
    private IntPtr _lastForegroundWindow;
    private DateTime _lastHotkeyTime = DateTime.MinValue;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _mutex = new Mutex(initiallyOwned: true, SingleInstanceMutexName, out var createdNew);
        if (!createdNew)
        {
            MessageBox.Show("Windows Paste is already running. Look for it in the system tray.",
                "Windows Paste", MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown();
            return;
        }

        DispatcherUnhandledException += (_, args) =>
        {
            MessageBox.Show(args.Exception.Message, "Windows Paste error",
                MessageBoxButton.OK, MessageBoxImage.Warning);
            args.Handled = true;
        };

        try
        {
#if DEBUG
            // Seed before services open the database so the UI shows the rows.
            var seedCount = ParseSeedCount(e.Args);
            if (seedCount > 0)
            {
                DevSeed.Run(new AppPaths(), seedCount);
            }
#endif

            InitializeServices();
        }
        catch (Exception ex)
        {
            MessageBox.Show($"Failed to start Windows Paste:\n{ex.Message}",
                "Windows Paste", MessageBoxButton.OK, MessageBoxImage.Error);
            Shutdown();
            return;
        }

        if (e.Args.Any(arg => string.Equals(arg, "--webhost", StringComparison.OrdinalIgnoreCase)))
        {
#if DEBUG
            _ = RunWebHostCheckAsync();
#endif
        }
    }

#if DEBUG
    /// <summary>
    /// Opens the web-hosted popup, lets it render, then shuts down. Used to
    /// verify the WebView2 pipeline without a human watching the screen.
    /// </summary>
    private async Task RunWebHostCheckAsync()
    {
        // Give the services a moment to settle before opening the popup.
        await Task.Delay(300);

        var reportPath = System.IO.Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "WindowsPaste", "webhost.txt");
        System.IO.File.Delete(reportPath);

        ShowPopup();

        await Task.Delay(6000);
        Shutdown();
    }
#endif

#if DEBUG
    /// <summary>
    /// Reads "--seed [n]" from the command line. Defaults to 50 entries so
    /// "--seed" alone is enough for a quick layout check.
    /// </summary>
    private static int ParseSeedCount(string[] args)
    {
        for (var i = 0; i < args.Length; i++)
        {
            if (!string.Equals(args[i], "--seed", StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            if (i + 1 < args.Length && int.TryParse(args[i + 1], out var explicitCount))
            {
                return Math.Clamp(explicitCount, 1, 1000);
            }

            return 50;
        }

        return 0;
    }
#endif

    private void InitializeServices()
    {
        _settings = AppSettings.Load();
        _settings.ApplyRunAtLogin(AppSettings.ExecutablePath());

        var paths = new AppPaths();
        _sniffer = new ClipboardSniffer(paths.Blobs);
        _repository = new ClipboardRepository(paths.Database, _settings.HistoryCapacity);
        _blobRoot = paths.Blobs;

        _messageWindow = new MessageWindow();
        _messageWindow.EnsureCreated();

        _monitor = new ClipboardMonitor(_messageWindow);
        _monitor.ClipboardChanged += OnClipboardChanged;
        _monitor.Start();

        _pasteService = new PasteService(_sniffer, _monitor);

        _viewModel = new MainViewModel(_repository, _sniffer, _settings);
        _viewModel.AttachBlobsRoot(paths.Blobs);

        _tray = new TrayIcon();
        _tray.OpenRequested += (_, _) => Dispatcher.Invoke(ShowPopup);
        _tray.ClearRequested += (_, _) => Dispatcher.Invoke(ClearHistory);
        _tray.SettingsRequested += (_, _) => Dispatcher.Invoke(ShowSettings);
        _tray.ExitRequested += (_, _) => Dispatcher.Invoke(Shutdown);
        _tray.PauseToggled += (_, paused) =>
        {
            if (_monitor is not null)
            {
                _monitor.Paused = paused;
            }
        };

        _hotkeyManager = new HotkeyManager(_messageWindow);
        _hotkeyManager.Pressed += OnHotkeyPressed;
        var definition = _settings.ToHotkeyDefinition();
        if (!_hotkeyManager.Register(definition))
        {
            _tray.ShowBalloon("Hotkey unavailable",
                $"Could not register {definition.Display}; another app may be using it.");
        }

        _lastForegroundWindow = NativeMethods.GetForegroundWindow();
    }

    private void OnHotkeyPressed(object? sender, EventArgs e)
    {
        // Debounce accidental double-taps of the hotkey.
        var now = DateTime.UtcNow;
        if ((now - _lastHotkeyTime).TotalMilliseconds < 200)
        {
            return;
        }

        _lastHotkeyTime = now;

        Dispatcher.Invoke(() =>
        {
            if (_popup is { IsVisible: true })
            {
                _popup.Hide();
                return;
            }

            ShowPopup();
        });
    }

    private void OnClipboardChanged(object? sender, EventArgs e)
    {
        Dispatcher.Invoke(() =>
        {
            try
            {
                var item = _sniffer?.Capture();
                if (item is not null && _viewModel is not null)
                {
                    _viewModel.Add(item);
                }
            }
            catch (Exception)
            {
                // A transient clipboard failure should never crash the app.
            }
        });
    }

    private void ShowPopup()
    {
        if (_viewModel is null || _pasteService is null)
        {
            return;
        }

        // Remember who had focus so the paste lands in the right window.
        _lastForegroundWindow = NativeMethods.GetForegroundWindow();

        if (_popup is null)
        {
            // WebView2 startup is expensive, so the host is created once and
            // reused for the life of the process rather than per open.
            _popup = new PopupHostWindow(_viewModel, WebRootPath(), _blobRoot);
            _popup.ItemActivated += OnItemActivated;
            _popup.ItemDeleted += (_, item) => _viewModel?.Delete(item);
            _popup.ItemPinToggled += (_, item) => _viewModel?.TogglePin(item);
            _popup.Closed += (_, _) => _popup = null;
        }

        _popup.ShowForCurrentScreen();
    }

    /// <summary>Folder containing the built Vue assets, copied next to the exe.</summary>
    private static string WebRootPath()
    {
        var baseDirectory = AppContext.BaseDirectory;
        var candidate = System.IO.Path.Combine(baseDirectory, "web");

        if (System.IO.File.Exists(System.IO.Path.Combine(candidate, "index.html")))
        {
            return candidate;
        }

        // Development fallback: run from the repository so a fresh `npm run build`
        // is picked up without a .NET rebuild.
        var repository = System.IO.Path.GetFullPath(
            System.IO.Path.Combine(baseDirectory, "..", "..", "..", "..", "..", "web", "dist"));

        return System.IO.Directory.Exists(repository) ? repository : candidate;
    }

    private async void OnItemActivated(object? sender, ClipboardItem item)
    {
        if (_pasteService is null || _viewModel is null)
        {
            return;
        }

        await _pasteService.PasteAsync(item, _lastForegroundWindow);
    }

    private void ClearHistory()
    {
        var confirm = MessageBox.Show("Delete all clipboard history?", "Windows Paste",
            MessageBoxButton.YesNo, MessageBoxImage.Question);

        if (confirm == MessageBoxResult.Yes)
        {
            _viewModel?.ClearAll();
        }
    }

    private void ShowSettings()
    {
        if (_hotkeyManager is null)
        {
            return;
        }

        var window = new SettingsWindow(_settings);
        var accepted = window.ShowDialog();

        if (accepted != true)
        {
            return;
        }

        if (window.ResultHotkey is { } definition)
        {
            if (!_hotkeyManager.Register(definition))
            {
                _tray?.ShowBalloon("Hotkey unavailable",
                    $"{definition.Display} is already used by another application.");
            }
        }

        if (_viewModel is not null)
        {
            _viewModel.Reload();
            _viewModel.RefreshHotkeyLabel();
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _hotkeyManager?.Dispose();
        _monitor?.Dispose();
        _messageWindow?.Dispose();
        _repository?.Dispose();
        _tray?.Dispose();
        _mutex?.Dispose();

        base.OnExit(e);
    }
}
