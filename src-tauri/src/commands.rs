//! Tauri command boundary.
//!
//! A single `dispatch` command mirrors the request shape the React UI speaks
//! (`{ type, ... }`), which keeps `bridge.ts` down to two exports and lets
//! components stay completely unaware of Tauri.
//!
//! 状态变化统一通过 `host-push` 事件推回前端，因此前端只是宿主状态的投影：
//! 剪贴板线程新增、托盘清空、界面里的置顶，走的都是同一套 push 助手。

use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::exporter;
use crate::paste::PasteService;
use crate::state::{AppState, ClipItemDto, SelectionDto, SharedState};

/// Requests the React UI sends to the native host.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HostRequest {
    Ready,
    Search { text: String },
    #[serde(rename_all = "camelCase")]
    Select {
        #[serde(alias = "item_id")]
        item_id: i64,
    },
    Move { delta: i64 },
    #[serde(rename_all = "camelCase")]
    Paste {
        #[serde(alias = "item_id")]
        item_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    Delete {
        #[serde(alias = "item_id")]
        item_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    TogglePin {
        #[serde(alias = "item_id")]
        item_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    SetTag {
        #[serde(alias = "item_id")]
        item_id: i64,
        tag: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    FilterTag {
        tag: Option<String>,
    },
    /// 删除一个分类标签（从所有条目上摘掉，条目保留）。
    #[serde(rename_all = "camelCase")]
    DeleteTag { tag: String },
    Columns { columns: i64 },
    /// 导出全部历史到指定路径；`path` 为空时用默认文件名。
    Export { path: Option<String> },
    /// 预检一个导出包：返回"共 N 条、M 条已存在"，不写库。
    #[serde(rename_all = "camelCase")]
    PreviewImport { path: String },
    /// 执行导入。以 hash 幂等。
    #[serde(rename_all = "camelCase")]
    Import { path: String },
    /// 弹出原生"打开文件"对话框，选完直接进入预检。
    PickImportFile,
    /// 清空全部历史（含已缓存的图片文件）。
    ClearAll,
    /// 按需取一张图片条目的缩略图。列表推送里不再内嵌缩略图。
    #[serde(rename_all = "camelCase")]
    LoadThumbnail { item_id: i64 },
    DragMove,
    Minimize,
    Maximize,
    ToggleMaximize,
    Close,
    Quit,
}

/// Push helper so both commands and background threads emit the same envelope.
pub fn emit_push(app: &AppHandle, payload: Value) {
    let _ = app.emit("host-push", payload);
}

pub fn push_items(app: &AppHandle, state: &AppState) {
    let items: Vec<ClipItemDto> = state
        .items
        .iter()
        .map(|item| ClipItemDto::from_item(item, &state.paths.blobs))
        .collect();

    emit_push(
        app,
        json!({
            "type": "items",
            "items": items,
            "isEmpty": items.is_empty(),
            "totalCount": state.total_count,
            "allTags": state.all_tags,
            // 每个分类的条目数，删除确认框用它说明"会影响多少条"
            "tagCounts": state.tag_counts,
            "activeTag": state.active_tag,
        }),
    );
}

pub fn push_selection(app: &AppHandle, state: &AppState) {
    emit_push(app, json!({ "type": "selection", "selection": SelectionDto::from_state(state) }));
}

pub fn push_window_state(app: &AppHandle) {
    let maximized = app
        .get_webview_window("main")
        .and_then(|w| w.is_maximized().ok())
        .unwrap_or(false);
    emit_push(app, json!({ "type": "windowState", "isMaximized": maximized }));
}

/// Main entry point used by the frontend bridge.
#[tauri::command]
pub fn dispatch(app: AppHandle, state: State<'_, SharedState>, req: HostRequest) -> Result<(), String> {
    match req {
        HostRequest::Ready => {
            let guard = lock(&state);
            push_items(&app, &guard);
            push_selection(&app, &guard);
            push_window_state(&app);
            emit_push(&app, json!({ "type": "hotkey", "label": "Ctrl + Shift + V" }));
            // 图片走 asset 协议，前端需要知道 blob 目录的绝对路径
            emit_push(
                &app,
                json!({
                    "type": "paths",
                    "blobsRoot": guard.paths.blobs.to_string_lossy(),
                }),
            );
        }
        HostRequest::Search { text } => {
            let mut guard = lock_mut(&state);
            guard.search_text = text;
            guard.reload();
            push_items(&app, &guard);
            push_selection(&app, &guard);
        }
        HostRequest::Select { item_id } => {
            let mut guard = lock_mut(&state);
            if guard.items.iter().any(|i| i.id == item_id) {
                guard.selected_id = Some(item_id);
            }
            push_selection(&app, &guard);
        }
        HostRequest::Move { delta } => {
            let mut guard = lock_mut(&state);
            move_selection(&mut guard, delta);
            push_selection(&app, &guard);
        }
        HostRequest::Paste { item_id } => {
            paste_item(&app, &state, item_id)?;
        }
        HostRequest::Delete { item_id } => {
            let mut guard = lock_mut(&state);
            // 删行的同时把图片文件一起删掉，否则 blobs/ 会无限增长
            if let Ok(Some(blob)) = guard.db.delete(item_id) {
                let _ = std::fs::remove_file(guard.paths.blobs.join(&blob));
                crate::state::forget_thumbnails(&[blob]);
            }
            guard.reload();
            push_items(&app, &guard);
            push_selection(&app, &guard);
        }
        HostRequest::TogglePin { item_id } => {
            let mut guard = lock_mut(&state);
            if let Some(item) = guard.items.iter().find(|i| i.id == item_id) {
                let _ = guard.db.set_pinned(item.id, !item.pinned);
                guard.reload();
            }
            push_items(&app, &guard);
            push_selection(&app, &guard);
        }
        HostRequest::SetTag { item_id, tag } => {
            let mut guard = lock_mut(&state);
            let _ = guard.db.set_tag(item_id, tag.as_deref());
            guard.reload();
            push_items(&app, &guard);
            push_selection(&app, &guard);
        }
        HostRequest::FilterTag { tag } => {
            let mut guard = lock_mut(&state);
            guard.active_tag = tag;
            guard.reload();
            push_items(&app, &guard);
            push_selection(&app, &guard);
        }
        HostRequest::DeleteTag { tag } => {
            let mut guard = lock_mut(&state);
            let affected = guard.db.delete_tag(&tag).unwrap_or(0);

            // 正在按这个标签筛选时要复位，否则列表会按一个已经不存在的
            // 分类去过滤，界面直接变空，看起来像"删标签把记录也删了"。
            if guard.active_tag.as_deref() == Some(tag.as_str()) {
                guard.active_tag = None;
            }

            guard.reload();
            push_items(&app, &guard);
            push_selection(&app, &guard);
            emit_push(
                &app,
                json!({
                    "type": "toast",
                    "message": format!("已删除分类「{}」，{} 条记录已移出该分类", tag, affected),
                }),
            );
        }
        HostRequest::Columns { columns } => {
            let mut guard = lock_mut(&state);
            guard.columns = columns.max(1) as usize;
        }
        HostRequest::PickImportFile => {
            // GetOpenFileNameW 是阻塞的模态对话框，不能在命令线程里等，
            // 否则这次 invoke 会一直挂到用户点完。放到后台线程，
            // 选完就地完成预检，直接复用 importPreview 这一个事件。
            let picker_app = app.clone();
            std::thread::spawn(move || {
                let picked = crate::filedialog::pick_open_file(
                    "选择要导入的 Windows Paste 备份",
                    "Windows Paste 备份 (*.json)",
                    "json",
                );

                let Some(path_buf) = picked else {
                    return; // 用户取消，什么都不用回
                };
                let path = path_buf.to_string_lossy().to_string();

                // 在线程内部从 AppHandle 取状态：State<'_, T> 借用自 handle，
                // 而 handle 归本线程所有，生命周期足够。
                let picker_state = picker_app.state::<SharedState>();
                let payload = match picker_state.lock() {
                    Ok(guard) => match exporter::preview_import(&guard.db, &path_buf) {
                        Ok(preview) => json!({
                            "type": "importPreview",
                            "path": path,
                            "preview": preview,
                        }),
                        Err(err) => json!({
                            "type": "importPreview",
                            "path": path,
                            "error": err,
                        }),
                    },
                    Err(_) => return,
                };

                let app = picker_app.clone();
                let _ = app.clone().run_on_main_thread(move || emit_push(&app, payload));
            });
        }
        HostRequest::ClearAll => {
            let mut guard = lock_mut(&state);
            // released 是待删的 blob 文件名列表，不是条目数——
            // 条目数要单独取，否则提示里的数字会跟用户预期的对不上。
            let removed_entries = guard.total_count;
            let released = guard.db.clear().unwrap_or_default();
            crate::remove_blobs(&guard.paths.blobs, &released);
            crate::state::forget_thumbnails(&released);
            guard.reload();
            push_items(&app, &guard);
            push_selection(&app, &guard);
            emit_push(
                &app,
                json!({
                    "type": "toast",
                    "message": format!("已清空 {} 条历史记录", removed_entries),
                }),
            );
        }
        HostRequest::Export { path } => {
            let guard = lock(&state);
            let target = path
                .map(PathBuf::from)
                .unwrap_or_else(|| exporter::default_export_dir().join(exporter::default_export_name()));

            match exporter::export_to_file(&guard.db, &guard.paths.blobs, &target) {
                Ok(count) => emit_push(
                    &app,
                    json!({
                        "type": "toast",
                        "message": format!("已导出 {} 条记录到 {}", count, target.display()),
                    }),
                ),
                Err(err) => emit_push(
                    &app,
                    json!({ "type": "toast", "message": format!("导出失败：{}", err) }),
                ),
            }
        }
        HostRequest::PreviewImport { path } => {
            let guard = lock(&state);
            // 借用 path 做预检，再把它原样回传——前端要用它来确认导入
            let payload = match exporter::preview_import(&guard.db, Path::new(&path)) {
                Ok(preview) => json!({ "type": "importPreview", "path": path, "preview": preview }),
                Err(err) => json!({ "type": "importPreview", "path": path, "error": err }),
            };
            emit_push(&app, payload);
        }
        HostRequest::Import { path } => {
            let mut guard = lock_mut(&state);
            // 先把 blob 目录取出来：否则 &mut guard.db 与 &guard.paths 会同时借用 guard
            let blobs_dir = guard.paths.blobs.clone();
            let result = exporter::import_from_file(&mut guard.db, &blobs_dir, Path::new(&path));

            match result {
                Ok(summary) => {
                    guard.reload();
                    push_items(&app, &guard);
                    push_selection(&app, &guard);
                    emit_push(
                        &app,
                        json!({
                            "type": "toast",
                            "message": format!(
                                "导入完成：新增 {} 条，合并 {} 条，跳过 {} 条",
                                summary.inserted, summary.merged, summary.skipped
                            ),
                        }),
                    );
                }
                Err(err) => emit_push(
                    &app,
                    json!({ "type": "toast", "message": format!("导入失败：{}", err) }),
                ),
            }
        }
        HostRequest::LoadThumbnail { item_id } => {
            // 锁内只查 blob 文件名，解码 + base64 放到锁外做——
            // 缩略图可能涉及一次 PNG 解码，不该占着全局锁。
            let lookup = {
                let guard = lock(&state);
                // 先取到需要的两个值再一起交出，避免在 `.map` 闭包里
                // 同时借用 items 和 paths 而和迭代器的借用打架
                let blob_name = guard
                    .items
                    .iter()
                    .find(|i| i.id == item_id)
                    .and_then(|i| i.blob_path.clone());
                blob_name.map(|name| (guard.paths.blobs.clone(), name))
            };

            if let Some((blobs_dir, file_name)) = lookup {
                let thumb = crate::state::thumbnail_for_blob(&blobs_dir, &file_name);
                emit_push(
                    &app,
                    json!({
                        "type": "thumbnail",
                        "itemId": item_id,
                        "imageUrl": thumb,
                    }),
                );
            }
        }
        HostRequest::DragMove => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.start_dragging();
            }
        }
        HostRequest::Minimize => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.minimize();
            }
        }
        HostRequest::Maximize | HostRequest::ToggleMaximize => {
            if let Some(window) = app.get_webview_window("main") {
                let maximized = window.is_maximized().unwrap_or(false);
                let _ = if maximized { window.unmaximize() } else { window.maximize() };
                push_window_state(&app);
            }
        }
        HostRequest::Close => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        HostRequest::Quit => {
            app.exit(0);
        }
    }

    Ok(())
}

