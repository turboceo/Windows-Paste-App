//! Small Win32 window helpers: square corners and cursor-monitor centering.

use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

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

/// 是否启用 DWM 亚克力。
///
/// 亚克力会让 DWM 对窗口**背后**的屏幕内容实时采样并模糊；窗口每移动一个像素，
/// 这片区域就得重新采样一次。它是 Windows 上拖动窗口掉帧最常见的原因，
/// 尤其在中低端显卡或副屏上。
///
/// 关掉之后面板回退成 `--panel-bg` 这种接近不透明的纯色，视觉上朴素一些，
/// 但拖动会流畅很多。需要毛玻璃观感时再打开。
const ACRYLIC_ENABLED: bool = false;

/// Applies the native window frame touches: square corners, and optionally an
/// acrylic backdrop so the panel reads like a floating Paste.app surface.
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

        if ACRYLIC_ENABLED {
            // Acrylic blur behind the transparent webview. Failures are non-fatal:
            // the panel still renders with its own opaque-ish background.
            let _ = window_vibrancy::apply_acrylic(window, Some((246, 248, 251, 180)));
        }
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
