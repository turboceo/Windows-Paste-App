//! Tauri command boundary.
//!
//! A single `dispatch` command mirrors the request shape the React UI speaks
//! (`{ type, ... }`), which keeps `bridge.ts` down to two exports and lets
//! components stay completely unaware of Tauri.
//!
//! 状态变化统一通过 `host-push` 事件推回前端，因此前端只是宿主状态的投影：
//! 剪贴板线程新增、托盘清空、界面里的置顶，走的都是同一套 push 助手。

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::exporter;
use crate::state::{AppState, ClipItemDto, SelectionDto};

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
    /// 更新某条记录的 Content（例如后台异步 OCR 提取结果就绪）
    #[serde(rename_all = "camelCase")]
    UpdateContent {
        #[serde(alias = "item_id")]
        item_id: i64,
        content: String,
    },
    /// 复制指定文本到系统剪贴板
    #[serde(rename_all = "camelCase")]
    CopyText { text: String },
    /// 对指定的图片条目立即执行一次 OCR（已存在的老记录也可以手动触发）
    #[serde(rename_all = "camelCase")]
    RunOcr {
        #[serde(alias = "item_id")]
        item_id: i64,
    },
    /// 仅将内容写入系统剪贴板，不隐藏窗口、不模拟按键
    #[serde(rename_all = "camelCase")]
    Copy {
        #[serde(alias = "item_id")]
        item_id: i64,
    },
    /// 打开数据与文件保存的本地目录
    OpenDataDir,
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
    /// 拖拽排序：把记录移动到 `before_id` 之前；`None` 表示移到本组末尾。
    #[serde(rename_all = "camelCase")]
    Reorder {
        #[serde(alias = "item_id")]
        item_id: i64,
        before_id: Option<i64>,
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
    /// 前端捕获到的运行时错误（window.onerror / ErrorBoundary / unhandledrejection）。
    ///
    /// 前端跑在 WebView 里，报错只出现在 DevTools 控制台，用户和日志都看不到；
    /// 白屏时更是连界面都没了。这里把它落进 `app.log`，事后才有据可查。
    #[serde(rename_all = "camelCase")]
    ReportFrontendError {
        message: String,
        #[serde(default)]
        stack: String,
        #[serde(default)]
        source: String,
    },
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
///
/// 现在它只做一件事：把请求投递给状态所有者线程，然后立刻返回。
///
/// 不在这里加锁、不在这里处理——`dispatch` 跑在 Tauri 的命令线程上，
/// 任何阻塞都会直接表现为"界面点不动"。真正的处理在 `handle` 里，
/// 由 `worker` 线程串行执行。
#[tauri::command]
pub fn dispatch(handle: State<'_, crate::worker::StateHandle>, req: HostRequest) -> Result<(), String> {
    // 只是投递，微秒级返回。前端 `send()` 本来就不等返回值。
    handle.send(crate::worker::Command::Dispatch(req));
    Ok(())
}

/// 真正处理请求。只在 worker 线程上被调用，`state` 是独占的可变引用。
///
/// 这里不再有任何 `lock()`——`state` 的所有权就在本线程手上。
pub fn handle(app: &AppHandle, state: &mut AppState, req: HostRequest) {
    match req {
        HostRequest::Ready => {
            push_items(app, state);
            push_selection(app, state);
            push_window_state(app);
            emit_push(app, json!({ "type": "hotkey", "label": "Ctrl + Shift + V" }));
            // 缩略图改为按需拉取，前端不再需要 blob 目录路径。
            // 保留这个事件只为兼容旧版前端。
            emit_push(
                app,
                json!({
                    "type": "paths",
                    "blobsRoot": state.paths.blobs.to_string_lossy(),
                    "dataRoot": state.paths.root.to_string_lossy(),
                    "version": app.package_info().version.to_string(),
                }),
            );
        }
        HostRequest::Search { text } => {
            state.search_text = text;
            state.reload();
            push_items(app, state);
            push_selection(app, state);
        }
        HostRequest::Select { item_id } => {
            if state.items.iter().any(|i| i.id == item_id) {
                state.selected_id = Some(item_id);
            }
            push_selection(app, state);
        }
        HostRequest::Move { delta } => {
            move_selection(state, delta);
            push_selection(app, state);
        }
        HostRequest::Paste { item_id } => {
            paste_item(app, state, item_id);
        }
        HostRequest::Copy { item_id } => {
            if let Some(item) = state.item_by_id(item_id) {
                let blobs = state.paths.blobs.clone();
                crate::paste::PasteService::write_to_clipboard(&item, &blobs);
                emit_push(
                    app,
                    json!({
                        "type": "toast",
                        "message": "已复制到剪贴板",
                    }),
                );
            }
        }
        HostRequest::CopyText { text } => {
            crate::paste::PasteService::write_to_clipboard(
                &crate::model::ClipboardItem {
                    id: 0,
                    item_type: crate::model::ClipboardItemType::Text,
                    content: text,
                    blob_path: None,
                    hash: String::new(),
                    created_at: chrono::Local::now(),
                    pinned: false,
                    tag: None,
                },
                &state.paths.blobs,
            );
            emit_push(
                app,
                json!({
                    "type": "toast",
                    "message": "已复制提取的文本",
                }),
            );
        }
        HostRequest::UpdateContent { item_id, content } => {
            if state.db.update_content(item_id, &content).is_ok() {
                state.reload();
                push_items(app, state);
                push_selection(app, state);
            }
        }
        HostRequest::RunOcr { item_id } => {
            // 手动对已有图片记录执行 OCR：复制老图片、导入历史、或当初识别失败
            // 的记录都能在这里补上文字，结果同样写回 Content 供搜索与复制。
            let Some(item) = state.item_by_id(item_id) else {
                return;
            };
            if item.item_type != crate::model::ClipboardItemType::Image {
                return;
            }
            let Some(blob_name) = item.blob_path.clone() else {
                emit_push(
                    app,
                    json!({ "type": "toast", "message": "这张图片的缓存文件已丢失，无法识别" }),
                );
                return;
            };
            let blob_path = state.paths.blobs.join(&blob_name);
            if !blob_path.exists() {
                emit_push(
                    app,
                    json!({ "type": "toast", "message": "这张图片的缓存文件已丢失，无法识别" }),
                );
                return;
            }

            // 已有识别结果时先清掉旧的那段，避免反复识别把文字越叠越长
            let base_content = existing_ocr_base(&item.content);
            let app_handle = app.clone();

            emit_push(
                app,
                json!({ "type": "toast", "message": "正在识别图片文字..." }),
            );

            std::thread::spawn(move || {
                let result = crate::ocr::recognize_file(&blob_path);
                let Some(state_handle) = app_handle.try_state::<crate::worker::StateHandle>() else {
                    return;
                };
                match result {
                    Some(recognized) => {
                        state_handle.send(crate::worker::Command::Dispatch(
                            HostRequest::UpdateContent {
                                item_id,
                                content: format!("{}\n\n{}", base_content, recognized),
                            },
                        ));
                        emit_push(
                            &app_handle,
                            json!({ "type": "toast", "message": "图片文字识别完成" }),
                        );
                    }
                    None => emit_push(
                        &app_handle,
                        json!({
                            "type": "toast",
                            "message": "没有识别到文字，或当前系统缺少 OCR 语言包",
                        }),
                    ),
                }
            });
        }
        HostRequest::OpenDataDir => {
            let root = state.paths.root.clone();
            std::thread::spawn(move || {
                let _ = std::process::Command::new("explorer").arg(&root).spawn();
            });
        }
        HostRequest::Delete { item_id } => {
            // 删除前先记下它的槽位，以及它是不是当前选中项。
            // reload() 找不到原选中 id 时会回退到第一项——那会让界面
            // 从用户正看着的位置直接跳回顶部，所以下面要覆盖掉。
            let removed_index = state.items.iter().position(|item| item.id == item_id);
            let was_selected = state.selected_id == Some(item_id);

            // 删行的同时把图片文件一起删掉，否则 blobs/ 会无限增长
            if let Ok(Some(blob)) = state.db.delete(item_id) {
                let _ = std::fs::remove_file(state.paths.blobs.join(&blob));
                crate::state::forget_thumbnails(&[blob]);
            }
            state.reload();

            // 只有删掉的是选中项时才纠正；删别的卡片不应改变当前选择。
            if was_selected {
                state.select_at_index_or_last(removed_index.unwrap_or(0));
            }

            push_items(app, state);
            push_selection(app, state);
        }
        HostRequest::TogglePin { item_id } => {
            if let Some(item) = state.items.iter().find(|i| i.id == item_id) {
                let pinned = item.pinned;
                let _ = state.db.set_pinned(item_id, !pinned);
                state.reload();
            }
            push_items(app, state);
            push_selection(app, state);
        }
        HostRequest::SetTag { item_id, tag } => {
            let _ = state.db.set_tag(item_id, tag.as_deref());
            state.reload();
            push_items(app, state);
            push_selection(app, state);
        }
        HostRequest::Reorder { item_id, before_id } => {
            // 顺序变化本身不改变任何条目内容，失败（例如条目已被删掉）时
            // 保持原样即可，不需要额外提示。
            if state.db.reorder(item_id, before_id).is_ok() {
                state.selected_id = Some(item_id);
                state.reload();
                push_items(app, state);
                push_selection(app, state);
            }
        }
        HostRequest::FilterTag { tag } => {
            state.active_tag = tag;
            state.reload();
            push_items(app, state);
            push_selection(app, state);
        }
        HostRequest::DeleteTag { tag } => {
            let affected = state.db.delete_tag(&tag).unwrap_or(0);

            // 正在按这个标签筛选时要复位，否则列表会按一个已经不存在的
            // 分类去过滤，界面直接变空，看起来像"删标签把记录也删了"。
            if state.active_tag.as_deref() == Some(tag.as_str()) {
                state.active_tag = None;
            }

            state.reload();
            push_items(app, state);
            push_selection(app, state);
            emit_push(
                app,
                json!({
                    "type": "toast",
                    "message": format!("已删除分类「{}」，{} 条记录已移出该分类", tag, affected),
                }),
            );
        }
        HostRequest::Columns { columns } => {
            state.columns = columns.max(1) as usize;
        }
        HostRequest::PickImportFile => {
            // GetOpenFileNameW 是阻塞的模态对话框。它**不能**在 worker 线程上执行，
            // 否则整个状态处理会被用户"还没点完"的对话框卡住，期间任何请求都排队。
            // 放到独立线程，选完再走一次正常的命令流程。
            let picker_app = app.clone();
            let picker_handle = app.state::<crate::worker::StateHandle>().inner().clone();
            std::thread::spawn(move || {
                let picked = crate::filedialog::pick_open_file(
                    "选择要导入的 Windows Paste 备份",
                    "Windows Paste 备份 (*.json)",
                    "json",
                );
                let Some(path_buf) = picked else {
                    return; // 用户取消，什么都不用回
                };
                // 预检仍然交给 owner 线程做，避免在这里抢 DB
                picker_handle.send(crate::worker::Command::Dispatch(
                    HostRequest::PreviewImport {
                        path: path_buf.to_string_lossy().to_string(),
                    },
                ));
                let _ = &picker_app;
            });
        }
        HostRequest::ClearAll => {
            // released 是待删的 blob 文件名列表，不是条目数——
            // 条目数要单独取，否则提示里的数字会跟用户预期的对不上。
            let removed_entries = state.total_count;
            let released = state.db.clear().unwrap_or_default();
            crate::remove_blobs(&state.paths.blobs, &released);
            crate::state::forget_thumbnails(&released);
            state.reload();
            push_items(app, state);
            push_selection(app, state);
            emit_push(
                app,
                json!({
                    "type": "toast",
                    "message": format!("已清空 {} 条历史记录", removed_entries),
                }),
            );
        }
        HostRequest::Export { path } => {
            let target = path.map(PathBuf::from).unwrap_or_else(|| {
                exporter::default_export_dir().join(exporter::default_export_name())
            });

            match exporter::export_to_file(&state.db, &state.paths.blobs, &target) {
                Ok(count) => emit_push(
                    app,
                    json!({
                        "type": "toast",
                        "message": format!("已导出 {} 条记录到 {}", count, target.display()),
                    }),
                ),
                Err(err) => emit_push(
                    app,
                    json!({ "type": "toast", "message": format!("导出失败：{}", err) }),
                ),
            }
        }
        HostRequest::PreviewImport { path } => {
            // 借用 path 做预检，再把它原样回传——前端要用它来确认导入
            let payload = match exporter::preview_import(&state.db, Path::new(&path)) {
                Ok(preview) => json!({ "type": "importPreview", "path": path, "preview": preview }),
                Err(err) => json!({ "type": "importPreview", "path": path, "error": err }),
            };
            emit_push(app, payload);
        }
        HostRequest::Import { path } => {
            let blobs_dir = state.paths.blobs.clone();
            let result = exporter::import_from_file(&mut state.db, &blobs_dir, Path::new(&path));

            match result {
                Ok(summary) => {
                    state.reload();
                    push_items(app, state);
                    push_selection(app, state);
                    emit_push(
                        app,
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
                    app,
                    json!({ "type": "toast", "message": format!("导入失败：{}", err) }),
                ),
            }
        }
        HostRequest::LoadThumbnail { item_id } => {
            // 先把文件名取出来，再和 blob 目录拼——避免在 `.map` 闭包里
            // 同时借用 items 和 paths 而和迭代器的借用打架
            let blob_name = state
                .items
                .iter()
                .find(|i| i.id == item_id)
                .and_then(|i| i.blob_path.clone());
            let lookup = blob_name.map(|name| (state.paths.blobs.clone(), name));

            if let Some((blobs_dir, file_name)) = lookup {
                let thumb = crate::state::thumbnail_for_blob(&blobs_dir, &file_name);
                emit_push(
                    app,
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
                push_window_state(app);
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
        HostRequest::ReportFrontendError { message, stack, source } => {
            // 日志里一行写完，避免多行堆栈被时间戳前缀打断后不好 grep。
            // 堆栈本身换行很多，压缩成 " | " 分隔。
            let flat_stack = stack.replace(['\r', '\n'], " | ");
            crate::logging::log(&format!(
                "[前端错误] source={} message={} stack={}",
                if source.is_empty() { "-" } else { &source },
                message,
                if flat_stack.is_empty() { "-" } else { &flat_stack }
            ));
        }
    }
}

/// 处理"剪贴板新增了一条"。
///
/// 捕获（读剪贴板、解码 DIB、写 PNG）已经在剪贴板线程上做完了，
/// 这里只负责入库与推送——都是纯内存 + 一次 SQLite 写。
pub fn handle_captured(app: &AppHandle, state: &mut AppState, mut item: crate::model::ClipboardItem) {
    if state.db.add(&mut item).is_err() {
        return;
    }
    state.reload();
    push_items(app, state);
    push_selection(app, state);

    // 如果捕获的是图片，并且存在本地 PNG 文件，则在独立后台线程静默执行 WinRT OCR
    // 识别完成后更新该条目的 Content 并刷新推送，使图片内容支持检索
    if matches!(item.item_type, crate::model::ClipboardItemType::Image) {
        if let Some(ref blob_name) = item.blob_path {
            let blob_path = state.paths.blobs.join(blob_name);
            let item_id = item.id;
            let base_content = existing_ocr_base(&item.content);
            let app_handle = app.clone();

            std::thread::spawn(move || {
                if let Some(recognized) = crate::ocr::recognize_file(&blob_path) {
                    let combined = format!("{}\n\n{}", base_content, recognized);
                    if let Some(state_handle) = app_handle.try_state::<crate::worker::StateHandle>() {
                        state_handle.send(crate::worker::Command::Dispatch(HostRequest::UpdateContent {
                            item_id,
                            content: combined,
                        }));
                    }
                }
            });
        }
    }
}

/// 取出一条图片记录内容里的"非 OCR 部分"。
///
/// Content 的约定是 `<尺寸描述>\n\n<OCR 文本>`，重新识别前必须先剥掉旧的
/// OCR 段，否则每触发一次就会把上一次的识别结果再接一遍，越叠越长。
pub fn existing_ocr_base(content: &str) -> String {
    match content.split_once("\n\n") {
        Some((base, _)) => base.trim().to_string(),
        None => content.trim().to_string(),
    }
}

/// 处理"面板被唤出"：记住目标窗口、清空搜索与分类、刷新列表。
pub fn handle_summoned(app: &AppHandle, state: &mut AppState, target_hwnd: isize) {
    let started = std::time::Instant::now();

    state.last_target_hwnd = target_hwnd;
    state.search_text.clear();
    state.active_tag = None;
    state.reload();
    push_items(app, state);
    push_selection(app, state);

    // 搜索词只在"唤出面板"这一刻归零，不能混在 items 推送里，
    // 否则其它来源的推送会把用户正在敲的字擦掉。
    emit_push(app, json!({ "type": "reset" }));

    let elapsed = started.elapsed();
    if elapsed.as_millis() > 200 {
        crate::logging::log_slow("唤出刷新", elapsed);
    }
}

/// 把选中的条目写回剪贴板、隐藏面板、并向之前的窗口注入 Ctrl+V。
///
/// 注意这里**不**在 worker 线程上做真正的粘贴动作：
/// `PasteService::paste_item` 内部有 `sleep(100ms)` 等前台窗口切换完成，
/// 还有一次 DIB 重建，把它压在 worker 线程上会让所有其它请求排队。
/// 所以先在 worker 线程上取好数据副本，再 spawn 一个短命线程去做。
fn paste_item(app: &AppHandle, state: &mut AppState, item_id: i64) {
    let Some(item) = state.item_by_id(item_id) else {
        return;
    };
    let target = state.last_target_hwnd;
    let blobs = state.paths.blobs.clone();

    // 先把面板藏起来，否则模拟的 Ctrl+V 会打到面板自己身上
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    std::thread::spawn(move || {
        crate::paste::PasteService::paste_item(
            &item,
            target as windows_sys::Win32::Foundation::HWND,
            &blobs,
        );
    });
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

        let reorder_json = r#"{"type":"reorder","itemId":3,"beforeId":7}"#;
        let reorder_res: Result<HostRequest, _> = serde_json::from_str(reorder_json);
        assert!(matches!(
            reorder_res,
            Ok(HostRequest::Reorder { item_id: 3, before_id: Some(7) })
        ));

        let reorder_end = r#"{"type":"reorder","itemId":3,"beforeId":null}"#;
        let reorder_end_res: Result<HostRequest, _> = serde_json::from_str(reorder_end);
        assert!(matches!(
            reorder_end_res,
            Ok(HostRequest::Reorder { item_id: 3, before_id: None })
        ));
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

    #[test]
    fn test_existing_ocr_base_strips_previous_result() {
        // 重新识别前必须先剥掉旧的 OCR 段，否则每触发一次就会越叠越长
        assert_eq!(
            existing_ocr_base("1920 x 1080 image\n\n第一行\n第二行"),
            "1920 x 1080 image"
        );
        // 没有 OCR 段时原样返回
        assert_eq!(existing_ocr_base("800 x 600 image"), "800 x 600 image");
        // 首尾空白不该带出来
        assert_eq!(existing_ocr_base("  640 x 480 image  "), "640 x 480 image");
    }
}