/// Writes the selected entry back to the clipboard, hides the panel, and
/// injects Ctrl+V into the previously focused window.
fn paste_item(app: &AppHandle, state: &State<'_, SharedState>, item_id: i64) -> Result<(), String> {
    let (item, target, blobs) = {
        let guard = lock(state);
        let item = guard
            .items
            .iter()
            .find(|i| i.id == item_id)
            .cloned()
            .ok_or_else(|| format!("item {} not found", item_id))?;
        (item, guard.last_target_hwnd, guard.paths.blobs.clone())
    };

    // Hide first so the simulated Ctrl+V reaches the previous foreground window.
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    PasteService::paste_item(&item, target as windows_sys::Win32::Foundation::HWND, &blobs);
    Ok(())
}

/// 移动选中项。
///
/// 约定：`|delta| == 1` 表示"移动一格"，`|delta| > 1` 表示"移动一行"，
/// 符号决定方向；具体一行有几张卡只由后端的 `columns` 决定。
///
/// 之前用 `delta >= columns` 判断跨行，当窗口最大化到 5 列以上时
/// `4 >= 5` 为假，上下键会退化成左右移动一格。
pub fn move_selection(state: &mut AppState, delta: i64) {
    if state.items.is_empty() || delta == 0 {
        return;
    }

    let current = state
        .selected_id
        .and_then(|id| state.items.iter().position(|i| i.id == id))
        .unwrap_or(0);

    let len = state.items.len() as i64;
    let columns = state.columns.max(1) as i64;
    let step = if delta.abs() > 1 { columns } else { 1 };
    let next = (current as i64 + delta.signum() * step).clamp(0, len - 1);

    state.selected_id = Some(state.items[next as usize].id);
}

