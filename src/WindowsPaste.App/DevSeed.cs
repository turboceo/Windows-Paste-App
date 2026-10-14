using WindowsPaste.Models;
using WindowsPaste.Services;

namespace WindowsPaste;

/// <summary>
/// Development helper that fills the history database with sample entries so the
/// layout can be exercised with a realistic amount of content. Enabled with the
/// --seed argument.
/// </summary>
internal static class DevSeed
{
    private static readonly string[] SampleTexts =
    {
        "https://github.com/dotnet/wpf",
        "dotnet build -c Release -r win-x64 --self-contained true",
        "SELECT Id, Type, Content FROM ClipboardItems ORDER BY CreatedAt DESC",
        "The quick brown fox jumps over the lazy dog",
        "git rebase --interactive HEAD~3",
        "npm run build && npm test",
        "user@example.com",
        "C:\\Users\\Pyzl\\AppData\\Local\\WindowsPaste\\history.db",
        "Meeting notes: review the Q4 roadmap and finalize the copy for the launch page.",
        "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
        "docker compose up -d --build",
        "Remember to water the plants before the trip.",
        "9384-1200-5567-2291",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleKeyForSeedingOnly",
        "Rust ownership prevents data races at compile time.",
        "1 + 1 = 2",
        "Windows Terminal > Settings > Profiles",
        "curl -sSL https://example.com/install.sh | bash",
        "TODO: refactor the clipboard repository before v1.0",
        "{\"name\":\"windows-paste\",\"version\":\"0.1.0\"}",
        "password: correct-horse-battery-staple",
        "Press Ctrl+Shift+V to open the clipboard history panel.",
        "SELECT COUNT(*) FROM ClipboardItems WHERE Pinned = 1",
        "好记性不如烂笔头",
        "The frosted panel uses acrylic blur behind the window.",
        "export PATH=\"$HOME/.local/bin:$PATH\"",
        "System.Windows.Clipboard.SetText(content)",
        "TODO: add OCR support for images",
        "192.168.1.1",
        "git commit -m \"fix: keep hover box size stable\"",
    };

    private static readonly string[] SampleFileSets =
    {
        "C:\\Users\\Pyzl\\Documents\\report.pdf",
        "C:\\Users\\Pyzl\\Pictures\\screenshot.png",
        "D:\\Projects\\assets\\logo.svg",
    };

    public static void Run(AppPaths paths, int count)
    {
        // Wipe existing rows so the seed is deterministic and easy to eyeball.
        using (var repository = new ClipboardRepository(paths.Database))
        {
            foreach (var blob in repository.Clear())
            {
                TryDelete(System.IO.Path.Combine(paths.Blobs, blob));
            }
        }

        var lines = new List<string>();
        var baseTime = DateTime.Now.AddMinutes(-count * 3);

        using (var repository = new ClipboardRepository(paths.Database))
        {
            for (var i = 0; i < count; i++)
            {
                var createdAt = baseTime.AddMinutes(i * 3);
                ClipboardItem item;

                // Every seventh entry is a file copy, every eleventh an image,
                // the rest text, so all three card styles appear in the strip.
                if (i % 11 == 5)
                {
                    var setName = string.Join(
                        Environment.NewLine,
                        SampleFileSets.Take(1 + i % SampleFileSets.Length));

                    item = new ClipboardItem
                    {
                        Type = ClipboardItemType.Files,
                        Content = setName,
                        Hash = $"seed-files-{i}",
                        CreatedAt = createdAt,
                    };
                }
                else
                {
                    var text = $"[{i + 1:00}] {SampleTexts[i % SampleTexts.Length]}";
                    item = new ClipboardItem
                    {
                        Type = ClipboardItemType.Text,
                        Content = text,
                        Hash = $"seed-text-{i}",
                        CreatedAt = createdAt,
                    };
                }

                // Pin a couple so the PIN badge and ordering both get exercised.
                if (i is 2 or 18)
                {
                    item.Pinned = true;
                }

                var added = repository.Add(item);
                if (added is not null && item.Pinned)
                {
                    repository.SetPinned(added.Id, true);
                }

                lines.Add($"#{i + 1:00} {item.Type,-5} {item.Content.Replace("\r", " ").Replace("\n", " | ")}");
            }
        }

        var reportPath = System.IO.Path.Combine(paths.Root, "seed.txt");
        System.IO.File.WriteAllLines(reportPath, lines);
    }

    private static void TryDelete(string path)
    {
        try
        {
            if (System.IO.File.Exists(path))
            {
                System.IO.File.Delete(path);
            }
        }
        catch (Exception)
        {
            // Best-effort cleanup.
        }
    }
}
