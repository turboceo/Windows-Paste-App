//! Windows Paste (Rust + Slint 极速原生重构版)
//!
//! 核心设计理念：
//! 1. 毫秒级呼出响应：纯原生 Direct3D 加速渲染，无任何 Chromium / Node.js / .NET 运行时开销；
//! 2. 深度复刻 macOS Paste 交互：格式自动嗅探着色、自适应视口栅格布局、置顶与模糊搜索；
//! 3. 原生系统级深度整合：Win32 消息级无损剪贴板监听、全局热键、托盘常驻以及基于 SendInput 的平滑智能粘贴。

pub mod clipboard;
pub mod db;
pub mod hotkey;
pub mod model;
pub mod paste;
pub mod tray;

use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer,
    TimerMode, VecModel,
};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, GetWindowRect, IsWindowVisible, IsZoomed, SendMessageW, SetWindowPos,
    ShowWindow, HTCAPTION, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_MINIMIZE,
    SW_RESTORE, SW_SHOW, WM_NCLBUTTONDOWN,
};

use crate::clipboard::{wide_str, ClipboardMonitor, ClipboardSniffer};
use crate::db::{AppPaths, Database};
use crate::hotkey::HotkeyManager;
use crate::model::{ClipboardItem, ClipboardItemType};
use crate::paste::PasteService;
use crate::tray::TrayIcon;

slint::include_modules!();

const WINDOW_TITLE: &str = "Windows Paste";
const DEFAULT_COLUMNS: usize = 4;
const FIXED_COLUMNS: usize = 4;

/// 应用程序全局运行状态上下文
struct AppState {
    db: Database,
    paths: AppPaths,
    sniffer: ClipboardSniffer,
    items: Vec<ClipboardItem>,
    total_count: usize,
    selected_id: Option<i64>,
    search_text: String,
    columns: usize,
    last_target_hwnd: isize,
    is_visible: bool,
}

impl AppState {
    fn new(paths: AppPaths, capacity: usize) -> Self {
        let db = Database::open(&paths.db, capacity).expect("Failed to open SQLite database");
        let sniffer = ClipboardSniffer::new(&paths.blobs);
        let total = db.get_total_count().unwrap_or(0);
        let items = db.get_all().unwrap_or_default();
        let selected_id = items.first().map(|i| i.id);

        Self {
            db,
            paths,
            sniffer,
            items,
            total_count: total,
            selected_id,
            search_text: String::new(),
            columns: DEFAULT_COLUMNS,
            last_target_hwnd: 0,
            is_visible: true,
        }
    }

    /// 根据当前搜索条件从本地 SQLite 重新拉取剪贴板项，并维持合法焦点
    fn reload(&mut self) {
        self.total_count = self.db.get_total_count().unwrap_or(0);
        self.items = if self.search_text.is_empty() {
            self.db.get_all().unwrap_or_default()
        } else {
            self.db.search(&self.search_text).unwrap_or_default()
        };

        if let Some(cur_id) = self.selected_id {
            if !self.items.iter().any(|i| i.id == cur_id) {
                self.selected_id = self.items.first().map(|i| i.id);
            }
        } else {
            self.selected_id = self.items.first().map(|i| i.id);
        }
    }
}

/// 查找主界面的 Win32 HWND 句柄
fn get_app_hwnd() -> HWND {
    unsafe { FindWindowW(std::ptr::null(), wide_str(WINDOW_TITLE).as_ptr()) }
}