/// 取状态锁。
///
/// 中毒时**不 panic**，而是把里面那份半残的状态取出来继续用。
/// 原来这里是 `expect("app state poisoned")`：只要有一次 panic 发生在持锁期间，
/// 这把 Mutex 就永久中毒，之后每一次唤出、每一个命令都会 panic——
/// 而 release 又是 `panic = "abort"`，进程不退出但功能全废，看起来就是假死。
///
/// 状态可能是脏的，但那总比整个应用永久不可用好。
pub fn lock<'a>(state: &'a State<'_, SharedState>) -> MutexGuard<'a, AppState> {
    state.lock().unwrap_or_else(|poisoned| {
        eprintln!("[windows-paste] 状态锁曾中毒，已恢复");
        poisoned.into_inner()
    })
}

fn lock_mut<'a>(state: &'a State<'_, SharedState>) -> MutexGuard<'a, AppState> {
    // 与 lock 完全同义：MutexGuard 本身就以 &mut 解引用，之前分成两个函数
    // 只是历史遗留，分开写反而容易只改一处漏掉另一处。
    lock(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_ipc_requests() {
        let pin_json = r#"{"type":"togglePin","itemId":3}"#;
        let pin_res: Result<HostRequest, _> = serde_json::from_str(pin_json);
        assert!(matches!(pin_res, Ok(HostRequest::TogglePin { item_id: 3 })));

        let select_json = r#"{"type":"select","itemId":10}"#;
        let select_res: Result<HostRequest, _> = serde_json::from_str(select_json);
        assert!(matches!(select_res, Ok(HostRequest::Select { item_id: 10 })));

        let paste_json = r#"{"type":"paste","itemId":7}"#;
        let paste_res: Result<HostRequest, _> = serde_json::from_str(paste_json);
        assert!(matches!(paste_res, Ok(HostRequest::Paste { item_id: 7 })));

        let delete_json = r#"{"type":"delete","itemId":12}"#;
        let delete_res: Result<HostRequest, _> = serde_json::from_str(delete_json);
        assert!(matches!(delete_res, Ok(HostRequest::Delete { item_id: 12 })));

        let set_tag_json = r#"{"type":"setTag","itemId":15,"tag":"代码"}"#;
        let set_tag_res: Result<HostRequest, _> = serde_json::from_str(set_tag_json);
        assert!(matches!(set_tag_res, Ok(HostRequest::SetTag { item_id: 15, tag: Some(t) }) if t == "代码"));

        let filter_tag_json = r#"{"type":"filterTag","tag":"工作"}"#;
        let filter_tag_res: Result<HostRequest, _> = serde_json::from_str(filter_tag_json);
        assert!(matches!(filter_tag_res, Ok(HostRequest::FilterTag { tag: Some(t) }) if t == "工作"));

        let export_json = r#"{"type":"export","path":"C:\\Users\\Pyzl\\Desktop\\clip.json"}"#;
        let export_res: Result<HostRequest, _> = serde_json::from_str(export_json);
        assert!(matches!(export_res, Ok(HostRequest::Export { path: Some(_) })));

        let import_json = r#"{"type":"import","path":"C:\\temp\\clip.json"}"#;
        let import_res: Result<HostRequest, _> = serde_json::from_str(import_json);
        assert!(matches!(import_res, Ok(HostRequest::Import { path }) if path.ends_with("clip.json")));

        let preview_json = r#"{"type":"previewImport","path":"C:\\temp\\clip.json"}"#;
        let preview_res: Result<HostRequest, _> = serde_json::from_str(preview_json);
        assert!(matches!(preview_res, Ok(HostRequest::PreviewImport { .. })));
    }

    #[test]
    fn test_move_selection_uses_columns_for_row_moves() {
        use crate::model::{ClipboardItem, ClipboardItemType};
        use chrono::Local;

        fn state_with(items: usize, columns: usize) -> AppState {
            let root = std::env::temp_dir().join(format!(
                "test_wp_move_{}_{}",
                std::process::id(),
                items * 100 + columns
            ));
            let paths = crate::db::AppPaths::at(root);
            let mut state = AppState::new(paths);

            state.items.clear();
            for i in 0..items {
                state.items.push(ClipboardItem {
                    id: (i + 1) as i64,
                    item_type: ClipboardItemType::Text,
                    content: format!("item {}", i),
                    blob_path: None,
                    hash: format!("h{}", i),
                    created_at: Local::now(),
                    pinned: false,
                    tag: None,
                });
            }
            state.total_count = items;
            state.columns = columns;
            state.selected_id = Some(1);
            state
        }

        // 5 列：向下一行应该落在第 6 项（index 5）
        let mut s = state_with(30, 5);
        move_selection(&mut s, 4);
        assert_eq!(s.selected_id, Some(6), "5 列时下移应跨 5 个");

        // 2 列
        let mut s = state_with(30, 2);
        move_selection(&mut s, 4);
        assert_eq!(s.selected_id, Some(3), "2 列时下移应跨 2 个");

        // 单格移动不受列数影响
        let mut s = state_with(30, 5);
        move_selection(&mut s, 1);
        assert_eq!(s.selected_id, Some(2));

        // 越界时夹在末尾
        let mut s = state_with(3, 8);
        move_selection(&mut s, 4);
        assert_eq!(s.selected_id, Some(3), "越界应夹到最后一项");
    }
}
