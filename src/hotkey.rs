use std::thread;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
    VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostQuitMessage, RegisterClassW, TranslateMessage, MSG, WNDCLASSW, WM_DESTROY,
    WM_HOTKEY,
};

use crate::clipboard::wide_str;

const HOTKEY_ID: i32 = 0x5041; // "PA"

pub struct HotkeyManager;

impl HotkeyManager {
    /// Spawns a dedicated thread listening for Ctrl+Shift+V
    pub fn start<F>(on_pressed: F)
    where
        F: Fn() + Send + 'static,
    {
        thread::spawn(move || unsafe {
            let class_name = wide_str("WindowsPaste_HotkeyWindow");

            let wnd_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: std::ptr::null_mut(),
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };

            RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );

            if hwnd.is_null() {
                return;
            }

            let registered = RegisterHotKey(
                hwnd,
                HOTKEY_ID,
                MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
                VK_V as u32,
            );

            if registered != 0 {
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    if msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID as usize {
                        on_pressed();
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                UnregisterHotKey(hwnd, HOTKEY_ID);
            }

            DestroyWindow(hwnd);
        });
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