/// 禁用 Windows 11/10 默认的窗口圆角拉伸，保持 Paste.app 纯正直角与自渲染圆角
fn apply_square_corners(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let preference: i32 = 1; // DWMWCP_DONOTROUND
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &preference as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

/// 将窗口准确定位到当前鼠标光标所在的显示器中央
fn position_center_on_cursor(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        // 如果处于最大化状态，先还原为优雅的悬浮窗口，杜绝全屏最大化
        if IsZoomed(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }

        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);

        let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;

        if GetMonitorInfoW(hmon, &mut mi) != 0 {
            let work = mi.rcWork;
            let work_w = work.right - work.left;
            let work_h = work.bottom - work.top;

            // 固定 4 列卡片的精致悬浮面板尺寸（标准宽度约 1080px，高度约占工作区 58%）
            let target_w = (1080.0f32).min(work_w as f32 * 0.90) as i32;
            let target_h = (work_h as f32 * 0.58).clamp(520.0, 680.0) as i32;

            let x = work.left + (work_w - target_w) / 2;
            let y = work.top + (work_h - target_h) / 2;

            SetWindowPos(hwnd, 0 as HWND, x, y, target_w, target_h, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

/// 读取本地 blobs 缓存的 PNG 图片并构造 Slint 原生 Image 数据
fn load_thumbnail(blobs_dir: &Path, blob_path: &Option<String>) -> Option<Image> {
    let name = blob_path.as_ref()?;
    let path = blobs_dir.join(name);
    let bytes = fs::read(path).ok()?;
    let dyn_img = image::load_from_memory(&bytes).ok()?;
    let rgba = dyn_img.to_rgba8();
    let (w, h) = rgba.dimensions();

    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        bytemuck::cast_slice(rgba.as_raw()),
        w,
        h,
    );
    Some(Image::from_rgba8(buffer))
}

/// 将数据实体转换为 Slint 界面渲染所需的行分段模型数据 (SlintClipRow)
fn map_to_slint_rows(
    items: &[ClipboardItem],
    selected_id: Option<i64>,
    blobs_dir: &Path,
    columns: usize,
) -> Vec<SlintClipRow> {
    let cols = columns.max(1);
    let mut rows: Vec<SlintClipRow> = Vec::new();

    for chunk in items.chunks(cols) {
        let mut row_items = Vec::new();
        for item in chunk {
            let is_selected = selected_id == Some(item.id);
            let is_url = item.item_type == ClipboardItemType::Text
                && (item.content.starts_with("http://") || item.content.starts_with("https://"));

            let is_color = item.item_type == ClipboardItemType::Text && {
                let s = item.content.trim();
                s.starts_with('#') && (s.len() == 4 || s.len() == 7 || s.len() == 9)
            };

            let category = match item.item_type {
                ClipboardItemType::Image => "image",
                ClipboardItemType::Files => "files",
                ClipboardItemType::Text => {
                    if is_url {
                        "link"
                    } else if is_color {
                        "color"
                    } else {
                        "text"
                    }
                }
            };

            let (cat_name, cat_glyph) = match category {
                "image" => ("图像", "\u{EB9F}"),
                "files" => ("文件", "\u{E8B7}"),
                "link" => ("链接", "\u{E71B}"),
                "color" => ("颜色", "\u{E790}"),
                _ => ("文本", "\u{E8A5}"),
            };

            let meta_badge = match category {
                "link" => {
                    let s = item.content.trim();
                    s.split("://")
                        .nth(1)
                        .and_then(|after| after.split('/').next())
                        .unwrap_or("网页")
                        .to_string()
                }
                "files" => {
                    let count = item.content.lines().count();
                    format!("{} 个文件", count)
                }
                "image" => "图像".to_string(),
                _ => format!("{} 字符", item.content.chars().count()),
            };

            let thumb = if item.item_type == ClipboardItemType::Image {
                load_thumbnail(blobs_dir, &item.blob_path)
            } else {
                None
            };

            let has_image = thumb.is_some();
            let image_data = thumb.unwrap_or_default();

            row_items.push(SlintClipItem {
                id: item.id as i32,
                category: SharedString::from(category),
                category_name: SharedString::from(cat_name),
                category_glyph: SharedString::from(cat_glyph),
                content: SharedString::from(&item.content),
                preview: SharedString::from(item.preview()),
                meta_badge: SharedString::from(meta_badge),
                created_at: SharedString::from(item.created_at_label()),
                pinned: item.pinned,
                selected: is_selected,
                has_image,
                image_data,
            });
        }

        rows.push(SlintClipRow {
            items: ModelRc::new(VecModel::from(row_items)),
        });
    }

    rows
}

/// 获取当前窗口实际渲染的物理宽度
fn get_window_width() -> f32 {
    let hwnd = get_app_hwnd();
    if !hwnd.is_null() {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        if unsafe { GetWindowRect(hwnd, &mut rect) } != 0 {
            let w = (rect.right - rect.left) as f32;
            if w > 400.0 {
                return w;
            }
        }
    }
    1400.0
}

/// 视口动态自适应算法：根据当前可用宽度智能推导最佳列数与等比卡片像素大小
/// 视口布局算法：严格保持每行 4 个卡片展示，根据当前宽度均匀铺满
fn compute_autofit_layout(window_width: f32) -> (usize, f32) {
    let padding_horizontal = 40.0; // 左右 padding 各 20px
    let scrollbar_reserve = 14.0;
    let usable_width = (window_width - padding_horizontal - scrollbar_reserve).max(300.0);
    let gap = 16.0;

    // 严格固定每行 4 列
    let columns = FIXED_COLUMNS;
    let card_size_px = (usable_width - (columns as f32 - 1.0) * gap) / columns as f32;

    (columns, card_size_px)
}

/// 同步更新 Slint UI 界面的所有属性与数据源
fn sync_ui(window: &MainWindow, state: &mut AppState) {
    let width = get_window_width();
    let (columns, card_size) = compute_autofit_layout(width);
    state.columns = columns;

    window.set_card_size(card_size);

    let rows = map_to_slint_rows(
        &state.items,
        state.selected_id,
        &state.paths.blobs,
        state.columns,
    );
    window.set_rows(ModelRc::new(VecModel::from(rows)));
    window.set_total_count(state.total_count as i32);
    window.set_filtered_count(state.items.len() as i32);

    let sel_idx = if let Some(sid) = state.selected_id {
        state.items.iter().position(|i| i.id == sid).unwrap_or(0)
    } else {
        0
    };
    window.set_can_prev(sel_idx > 0);
    window.set_can_next(!state.items.is_empty() && sel_idx < state.items.len() - 1);
}

/// 灌入初始模拟样本数据（用于调试与展示）
fn seed_sample_data(paths: &AppPaths, count: usize) {
    let mut db = Database::open(&paths.db, 500).expect("Cannot open DB for seeding");
    let _ = db.clear();

    let samples = [
        "https://github.com/slint-ui/slint",
        "cargo build --release",
        "SELECT Id, Type, Content FROM ClipboardItems ORDER BY CreatedAt DESC",
        "The quick brown fox jumps over the lazy dog",
        "好记性不如烂笔头",
        "git rebase --interactive HEAD~3",
        "C:\\Users\\Pyzl\\Documents\\report.pdf",
        "#3b82f6",
    ];

    let base_time = chrono::Local::now() - chrono::Duration::minutes((count * 3) as i64);

    for i in 0..count {
        let created_at = base_time + chrono::Duration::minutes((i * 3) as i64);
        let sample = samples[i % samples.len()];
        let item_type = if sample.contains('\\') {
            ClipboardItemType::Files
        } else {
            ClipboardItemType::Text
        };

        let mut item = ClipboardItem {
            id: 0,
            item_type,
            content: format!("[{:02}] {}", i + 1, sample),
            blob_path: None,
            hash: format!("seed-{}", i),
            created_at,
            pinned: i == 2 || i == 18,
        };

        let _ = db.add(&mut item);
        if item.pinned {
            let _ = db.set_pinned(item.id, true);
        }
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let paths = AppPaths::get();

    // 检查命令行是否指定灌入测试数据
    let args: Vec<String> = env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--seed") {
        let count = args
            .get(pos + 1)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(50);
        seed_sample_data(&paths, count);
        println!("Seeded {} test entries successfully.", count);
    }

    let state = Arc::new(Mutex::new(AppState::new(paths, 500)));

    let window = MainWindow::new()?;
    let win_handle = window.as_weak();

    // 初始化界面首次布局同步
    {
        let mut st = state.lock().unwrap();
        sync_ui(&window, &mut st);
    }

    // 1. 初始化 Win32 边框属性与光标屏幕居中
    {
        let win_weak = win_handle.clone();
        slint::Timer::single_shot(Duration::from_millis(50), move || {
            let hwnd = get_app_hwnd();
            if !hwnd.is_null() {
                apply_square_corners(hwnd);
                position_center_on_cursor(hwnd);
            }
            if let Some(w) = win_weak.upgrade() {
                w.show().ok();
            }
        });
    }

    // 2. 剪贴板后台事件监听驱动服务 (AddClipboardFormatListener)
    let clip_monitor = ClipboardMonitor::new();
    {
        let state_clip = state.clone();
        let win_weak = win_handle.clone();
        clip_monitor.start(move || {
            let mut st = state_clip.lock().unwrap();
            if let Some(mut item) = st.sniffer.capture() {
                if let Ok(added) = st.db.add(&mut item) {
                    if added {
                        st.reload();
                        let weak = win_weak.clone();
                        let st_copy = state_clip.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak.upgrade() {
                                let mut lock = st_copy.lock().unwrap();
                                sync_ui(&w, &mut lock);
                            }
                        });
                    }
                }
            }
        });
    }

    // 3. 全局热键唤出监听 (Ctrl+Shift+V)
    {
        let state_hotkey = state.clone();
        let win_weak = win_handle.clone();
        HotkeyManager::start(move || {
            let hwnd = get_app_hwnd();
            let is_vis = unsafe { IsWindowVisible(hwnd) != 0 };

            if is_vis {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
                let mut st = state_hotkey.lock().unwrap();
                st.is_visible = false;
            } else {
                let fg_hwnd = PasteService::get_foreground_window();
                {
                    let mut st = state_hotkey.lock().unwrap();
                    st.last_target_hwnd = fg_hwnd as isize;
                    st.search_text.clear();
                    st.reload();
                }

                let weak = win_weak.clone();
                let st_copy = state_hotkey.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let hwnd = get_app_hwnd();
                    position_center_on_cursor(hwnd);
                    unsafe {
                        ShowWindow(hwnd, SW_SHOW);
                    }
                    if let Some(w) = weak.upgrade() {
                        let mut lock = st_copy.lock().unwrap();
                        w.set_search_text(SharedString::new());
                        sync_ui(&w, &mut lock);
                    }
                });
            }
        });
    }

    // 4. 任务栏系统托盘服务 (TrayIcon: 唤起/清空/退出)
    {
        let win_weak = win_handle.clone();
        let state_tray = state.clone();
        let state_clear = state.clone();
        let win_weak_clear = win_handle.clone();

        TrayIcon::start(
            // 左键点击或右键菜单唤起面板
            move || {
                let weak = win_weak.clone();
                let st_copy = state_tray.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let hwnd = get_app_hwnd();
                    let is_vis = unsafe { IsWindowVisible(hwnd) != 0 };
                    if is_vis {
                        unsafe {
                            ShowWindow(hwnd, SW_HIDE);
                        }
                    } else {
                        let fg_hwnd = PasteService::get_foreground_window();
                        {
                            let mut lock = st_copy.lock().unwrap();
                            lock.last_target_hwnd = fg_hwnd as isize;
                            lock.search_text.clear();
                            lock.reload();
                        }
                        position_center_on_cursor(hwnd);
                        unsafe {
                            ShowWindow(hwnd, SW_SHOW);
                        }
                        if let Some(w) = weak.upgrade() {
                            let mut lock = st_copy.lock().unwrap();
                            w.set_search_text(SharedString::new());
                            sync_ui(&w, &mut lock);
                        }
                    }
                });
            },
            // 右键菜单清空历史记录
            move || {
                let weak = win_weak_clear.clone();
                let st_copy = state_clear.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let mut lock = st_copy.lock().unwrap();
                    let _ = lock.db.clear();
                    lock.reload();
                    if let Some(w) = weak.upgrade() {
                        sync_ui(&w, &mut lock);
                    }
                });
            },
            // 右键菜单彻底退出程序
            move || {
                let _ = slint::invoke_from_event_loop(|| {
                    slint::quit_event_loop().ok();
                });
            },
        );
    }

    // 5. 交互回调绑定与事件分派
    // (1) 鼠标单击卡片选中
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_card_selected(move |id| {
            let mut st = state_ui.lock().unwrap();
            st.selected_id = Some(id as i64);
            if let Some(w) = win_weak.upgrade() {
                sync_ui(&w, &mut st);
            }
        });
    }

    // (2) 鼠标双击卡片 -> 隐藏窗口并模拟粘贴
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_card_double_clicked(move |id| {
            let (item_opt, target_hwnd, blobs_dir) = {
                let st = state_ui.lock().unwrap();
                let item = st.items.iter().find(|i| i.id == id as i64).cloned();
                (item, st.last_target_hwnd, st.paths.blobs.clone())
            };

            let hwnd = get_app_hwnd();
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }

            if let Some(w) = win_weak.upgrade() {
                w.hide().ok();
            }

            if let Some(item) = item_opt {
                PasteService::paste_item(&item, target_hwnd as HWND, &blobs_dir);
            }
        });
    }

    // (3) 置顶/取消置顶 (Pin)
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_toggle_pin(move |id| {
            let mut st = state_ui.lock().unwrap();
            if let Some(item) = st.items.iter().find(|i| i.id == id as i64) {
                let _ = st.db.set_pinned(item.id, !item.pinned);
                st.reload();
                if let Some(w) = win_weak.upgrade() {
                    sync_ui(&w, &mut st);
                }
            }
        });
    }

    // (4) 删除记录 (Remove)
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_remove_item(move |id| {
            let mut st = state_ui.lock().unwrap();
            let _ = st.db.delete(id as i64);
            st.reload();
            if let Some(w) = win_weak.upgrade() {
                sync_ui(&w, &mut st);
            }
        });
    }

    // (5) 标题栏拖拽移动
    window.on_drag_move(|| {
        let hwnd = get_app_hwnd();
        if !hwnd.is_null() && unsafe { IsZoomed(hwnd) == 0 } {
            unsafe {
                ReleaseCapture();
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
        }
    });



    // (7) 最小化窗口
    window.on_minimize(|| {
        let hwnd = get_app_hwnd();
        if !hwnd.is_null() {
            unsafe {
                ShowWindow(hwnd, SW_MINIMIZE);
            }
        }
    });

    // (8) 关闭窗口（隐藏至托盘）
    {
        let win_weak = win_handle.clone();
        window.on_close_window(move || {
            let hwnd = get_app_hwnd();
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            if let Some(w) = win_weak.upgrade() {
                w.hide().ok();
            }
        });
    }

    // (9) 搜索框输入防抖处理 (Debounce 120ms，消除高频键盘重绘开销)
    {
        let search_timer = Arc::new(Mutex::new(Timer::default()));
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        let timer_clone = search_timer.clone();

        window.on_search_edited(move |text| {
            let t = text.to_string();
            let state_c = state_ui.clone();
            let weak_c = win_weak.clone();

            let timer = timer_clone.lock().unwrap();
            timer.start(
                TimerMode::SingleShot,
                Duration::from_millis(120),
                move || {
                    let mut st = state_c.lock().unwrap();
                    st.search_text = t.clone();
                    st.reload();
                    if let Some(w) = weak_c.upgrade() {
                        sync_ui(&w, &mut st);
                    }
                },
            );
        });
    }

    // (10) 窗口动态拉伸自适应布局回调 (Resize Adaptation)
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_window_resized(move |_new_width| {
            if let Some(w) = win_weak.upgrade() {
                let mut st = state_ui.lock().unwrap();
                sync_ui(&w, &mut st);
            }
        });
    }

    // (11) 键盘上/下/左/右矩阵式焦点导航 (Matrix Keyboard Navigation)
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_key_navigate(move |direction| {
            let mut st = state_ui.lock().unwrap();
            let total = st.items.len();
            if total == 0 {
                return;
            }
            let cur_idx = st
                .selected_id
                .and_then(|id| st.items.iter().position(|i| i.id == id))
                .unwrap_or(0);

            let new_idx = match direction.as_str() {
                "left" => {
                    if cur_idx > 0 {
                        cur_idx - 1
                    } else {
                        cur_idx
                    }
                }
                "right" => {
                    if cur_idx + 1 < total {
                        cur_idx + 1
                    } else {
                        cur_idx
                    }
                }
                "up" => {
                    if cur_idx >= st.columns {
                        cur_idx - st.columns
                    } else {
                        cur_idx
                    }
                }
                "down" => {
                    if cur_idx + st.columns < total {
                        cur_idx + st.columns
                    } else if cur_idx < total - 1 {
                        total - 1
                    } else {
                        cur_idx
                    }
                }
                _ => cur_idx,
            };

            if new_idx != cur_idx {
                st.selected_id = Some(st.items[new_idx].id);
                if let Some(w) = win_weak.upgrade() {
                    sync_ui(&w, &mut st);
                }
            }
        });
    }

    // (12) 键盘 Enter 快捷粘贴当前选中条目
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_key_paste_selected(move || {
            let (item_opt, target_hwnd, blobs_dir) = {
                let st = state_ui.lock().unwrap();
                let item = st
                    .selected_id
                    .and_then(|id| st.items.iter().find(|i| i.id == id).cloned());
                (item, st.last_target_hwnd, st.paths.blobs.clone())
            };

            let hwnd = get_app_hwnd();
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            if let Some(w) = win_weak.upgrade() {
                w.hide().ok();
            }

            if let Some(item) = item_opt {
                PasteService::paste_item(&item, target_hwnd as HWND, &blobs_dir);
            }
        });
    }

    // (13) 键盘 Delete 快捷删除当前选中记录
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_key_delete_selected(move || {
            let mut st = state_ui.lock().unwrap();
            if let Some(cur_id) = st.selected_id {
                let _ = st.db.delete(cur_id);
                st.reload();
                if let Some(w) = win_weak.upgrade() {
                    sync_ui(&w, &mut st);
                }
            }
        });
    }

    // (14) 键盘 Ctrl + P 快捷切换当前条目置顶状态
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_key_toggle_pin_selected(move || {
            let mut st = state_ui.lock().unwrap();
            if let Some(cur_id) = st.selected_id {
                if let Some(item) = st.items.iter().find(|i| i.id == cur_id) {
                    let _ = st.db.set_pinned(item.id, !item.pinned);
                    st.reload();
                    if let Some(w) = win_weak.upgrade() {
                        sync_ui(&w, &mut st);
                    }
                }
            }
        });
    }

    // (15) 底部工具栏卡片切换上一个/下一个
    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_prev_card(move || {
            let mut st = state_ui.lock().unwrap();
            if let Some(cur_id) = st.selected_id {
                if let Some(idx) = st.items.iter().position(|i| i.id == cur_id) {
                    if idx > 0 {
                        st.selected_id = Some(st.items[idx - 1].id);
                        if let Some(w) = win_weak.upgrade() {
                            sync_ui(&w, &mut st);
                        }
                    }
                }
            }
        });
    }

    {
        let state_ui = state.clone();
        let win_weak = win_handle.clone();
        window.on_next_card(move || {
            let mut st = state_ui.lock().unwrap();
            if let Some(cur_id) = st.selected_id {
                if let Some(idx) = st.items.iter().position(|i| i.id == cur_id) {
                    if idx + 1 < st.items.len() {
                        st.selected_id = Some(st.items[idx + 1].id);
                        if let Some(w) = win_weak.upgrade() {
                            sync_ui(&w, &mut st);
                        }
                    }
                }
            }
        });
    }

    // 自动化单测快速退出探针
    if args.iter().any(|a| a == "--test-quit") {
        slint::Timer::single_shot(Duration::from_millis(500), move || {
            let hwnd = get_app_hwnd();
            println!("TEST_HWND: {:?}", hwnd);
            slint::quit_event_loop().ok();
        });
    }

    window.run()
}
