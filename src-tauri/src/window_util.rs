//! Small Win32 window helpers: square corners and cursor-monitor centering.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

// 记录面板最近一次被显示的时刻，用于挡掉 `show()` 之后那一小段
// "还没真正拿到焦点、系统却先报失焦"的窗口。否则面板会闪一下就消失，
// 观感与假死无异。
//
// 用两个原子量而不是 `Mutex<Option<Instant>>`：后者没法做 `static` 初始化，
// 前者还省掉了一把锁——这个函数会在窗口事件回调里被频繁调用。

/// 进程启动的参考点，`OnceLock` 只写一次。
fn start_instant() -> &'static Instant {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now)
}

/// 上次显示面板的时刻，以"距进程启动的毫秒数"表示；0 表示从未显示过。
static LAST_SHOWN_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_millis() -> u64 {
    start_instant().elapsed().as_millis() as u64
}

/// 记录"刚刚显示了一次面板"。必须在 `show()` 之后立刻调用。
pub fn mark_shown() {
    // +1 是为了避开"恰好 0ms"这个哨兵值
    LAST_SHOWN_MS.store(now_millis() + 1, Ordering::SeqCst);
}

/// 面板是否在 `window` 时长之内刚被显示过。
pub fn recently_shown_within(window: Duration) -> bool {
    let last = LAST_SHOWN_MS.load(Ordering::SeqCst);
    if last == 0 {
        return false;
    }
    now_millis().saturating_sub(last.saturating_sub(1)) < window.as_millis() as u64
}
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

fn hwnd_of(window: &WebviewWindow) -> Option<HWND> {
    window.hwnd().ok().map(|h| h.0 as HWND)
}

/// 显式把 exe 内嵌图标设置为窗口图标（任务栏 / Alt+Tab / 标题栏）。
///
/// 不依赖 Shell 的图标缓存判断：直接 `SendMessageW(WM_SETICON)`，
/// 让系统重新读取图标句柄，因此新图标能立刻生效。
pub fn apply_window_icon(window: &WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_LOADFROMFILE, LR_SHARED,
        WM_SETICON,
    };

    let Some(hwnd) = hwnd_of(window) else {
        return;
    };

    // 优先使用磁盘上的 PNG/ICO，保证每次启动都重新读取，绕开资源缓存。
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("icons").join("icon.ico"));
            candidates.push(dir.join("tray-icon.png"));
        }
    }
    candidates.push(std::path::PathBuf::from("src-tauri/icons/icon.ico"));

    let resolved = candidates
        .into_iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string());

    unsafe {
        let mut big: isize = 0;
        let mut small: isize = 0;

        if let Some(path) = resolved {
            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let h = LoadImageW(
                0 as _,
                wide.as_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE,
            ) as isize;
            if h != 0 {
                big = h;
                small = h;
            }
        }

        if big == 0 {
            // 退回 exe 内嵌资源
            let hinstance = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
                std::ptr::null(),
            );
            for id in [1usize, 2] {
                let h = LoadImageW(
                    hinstance,
                    id as *const u16,
                    IMAGE_ICON,
                    0,
                    0,
                    LR_SHARED,
                ) as isize;
                if h != 0 {
                    big = h;
                    small = h;
                    break;
                }
            }
        }

        if big != 0 {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, big);
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, small);
        }
    }
}

/// Applies the native window frame touches: square corners plus an acrylic
/// backdrop so the panel reads like a floating Paste.app surface.
pub fn polish(window: &WebviewWindow) {
    let Some(hwnd) = hwnd_of(window) else {
        return;
    };

    unsafe {
        let preference: i32 = 1; // DWMWCP_DONOTROUND
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &preference as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<i32>() as u32,
        );

        // Acrylic blur behind the transparent webview. Failures are non-fatal:
        // the panel still renders with its own opaque-ish background.
        let _ = window_vibrancy::apply_acrylic(window, Some((246, 248, 251, 180)));
    }
}

/// Centers the panel on whichever monitor currently owns the mouse cursor.
pub fn center_on_cursor(window: &WebviewWindow) {
    let (width, height) = match window.outer_size() {
        Ok(size) => (size.width, size.height),
        Err(_) => return,
    };

    unsafe {
        let mut point: POINT = std::mem::zeroed();
        if GetCursorPos(&mut point) == 0 {
            return;
        }

        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return;
        }

        let work = info.rcWork;
        let work_w = work.right - work.left;
        let work_h = work.bottom - work.top;

        let x = work.left + (work_w - width as i32) / 2;
        let y = work.top + (work_h - height as i32) / 2;

        let _ = window.set_position(PhysicalPosition::new(x, y));
        let _ = window.set_size(PhysicalSize::new(width, height));
    }
}
