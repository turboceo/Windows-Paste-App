# Windows Paste

A Windows clipboard-history manager inspired by [Paste](https://pasteapp.io/) on macOS.
Press a global hotkey to slide up a bottom shelf of your clipboard history, search it,
and paste any entry straight back into the app you were using.

## Features

- Global hotkey (`Ctrl + Shift + V` by default) opens the panel, which drifts up as it fades in
- White frosted (acrylic) panel with a light colour scheme and dark, type-coloured icons
- The panel is centred on the monitor under the cursor, spanning 90% of its width and 60% of its height
- Vertical layout: the card strip fills the top, with a toolbar along the bottom
- Toolbar holds the search box on the left and previous/next navigation on the right
- History appears as a horizontally scrolling strip of square cards, newest on the left
- Cards are tinted by type (text / image / files) with image thumbnails inline
- Captures text, images, and copied files
- Search filters the strip live and rewinds it to the first match
- Keyboard-driven: `Left`/`Right` (or `Up`/`Down`) to move, `Enter` to paste, `Ctrl+P` to pin, `Del` to remove, `Esc` to close
- On-screen previous/next buttons in the toolbar for mouse-driven navigation
- Double-click a card to paste; the shelf opens with the newest entry selected
- Pasting writes the entry back to the clipboard and sends `Ctrl+V` to the previously focused window
- Local-only storage in SQLite under `%LOCALAPPDATA%\WindowsPaste`
- Tray icon with pause capture, clear history, settings, and exit
- Configurable hotkey, history capacity, dark theme, and run-at-login

## Tech stack

| Concern | Choice |
| --- | --- |
| Language / runtime | C# / .NET 10 (WPF) |
| Clipboard monitoring | Win32 `AddClipboardFormatListener` + `WM_CLIPBOARDUPDATE` |
| Global hotkey | `RegisterHotKey` + `WM_HOTKEY` |
| Simulated paste | `SendInput` replaying `Ctrl+V` |
| Frosted backdrop | Win32 `SetWindowCompositionAttribute` acrylic blur |
| Storage | SQLite via `Microsoft.Data.Sqlite` |

No global keyboard hooks are installed, so the app does not trip most
keylogger heuristics: the hotkey is registered with the OS, and the clipboard
listener is message-driven (no polling).

## Layout

```
src/WindowsPaste.App/
  App.xaml(.cs)                 Composition root: single instance, wiring, tray
  Interop/NativeMethods.cs      All P/Invoke declarations
  Models/ClipboardItem.cs       History entry model
  Services/
    MessageWindow.cs            Hidden HWND shared by monitor + hotkeys
    ClipboardMonitor.cs         WM_CLIPBOARDUPDATE listener
    ClipboardSniffer.cs         Reads text / image / file formats, hashes content
    ClipboardRepository.cs      SQLite CRUD, de-duplication, capacity trimming
    HotkeyManager.cs            Global hotkey registration
    PasteService.cs             Clipboard write-back + Ctrl+V injection
    WindowBlur.cs               Acrylic frosted backdrop for the shelf
    AppPaths.cs                 %LOCALAPPDATA% locations
    AppSettings.cs              Registry-backed preferences and run-at-login
  ViewModels/MainViewModel.cs   List, search, selection
  Views/
    PopupWindow.xaml(.cs)       The bottom shelf UI
    SettingsWindow.xaml(.cs)    Hotkey / capacity / theme / startup
    TrayIcon.cs                 NotifyIcon with context menu
  Converters/                   Type glyph, thumbnail, card colour, visibility
  SelfTest.cs                   Debug-only layout harness (--selftest)
```

## Build and run

```powershell
dotnet build
dotnet run --project src/WindowsPaste.App
```

The app starts hidden with a tray icon. Press `Ctrl + Shift + V` to open the overlay.

## Release build (single file)

```powershell
dotnet publish src/WindowsPaste.App -c Release -r win-x64 `
  --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true
```

Output lands in `src/WindowsPaste.App/bin/Release/net10.0-windows/win-x64/publish/`.

## Manual verification checklist

- [ ] Copy text in Notepad, press the hotkey, confirm the panel appears centred and fades in
- [ ] Confirm the panel spans 90% of the screen width and 60% of its height, centred
- [ ] Confirm the layout is vertical: cards on top, toolbar along the bottom
- [ ] Confirm the toolbar has the search box on the left and prev/next arrows on the right
- [ ] Click the arrows and confirm the selection moves between cards (disabled at each end)
- [ ] Confirm the panel is a white frosted surface with a light colour scheme and dark, readable icons
- [ ] Hover a card and confirm its box size does not change (only the ring colour does)
- [ ] Press `Esc` or click away and confirm the panel fades back out
- [ ] Confirm cards scroll horizontally and the newest entry is on the left, pre-selected
- [ ] Confirm each card is square (equal width and height)
- [ ] Search filters the strip as you type and jumps back to the first match
- [ ] `Enter` on a card pastes into the previously focused window
- [ ] Double-clicking a card pastes it
- [ ] Copy an image (e.g. Snipping Tool) and confirm it renders as a thumbnail card
- [ ] Copy files in Explorer and confirm a file-list card appears
- [ ] Copying the same text twice does not create duplicate entries
- [ ] `Ctrl+P` pins an entry; `Del` removes it
- [ ] Tray menu: pause capture stops new entries; clear history empties the shelf
- [ ] Change the hotkey in Settings and confirm the new combination opens the shelf
- [ ] Enable "Start with Windows" and confirm the registry Run value is created
- [ ] Confirm the shelf appears on the monitor where the cursor is, spanning that screen

## Known limitations

- Applications running elevated (for example Task Manager) reject `SendInput`,
  so automatic paste will not reach them. Copy still works via the clipboard.
- If another application already owns the configured hotkey, registration fails
  and a tray notification is shown; pick a different combination in Settings.
- History is capped (default 500 unpinned entries); the oldest entries are evicted
  and their image blobs removed.
