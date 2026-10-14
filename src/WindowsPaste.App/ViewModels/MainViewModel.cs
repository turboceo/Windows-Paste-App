using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using WindowsPaste.Models;
using WindowsPaste.Services;

namespace WindowsPaste.ViewModels;

public sealed class MainViewModel : INotifyPropertyChanged
{
    private readonly ClipboardRepository _repository;
    private readonly ClipboardSniffer _sniffer;
    private string _searchText = string.Empty;
    private ClipboardItem? _selectedItem;
    private bool _isEmpty = true;

    public MainViewModel(ClipboardRepository repository, ClipboardSniffer sniffer, AppSettings settings)
    {
        _repository = repository;
        _sniffer = sniffer;
        Settings = settings;
        Items = new ObservableCollection<ClipboardItem>();
        Reload();
    }

    public ObservableCollection<ClipboardItem> Items { get; }

    public AppSettings Settings { get; }

    public string HotkeyLabel => Settings.HotkeyDisplay;

    public void RefreshHotkeyLabel()
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(HotkeyLabel)));

    public string SearchText
    {
        get => _searchText;
        set
        {
            if (SetField(ref _searchText, value))
            {
                // Always land on the best match and reset the strip's scroll offset.
                Reload(preserveSelection: false);
                SearchChanged?.Invoke(this, EventArgs.Empty);
            }
        }
    }

    /// <summary>Raised after a search changes the list, so the view can reset scrolling.</summary>
    public event EventHandler? SearchChanged;

    public ClipboardItem? SelectedItem
    {
        get => _selectedItem;
        set => SetField(ref _selectedItem, value);
    }

    public bool IsEmpty
    {
        get => _isEmpty;
        private set => SetField(ref _isEmpty, value);
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    public void Reload()
    {
        Reload(preserveSelection: true);
    }

    /// <summary>
    /// Rebuilds the list. When <paramref name="preserveSelection"/> is false the
    /// newest entry is selected, which is what opening the shelf should do.
    /// </summary>
    public void Reload(bool preserveSelection)
    {
        var selectedId = preserveSelection ? SelectedItem?.Id : null;

        var results = string.IsNullOrWhiteSpace(SearchText)
            ? _repository.GetAll()
            : _repository.Search(SearchText);

        Items.Clear();
        foreach (var item in results)
        {
            Items.Add(item);
        }

        IsEmpty = Items.Count == 0;

        SelectedItem = selectedId is null
            ? Items.FirstOrDefault()
            : Items.FirstOrDefault(i => i.Id == selectedId) ?? Items.FirstOrDefault();
    }

    public ClipboardItem? Add(ClipboardItem item)
    {
        var added = _repository.Add(item);
        if (added is not null)
        {
            Reload();
        }

        return added;
    }

    public void TogglePin(ClipboardItem item)
    {
        _repository.SetPinned(item.Id, !item.Pinned);
        Reload();
    }

    public void Delete(ClipboardItem item)
    {
        _repository.Delete(item.Id);
        DeleteBlob(item);
        Reload();
    }

    public void ClearAll()
    {
        foreach (var blob in _repository.Clear())
        {
            DeleteBlobFile(blob);
        }

        Reload();
    }

    public void MoveSelection(int delta)
    {
        if (Items.Count == 0)
        {
            return;
        }

        var index = SelectedItem is null ? -1 : Items.IndexOf(SelectedItem);
        var next = Math.Clamp(index + delta, 0, Items.Count - 1);
        SelectedItem = Items[next];
    }

    public ImageSource? GetThumbnail(ClipboardItem item)
    {
        if (item.Type != ClipboardItemType.Image)
        {
            return null;
        }

        var path = _sniffer.ResolveBlobPath(item);
        if (!System.IO.File.Exists(path))
        {
            return null;
        }

        try
        {
            var bitmap = new BitmapImage();
            bitmap.BeginInit();
            bitmap.CacheOption = BitmapCacheOption.OnLoad;
            bitmap.DecodePixelWidth = 120;
            bitmap.UriSource = new Uri(path);
            bitmap.EndInit();
            bitmap.Freeze();
            return bitmap;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private void DeleteBlob(ClipboardItem item)
    {
        if (item.BlobPath is not null)
        {
            DeleteBlobFile(item.BlobPath);
        }
    }

    private void DeleteBlobFile(string blobName)
    {
        if (string.IsNullOrEmpty(_blobsRoot))
        {
            return;
        }

        try
        {
            var path = System.IO.Path.Combine(_blobsRoot, blobName);
            if (System.IO.File.Exists(path))
            {
                System.IO.File.Delete(path);
            }
        }
        catch (Exception)
        {
            // Best-effort cleanup; orphaned blobs are harmless.
        }
    }

    private string _blobsRoot = string.Empty;

    public void AttachBlobsRoot(string root) => _blobsRoot = root;

    private bool SetField<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }

        field = value;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        return true;
    }
}
