using Microsoft.Data.Sqlite;
using WindowsPaste.Models;

namespace WindowsPaste.Services;

/// <summary>
/// SQLite-backed clipboard history with de-duplication and a rolling capacity cap.
/// Not thread-safe; all access happens on the UI thread dispatcher.
/// </summary>
public sealed class ClipboardRepository : IDisposable
{
    private readonly SqliteConnection _connection;
    private readonly int _capacity;

    public ClipboardRepository(string databasePath, int capacity = 500)
    {
        _capacity = capacity;

        var builder = new SqliteConnectionStringBuilder
        {
            DataSource = databasePath,
            Mode = SqliteOpenMode.ReadWriteCreate,
        };

        _connection = new SqliteConnection(builder.ToString());
        _connection.Open();
        Initialize();
    }

    private void Initialize()
    {
        using var command = _connection.CreateCommand();
        command.CommandText = """
            CREATE TABLE IF NOT EXISTS ClipboardItems (
                Id        INTEGER PRIMARY KEY AUTOINCREMENT,
                Type      INTEGER NOT NULL,
                Content   TEXT    NOT NULL,
                BlobPath  TEXT    NULL,
                Hash      TEXT    NOT NULL,
                CreatedAt TEXT    NOT NULL,
                Pinned    INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS IX_ClipboardItems_Hash ON ClipboardItems(Hash);
            CREATE INDEX IF NOT EXISTS IX_ClipboardItems_CreatedAt ON ClipboardItems(CreatedAt DESC);
            """;
        command.ExecuteNonQuery();
    }

    /// <summary>
    /// Inserts a new item, skipping it when the newest entry has the same hash
    /// (avoids duplicate rows from hotkey re-pastes).
    /// </summary>
    public ClipboardItem? Add(ClipboardItem item)
    {
        var newest = GetMostRecent();
        if (newest is not null && newest.Hash == item.Hash)
        {
            return null;
        }

        using (var command = _connection.CreateCommand())
        {
            command.CommandText = """
                INSERT INTO ClipboardItems (Type, Content, BlobPath, Hash, CreatedAt, Pinned)
                VALUES ($type, $content, $blob, $hash, $created, 0);
                SELECT last_insert_rowid();
                """;
            command.Parameters.AddWithValue("$type", (int)item.Type);
            command.Parameters.AddWithValue("$content", item.Content);
            command.Parameters.AddWithValue("$blob", (object?)item.BlobPath ?? DBNull.Value);
            command.Parameters.AddWithValue("$hash", item.Hash);
            command.Parameters.AddWithValue("$created", item.CreatedAt.ToString("o"));

            item.Id = (long)(command.ExecuteScalar() ?? 0L);
        }

        Trim();
        return item;
    }

    public IReadOnlyList<ClipboardItem> GetAll()
    {
        using var command = _connection.CreateCommand();
        command.CommandText = """
            SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned
            FROM ClipboardItems
            ORDER BY Pinned DESC, CreatedAt DESC, Id DESC
            """;
        return ReadAll(command);
    }

    public IReadOnlyList<ClipboardItem> Search(string term)
    {
        if (string.IsNullOrWhiteSpace(term))
        {
            return GetAll();
        }

        using var command = _connection.CreateCommand();
        command.CommandText = """
            SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned
            FROM ClipboardItems
            WHERE Content LIKE $term
            ORDER BY Pinned DESC, CreatedAt DESC, Id DESC
            LIMIT 200
            """;
        command.Parameters.AddWithValue("$term", "%" + term.Trim() + "%");
        return ReadAll(command);
    }

    public ClipboardItem? GetMostRecent()
    {
        using var command = _connection.CreateCommand();
        command.CommandText = """
            SELECT Id, Type, Content, BlobPath, Hash, CreatedAt, Pinned
            FROM ClipboardItems
            ORDER BY CreatedAt DESC, Id DESC
            LIMIT 1
            """;
        return ReadAll(command).FirstOrDefault();
    }

    public int GetTotalCount()
    {
        using var command = _connection.CreateCommand();
        command.CommandText = "SELECT COUNT(*) FROM ClipboardItems";
        return Convert.ToInt32(command.ExecuteScalar() ?? 0);
    }

    public void SetPinned(long id, bool pinned)
    {
        using var command = _connection.CreateCommand();
        command.CommandText = "UPDATE ClipboardItems SET Pinned = $pinned WHERE Id = $id";
        command.Parameters.AddWithValue("$pinned", pinned ? 1 : 0);
        command.Parameters.AddWithValue("$id", id);
        command.ExecuteNonQuery();
    }

    public void Delete(long id)
    {
        using var command = _connection.CreateCommand();
        command.CommandText = "DELETE FROM ClipboardItems WHERE Id = $id";
        command.Parameters.AddWithValue("$id", id);
        command.ExecuteNonQuery();
    }

    public IReadOnlyList<string> Clear()
    {
        var blobs = GetBlobPaths();

        using (var command = _connection.CreateCommand())
        {
            command.CommandText = "DELETE FROM ClipboardItems";
            command.ExecuteNonQuery();
        }

        return blobs;
    }

    private List<string> GetBlobPaths()
    {
        using var command = _connection.CreateCommand();
        command.CommandText = "SELECT BlobPath FROM ClipboardItems WHERE BlobPath IS NOT NULL";

        var result = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read())
        {
            result.Add(reader.GetString(0));
        }

        return result;
    }

    private void Trim()
    {
        // Keep pinned items regardless of capacity; only evict the oldest unpinned rows.
        using var command = _connection.CreateCommand();
        command.CommandText = """
            SELECT Id, BlobPath FROM ClipboardItems
            WHERE Pinned = 0
            ORDER BY CreatedAt DESC, Id DESC
            LIMIT -1 OFFSET $keep
            """;
        command.Parameters.AddWithValue("$keep", _capacity);

        var stale = new List<(long Id, string? Blob)>();
        using (var reader = command.ExecuteReader())
        {
            while (reader.Read())
            {
                stale.Add((reader.GetInt64(0), reader.IsDBNull(1) ? null : reader.GetString(1)));
            }
        }

        if (stale.Count == 0)
        {
            return;
        }

        foreach (var (id, _) in stale)
        {
            Delete(id);
        }
    }

    private static List<ClipboardItem> ReadAll(SqliteCommand command)
    {
        var items = new List<ClipboardItem>();
        using var reader = command.ExecuteReader();
        while (reader.Read())
        {
            items.Add(new ClipboardItem
            {
                Id = reader.GetInt64(0),
                Type = (ClipboardItemType)reader.GetInt32(1),
                Content = reader.GetString(2),
                BlobPath = reader.IsDBNull(3) ? null : reader.GetString(3),
                Hash = reader.GetString(4),
                CreatedAt = DateTime.Parse(reader.GetString(5), null, System.Globalization.DateTimeStyles.RoundtripKind),
                Pinned = reader.GetInt32(6) != 0,
            });
        }

        return items;
    }

    public void Dispose() => _connection.Dispose();
}
