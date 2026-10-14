using System.Drawing;
using System.Windows.Forms;
using WindowsPaste.Services;

namespace WindowsPaste.Views;

/// <summary>System tray presence with a context menu for quick control.</summary>
public sealed class TrayIcon : IDisposable
{
    private readonly NotifyIcon _notifyIcon;
    private bool _disposed;

    public TrayIcon()
    {
        var menu = new ContextMenuStrip();
        menu.Items.Add("Open history", null, (_, _) => OpenRequested?.Invoke(this, EventArgs.Empty));
        menu.Items.Add(new ToolStripSeparator());

        _pauseItem = new ToolStripMenuItem("Pause capture") { CheckOnClick = true };
        _pauseItem.CheckedChanged += (_, _) => PauseToggled?.Invoke(this, _pauseItem.Checked);
        menu.Items.Add(_pauseItem);

        menu.Items.Add("Clear history", null, (_, _) => ClearRequested?.Invoke(this, EventArgs.Empty));
        menu.Items.Add("Settings...", null, (_, _) => SettingsRequested?.Invoke(this, EventArgs.Empty));
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add("Exit", null, (_, _) => ExitRequested?.Invoke(this, EventArgs.Empty));

        _notifyIcon = new NotifyIcon
        {
            Icon = CreateIcon(),
            Text = "Windows Paste",
            Visible = true,
            ContextMenuStrip = menu,
        };

        _notifyIcon.DoubleClick += (_, _) => OpenRequested?.Invoke(this, EventArgs.Empty);
    }

    private readonly ToolStripMenuItem _pauseItem;

    public event EventHandler? OpenRequested;
    public event EventHandler<bool>? PauseToggled;
    public event EventHandler? ClearRequested;
    public event EventHandler? SettingsRequested;
    public event EventHandler? ExitRequested;

    public bool IsPaused
    {
        get => _pauseItem.Checked;
        set => _pauseItem.Checked = value;
    }

    public void ShowBalloon(string title, string message)
    {
        try
        {
            _notifyIcon.ShowBalloonTip(3000, title, message, ToolTipIcon.Info);
        }
        catch (Exception)
        {
            // Balloons are cosmetic; ignore failures.
        }
    }

    private static Icon CreateIcon()
    {
        // Draw a simple clipboard glyph at runtime so we ship no binary assets.
        using var bitmap = new Bitmap(32, 32);
        using (var graphics = Graphics.FromImage(bitmap))
        {
            graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
            graphics.Clear(Color.Transparent);

            using var boardBrush = new SolidBrush(Color.FromArgb(255, 58, 120, 255));
            using var paperBrush = new SolidBrush(Color.White);
            using var clipBrush = new SolidBrush(Color.FromArgb(255, 210, 225, 255));

            graphics.FillRectangle(paperBrush, 7, 6, 18, 22);
            graphics.FillRectangle(boardBrush, 5, 4, 22, 6);
            graphics.FillRectangle(clipBrush, 12, 2, 8, 5);

            using var linePen = new Pen(Color.FromArgb(255, 150, 170, 210), 1.6f);
            graphics.DrawLine(linePen, 10, 15, 22, 15);
            graphics.DrawLine(linePen, 10, 19, 22, 19);
            graphics.DrawLine(linePen, 10, 23, 18, 23);
        }

        return Icon.FromHandle(bitmap.GetHicon());
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        _notifyIcon.Visible = false;
        _notifyIcon.Dispose();
    }
}
