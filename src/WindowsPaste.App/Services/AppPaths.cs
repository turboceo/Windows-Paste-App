using System.IO;

namespace WindowsPaste.Services;

/// <summary>Resolves the on-disk locations the app uses under %LOCALAPPDATA%.</summary>
public sealed class AppPaths
{
    public AppPaths()
    {
        Root = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "WindowsPaste");

        Blobs = Path.Combine(Root, "blobs");
        Database = Path.Combine(Root, "history.db");

        Directory.CreateDirectory(Root);
        Directory.CreateDirectory(Blobs);
    }

    public string Root { get; }
    public string Blobs { get; }
    public string Database { get; }
}
