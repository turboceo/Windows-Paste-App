//! Windows Paste (Tauri 2 + React)
//!
//! Native backend: clipboard monitoring, global hotkey, tray, simulated paste,
//! local SQLite history. The visible UI is a React app rendered in the webview.

pub mod clipboard;
pub mod commands;
pub mod db;
pub mod hotkey;
pub mod model;
pub mod paste;
pub mod selftest;
pub mod state;
pub mod tray;
pub mod window_util;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{Manager, WindowEvent};

use crate::clipboard::{ClipboardMonitor, ClipboardSniffer};
use crate::commands::{emit_push, push_items, push_selection};
use crate::db::AppPaths;
use crate::state::{AppState, SharedState};

pub use crate::state::HISTORY_CAPACITY;

/// Tracks whether the panel has been shown at least once. Blur-to-hide is
/// suppressed until the first real summon so startup focus churn is ignored.
#[derive(Default)]
pub struct BlurGuard(pub AtomicBool);

/// Builds and runs the Tauri application.
pub fn run() {
    let paths = AppPaths::get();

    // 命令行参数处理
    let args: Vec<String> = std::env::args().collect();
    let is_selftest = args.iter().any(|a| a == "--selftest");
    if is_selftest {
        seed_sample_data(&paths, 50);
    } else if let Some(index) = args.iter().position(|a| a == "--seed") {
        let count = args
            .get(index + 1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(50);
        seed_sample_data(&paths, count.clamp(1, 1000));
    }

    let state = Mutex::new(AppState::new(paths));

    // Debug helper: `--show` 或 `--selftest` 立即打开面板
    let show_on_start = args.iter().any(|a| a == "--show") || is_selftest;

    tauri::Builder::default()
        .manage(state)
        .manage(BlurGuard::default())
        .invoke_handler(tauri::generate_handler![commands::dispatch])
        .setup(move |app| {
            let handle: tauri::AppHandle = app.handle().clone();

            // Native window polish: square corners, centered on the cursor monitor.
            if let Some(window) = app.get_webview_window("main") {
                window_util::polish(&window);
                // 显式设置窗口/任务栏图标（避免依赖资源缓存）
                window_util::apply_window_icon(&window);
                if show_on_start {
                    window_util::center_on_cursor(&window);
                    window.state::<BlurGuard>().0.store(true, Ordering::SeqCst);
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            // Intercept the OS close so the panel hides to the tray instead of
            // tearing down the process (the app stays resident).
            if let Some(window) = app.get_webview_window("main") {
                let hide_handle = handle.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(w) = hide_handle.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                });
            }

            // Clipboard listener (WM_CLIPBOARDUPDATE on a dedicated Win32 thread).
            let clip_handle = handle.clone();
            ClipboardMonitor::new().start(move || {
                let blobs_dir = {
                    let state = clip_handle.state::<SharedState>();
                    let guard = state.lock().unwrap();
                    guard.paths.blobs.clone()
                };

                let captured = ClipboardSniffer::new(&blobs_dir).capture();
                if let Some(mut item) = captured {
                    let state = clip_handle.state::<SharedState>();
                    let mut guard = state.lock().unwrap();
                    if let Ok(added) = guard.db.add(&mut item) {
                        if added {
                            guard.reload();
                            push_items(&clip_handle, &guard);
                            push_selection(&clip_handle, &guard);
                        }
                    }
                }
            });

            // Global hotkey: Ctrl+Shift+V summons / dismisses the panel.
            let hotkey_handle = handle.clone();
            hotkey::HotkeyManager::start(move || {
                let app = hotkey_handle.clone();
                // Hop back onto the main thread before touching the window.
                let _ = app.clone().run_on_main_thread(move || toggle_panel(&app));
            });

            // System tray: left click opens, right menu clears / exits.
            let tray_show = handle.clone();
            let tray_clear = handle.clone();
            let tray_exit = handle.clone();
            tray::TrayIcon::start(
                move || {
                    let app = tray_show.clone();
                    let _ = app.clone().run_on_main_thread(move || toggle_panel(&app));
                },
                move || {
                    let app = tray_clear.clone();
                    let _ = app.clone().run_on_main_thread(move || clear_history(&app));
                },
                move || {
                    let app = tray_exit.clone();
                    let _ = app.run_on_main_thread(|| std::process::exit(0));
                },
            );

            // 自动化端到端测试流程触发
            if is_selftest {
                selftest::start(handle.clone());
            }

            Ok(())
        })
        .on_window_event(move |window, event| {
            if let WindowEvent::Focused(focused) = event {
                if !focused && !is_selftest {
                    // Only dismiss after the panel has actually been summoned;
                    // otherwise the startup focus transitions would close it.
                    let guard = window.state::<BlurGuard>();
                    if guard.0.load(Ordering::SeqCst) {
                        let _ = window.hide();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Windows Paste");
}

/// Summons or dismisses the panel. Used by the global hotkey and the tray.
pub fn toggle_panel(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let visible = window.is_visible().unwrap_or(false);
    if visible {
        let _ = window.hide();
        return;
    }

    // Remember the foreground window so the paste lands in the right place.
    let foreground = paste::PasteService::get_foreground_window();
    if let Some(state) = app.try_state::<SharedState>() {
        if let Ok(mut guard) = state.lock() {
            guard.last_target_hwnd = foreground as isize;
            guard.search_text.clear();
            guard.reload();
            push_items(app, &guard);
            push_selection(app, &guard);
        }
    }

    window_util::center_on_cursor(&window);
    window.state::<BlurGuard>().0.store(true, Ordering::SeqCst);
    let _ = window.show();
    let _ = window.set_focus();
}

/// Clears all history and refreshes the UI. Used by the tray menu.
pub fn clear_history(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<SharedState>() {
        if let Ok(mut guard) = state.lock() {
            let _ = guard.db.clear();
            guard.reload();
            push_items(app, &guard);
            push_selection(app, &guard);
        }
    }
}

/// Emits a small status message so the UI can react (kept for parity with tray).
pub fn notify(app: &tauri::AppHandle, message: &str) {
    emit_push(app, serde_json::json!({ "type": "toast", "message": message }));
}

/// Fills the history with sample rows so the UI can be inspected without
/// copying real data. Used by `--seed [n]` (default 50).
pub fn seed_sample_data(paths: &AppPaths, count: usize) {
    use crate::model::{ClipboardItem, ClipboardItemType};
    use chrono::Local;

    let mut db = match crate::db::Database::open(&paths.db, state::HISTORY_CAPACITY) {
        Ok(db) => db,
        Err(_) => return,
    };
    let _ = db.clear();

    let samples = [
        "https://github.com/tauri-apps/tauri",
        "npm run tauri dev",
        "SELECT Id, Type, Content FROM ClipboardItems ORDER BY CreatedAt DESC",
        "The quick brown fox jumps over the lazy dog",
        "好记性不如烂笔头",
        "git rebase --interactive HEAD~3",
        "#3b82f6",
        "C:\\Users\\Pyzl\\Documents\\report.pdf",
    ];

    let base = Local::now() - chrono::Duration::minutes((count * 3) as i64);

    for i in 0..count {
        let sample = samples[i % samples.len()];
        let item_type = if sample.contains('\\') {
            ClipboardItemType::Files
        } else {
            ClipboardItemType::Text
        };

        let tag = if i % 6 == 0 {
            Some("常用".to_string())
        } else if i % 6 == 1 {
            Some("代码".to_string())
        } else if i % 6 == 2 {
            Some("工作".to_string())
        } else {
            None
        };

        let mut item = ClipboardItem {
            id: 0,
            item_type,
            content: format!("[{:02}] {}", i + 1, sample),
            blob_path: None,
            hash: format!("seed-{}", i),
            created_at: base + chrono::Duration::minutes((i * 3) as i64),
            pinned: i == 2 || i == 15,
            tag,
        };
        let _ = db.add(&mut item);
    }
}
