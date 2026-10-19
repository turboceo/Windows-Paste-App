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
