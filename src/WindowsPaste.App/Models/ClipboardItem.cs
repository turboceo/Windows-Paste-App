namespace WindowsPaste.Models;

public enum ClipboardItemType
{
    Text = 0,
    Image = 1,
    Files = 2,
}

public sealed class ClipboardItem
{
    public long Id { get; set; }

    public ClipboardItemType Type { get; set; }

    /// <summary>Display / search text. For files this is the newline-joined path list.</summary>
    public string Content { get; set; } = string.Empty;

    /// <summary>Relative blob file name (images only). Stored under the blobs folder.</summary>
    public string? BlobPath { get; set; }

    /// <summary>Content hash used for de-duplication.</summary>
    public string Hash { get; set; } = string.Empty;

    public DateTime CreatedAt { get; set; }

    public bool Pinned { get; set; }

    public string TypeLabel => Type switch
    {
        ClipboardItemType.Image => "Image",
        ClipboardItemType.Files => "Files",
        _ => "Text",
    };

    /// <summary>Short single-line preview for the list view.</summary>
    public string Preview
    {
        get
        {
            if (Type == ClipboardItemType.Image)
            {
                return string.IsNullOrEmpty(Content) ? "Image" : Content;
            }

            var flat = Content.Replace('\r', ' ').Replace('\n', ' ').Trim();
            return flat.Length <= 200 ? flat : flat[..200];
        }
    }

    public string CreatedAtLabel => CreatedAt.ToString("yyyy-MM-dd HH:mm");
}
