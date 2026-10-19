//! Tauri command boundary.
//!
//! A single `dispatch` command mirrors the request shape the React UI already
//! speaks (`{ type, ... }`), which keeps the frontend bridge thin and lets the
//! existing Vue component logic translate almost line for line.

use std::sync::MutexGuard;

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

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
    Columns { columns: i64 },
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
        .map(|i| ClipItemDto::from_item(i, &state.paths.blobs))
        .collect();

    emit_push(
        app,
        json!({
            "type": "items",
            "items": items,
            "isEmpty": items.is_empty(),
            "totalCount": state.total_count,
            "allTags": state.all_tags,
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
            let _ = guard.db.delete(item_id);
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
        HostRequest::Columns { columns } => {
            let mut guard = lock_mut(&state);
            guard.columns = columns.max(1) as usize;
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

fn move_selection(state: &mut AppState, delta: i64) {
    if state.items.is_empty() {
        return;
    }

    let current = state
        .selected_id
        .and_then(|id| state.items.iter().position(|i| i.id == id))
        .unwrap_or(0);

    let len = state.items.len() as i64;
    let vertical = delta.unsigned_abs() >= state.columns as u64 && delta.abs() > 1;
    let step = if vertical { state.columns as i64 } else { 1 };
    let next = (current as i64 + delta.signum() * step).clamp(0, len - 1);

    state.selected_id = Some(state.items[next as usize].id);
}

fn lock<'a>(state: &'a State<'_, SharedState>) -> MutexGuard<'a, AppState> {
    state.lock().expect("app state poisoned")
}

fn lock_mut<'a>(state: &'a State<'_, SharedState>) -> MutexGuard<'a, AppState> {
    state.lock().expect("app state poisoned")
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
    }
}
